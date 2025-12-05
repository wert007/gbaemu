pub mod conversion;

use std::{collections::HashMap, ops::Index};

use crate::{
    BoundId, Compiler, HasLocation, Location, SourceTextId, StringId,
    bind::conversion::ConversionKind,
    const_evaluator,
    lexer::{Token, TokenKind},
    parser::Parser,
    syntax_tree::*,
    typing::{StructLayout, StructType, Type, TypeId, Types},
    value::Value,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundBinaryOperator {
    Addition,
    Subtraction,
    Multiplication,
    Division,
    Modulo,
}
impl BoundBinaryOperator {
    pub fn diagnostic_name(&self) -> &'static str {
        match self {
            BoundBinaryOperator::Addition => "add",
            BoundBinaryOperator::Subtraction => "subtract",
            BoundBinaryOperator::Multiplication => "multiply",
            BoundBinaryOperator::Division => "divide",
            BoundBinaryOperator::Modulo => "take the remainder",
        }
    }
    #[rustfmt::skip]
    fn resolve_types(
        &self,
        lhs_id: TypeId,
        rhs_id: TypeId,
        types: &mut Types,
    ) -> (TypeId, TypeId, TypeId) {
        let lhs = &types[lhs_id];
        let rhs = &types[rhs_id];
        match (lhs, rhs, self) {
            (Type::UnsignedInteger8, _, Self::Addition | Self::Subtraction | Self::Multiplication | Self::Division | Self::Modulo) |
            (Type::UnsignedInteger16, _, Self::Addition | Self::Subtraction | Self::Multiplication | Self::Division | Self::Modulo) |
            (Type::UnsignedInteger32, _, Self::Addition | Self::Subtraction | Self::Multiplication | Self::Division | Self::Modulo)
            => (lhs_id, lhs_id, lhs_id),
            (_, Type::UnsignedInteger8, Self::Addition | Self::Subtraction | Self::Multiplication | Self::Division | Self::Modulo) |
            (_, Type::UnsignedInteger16, Self::Addition | Self::Subtraction | Self::Multiplication | Self::Division | Self::Modulo) |
            (_, Type::UnsignedInteger32, Self::Addition | Self::Subtraction | Self::Multiplication | Self::Division | Self::Modulo)
            => (rhs_id, rhs_id, rhs_id),
            _ => (lhs_id, rhs_id, TypeId::ERROR)
        }
    }
}

#[derive(Debug)]
pub struct VariableDeclaration {
    pub location: Location,
    pub id: VariableId,
    pub namespaces: Vec<StringId>,
    pub name: StringId,
    pub type_: TypeId,
    pub scope: ScopeId,
    pub can_be_overshadowed: bool,
}

impl HasLocation for VariableDeclaration {
    fn location(&self) -> Location {
        self.location
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(usize);
impl ScopeId {
    fn increase(&mut self) {
        self.0 += 1;
    }
}
#[derive(Debug)]
pub struct Scope {
    id: ScopeId,
    parents: Vec<ScopeId>,
}

impl Scope {
    pub fn global() -> Self {
        Scope {
            id: ScopeId(0),
            parents: Vec::new(),
        }
    }

    fn create_child(&self, next_scope_id: &mut ScopeId) -> Scope {
        let mut parents = self.parents.clone();
        parents.push(self.id);
        let id = *next_scope_id;
        next_scope_id.increase();
        Scope { id, parents }
    }
}

#[derive(Debug)]
pub struct Variables {
    variables: HashMap<ScopeId, Vec<VariableDeclaration>>,
    active_scopes: Vec<Scope>,
    next_scope_id: ScopeId,
    next_variable_id: VariableId,
}

impl Variables {
    pub fn new() -> Self {
        let mut variables = HashMap::new();
        let mut next_scope_id = ScopeId(0);
        let global = Scope::global();
        variables.insert(next_scope_id, Vec::new());
        next_scope_id.increase();
        let result = Self {
            variables,
            active_scopes: vec![global],
            next_scope_id,
            next_variable_id: VariableId(0),
        };
        result
    }

    pub fn register(
        &mut self,
        location: Location,
        namespaces: &[StringId],
        name: StringId,
        type_: TypeId,
        can_be_overshadowed: bool,
    ) -> Option<VariableId> {
        let current_scope = self
            .active_scopes
            .last()
            .expect("There should always be a scope available!")
            .id;
        if self.variables[&current_scope]
            .iter()
            .any(|v| !v.can_be_overshadowed && v.name == name)
        {
            None
        } else {
            let id = self.next_variable_id;
            self.next_variable_id.increase();
            self.variables
                .get_mut(&current_scope)
                .unwrap()
                .push(VariableDeclaration {
                    scope: current_scope,
                    location,
                    namespaces: namespaces.to_vec(),
                    id,
                    name,
                    type_,
                    can_be_overshadowed,
                });
            Some(id)
        }
    }

    fn find_by_name(&self, name: StringId) -> Option<&VariableDeclaration> {
        for scope in self
            .active_scopes
            .last()
            .expect("There should always be a scope available")
            .parents
            .iter()
            .copied()
            .rev()
            .chain(std::iter::once(
                self.active_scopes
                    .last()
                    .expect("There should always be a scope available")
                    .id,
            ))
        {
            if let Some(it) = self.variables[&scope].iter().find(|v| v.name == name) {
                return Some(it);
            }
        }
        None
    }

    fn start_scope(&mut self) {
        let child = self
            .active_scopes
            .last()
            .expect("There should always be a scope available!")
            .create_child(&mut self.next_scope_id);
        self.variables.insert(child.id, Vec::new());
        self.active_scopes.push(child);
    }

    fn end_scope(&mut self) {
        self.active_scopes.pop();
        assert!(self.active_scopes.len() >= 1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VariableId(usize);
impl VariableId {
    fn increase(&mut self) {
        self.0 += 1;
    }
}

#[derive(Debug)]
pub struct Binder {
    file: SourceTextId,
    variables: Variables,
    constants: HashMap<VariableId, Value>,
    namespaces: Vec<StringId>,
}

impl Binder {
    pub fn new(file: SourceTextId) -> Binder {
        Self {
            file,
            variables: Variables::new(),
            constants: HashMap::new(),
            namespaces: Vec::new(),
        }
    }

    pub fn bind(mut self, compiler: &mut Compiler) -> BoundId {
        let tree = Parser::new(self.file).parse(compiler);
        let node = self.bind_node(tree.node, TypeId::VOID, compiler);
        dbg!(self.constants);
        node
    }

    fn bind_node_with_id(
        &mut self,
        node: SyntaxNode<Parsed>,
        expected: TypeId,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> BoundId {
        let location = node.location();
        let node = match node.kind {
            SyntaxNodeKind::Error => {
                dbg!();
                SyntaxNode::<Bound>::error(node.location(), id)
            }
            SyntaxNodeKind::Program(program_node) => {
                self.bind_program(program_node, expected, location, compiler, id)
            }
            SyntaxNodeKind::ConstDeclaration(const_declaration_node) => self
                .bind_const_declaration(const_declaration_node, expected, location, compiler, id),
            SyntaxNodeKind::Literal(token) => self.bind_literal(token, expected, compiler, id),
            SyntaxNodeKind::Binary(binary_node) => {
                self.bind_binary(location, binary_node, expected, compiler, id)
            }
            SyntaxNodeKind::Identifier(identifier) => {
                self.bind_identifier(identifier, expected, compiler, id)
            }
            SyntaxNodeKind::CommaedExpression((e, _)) => {
                return self.bind_node_with_id(*e, expected, compiler, id);
            }
            SyntaxNodeKind::ArrayLiteral(array_literal_node) => {
                self.bind_array_literal(array_literal_node, expected, location, compiler, id)
            }
            SyntaxNodeKind::ExpressionStatement(expression_statement_node) => self
                .bind_expression_statement(
                    expression_statement_node,
                    expected,
                    location,
                    compiler,
                    id,
                ),
            SyntaxNodeKind::BlockExpression(block_expression_node) => {
                self.bind_block_expression(block_expression_node, expected, location, compiler, id)
            }
            SyntaxNodeKind::FunctionDeclaration(function_declaration_node) => self
                .bind_function_declaration(
                    function_declaration_node,
                    expected,
                    location,
                    compiler,
                    id,
                ),
            SyntaxNodeKind::StructDeclaration(struct_declaration_node) => self
                .bind_struct_declaration(struct_declaration_node, expected, location, compiler, id),
            SyntaxNodeKind::ImplBlock(impl_block_node) => {
                self.bind_impl_block(impl_block_node, expected, location, compiler, id)
            }
            SyntaxNodeKind::FunctionCall(function_call_node) => {
                self.bind_function_call(function_call_node, expected, location, compiler, id)
            }
            SyntaxNodeKind::FieldAccess(field_access_node) => {
                self.bind_field_access(field_access_node, expected, location, compiler, id)
            }
            SyntaxNodeKind::AssignmentStatement(assignment_statement_node) => self
                .bind_assignment_statement(
                    assignment_statement_node,
                    expected,
                    location,
                    compiler,
                    id,
                ),
            SyntaxNodeKind::Conversion(_conversion_node) => {
                todo!("These are not created yet during parsing")
            }
            SyntaxNodeKind::StructLiteral(struct_literal_node) => {
                self.bind_struct_literal(struct_literal_node, expected, location, compiler, id)
            }
        };
        unsafe {
            compiler.nodes.set(id, node);
        }
        self.bind_conversion(id, expected, compiler, ConversionKind::Implicit)
    }

    fn bind_node(
        &mut self,
        node: SyntaxNode<Parsed>,
        expected: TypeId,
        compiler: &mut Compiler,
    ) -> BoundId {
        let id = unsafe { compiler.nodes.prepare_id() };
        self.bind_node_with_id(node, expected, compiler, id)
    }

    fn bind_literal(
        &mut self,
        token: Token,
        expected: TypeId,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        match token.kind {
            TokenKind::Error => SyntaxNode::<Bound>::error(token.location(), id),
            TokenKind::Integer => {
                let lexeme = &compiler[token.location()];
                let (value, type_) = match expected {
                    TypeId::UNSIGNED_INTEGER_8 => {
                        let expected = TypeId::UNSIGNED_INTEGER_8;
                        let value = lexeme
                            .parse::<u8>()
                            .map(|v| Value::UnsignedInteger8(v))
                            .unwrap_or_else(|_| {
                                compiler.diagnostics.report_cannot_parse_integer_literal_to(
                                    token.location(),
                                    expected,
                                );
                                Value::Error
                            });
                        (value, expected)
                    }
                    TypeId::UNSIGNED_INTEGER_16 => {
                        let expected = TypeId::UNSIGNED_INTEGER_16;
                        let value = lexeme
                            .parse::<u16>()
                            .map(|v| Value::UnsignedInteger16(v))
                            .unwrap_or_else(|_| {
                                compiler.diagnostics.report_cannot_parse_integer_literal_to(
                                    token.location(),
                                    expected,
                                );
                                Value::Error
                            });
                        (value, expected)
                    }
                    TypeId::UNSIGNED_INTEGER_32 | TypeId::UNKNOWN | _ => {
                        let expected = TypeId::UNSIGNED_INTEGER_32;
                        let value = lexeme
                            .parse::<u32>()
                            .map(|v| Value::UnsignedInteger32(v))
                            .unwrap_or_else(|_| {
                                compiler.diagnostics.report_cannot_parse_integer_literal_to(
                                    token.location(),
                                    expected,
                                );
                                Value::Error
                            });
                        (value, expected)
                    }
                };
                SyntaxNode::<Bound>::literal(token, value, type_, id)
            }
            TokenKind::FalseKeyword | TokenKind::TrueKeyword => {
                assert!(expected == TypeId::UNKNOWN || expected == TypeId::BOOL);
                let value = token.kind == TokenKind::TrueKeyword;
                SyntaxNode::<Bound>::literal(token, Value::Bool(value), TypeId::BOOL, id)
            }
            unexpected => todo!("Unexpected literal {unexpected:#?}!"),
        }
    }

    fn bind_conversion(
        &mut self,
        base_id: BoundId,
        expected: TypeId,
        compiler: &mut Compiler,
        conversion_kind: ConversionKind,
    ) -> BoundId {
        let base_type = compiler.nodes.type_of(base_id);
        let location = compiler.nodes.location_of(base_id);
        if base_type == expected
            || base_type == TypeId::ERROR
            || expected == TypeId::UNKNOWN
            || expected == TypeId::ERROR
        {
            return base_id;
        }
        let id = unsafe { compiler.nodes.prepare_id() };
        let node = if conversion_kind.convert(base_type, expected, &mut compiler.types) {
            SyntaxNode::<Bound>::conversion(location, base_id, expected, conversion_kind, id)
        } else {
            compiler
                .diagnostics
                .report_cannot_convert(location, base_type, expected);
            SyntaxNode::<Bound>::error(location, id)
        };
        unsafe { compiler.nodes.set(id, node) };
        id
    }

    fn bind_identifier(
        &mut self,
        token: Token,
        expected: TypeId,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        let name = compiler.intern_location(token.location());
        if let Some(variable) = self.look_up_variable_by_name(name) {
            SyntaxNode::<Bound>::variable(
                token.location(),
                variable,
                self.look_up_constant(variable.id),
                id,
            )
        } else {
            compiler
                .diagnostics
                .report_cannot_find_variable_by_name(token.location());
            SyntaxNode::<Bound>::error(token.location(), id)
        }
    }

    fn bind_const_declaration(
        &mut self,
        const_declaration_node: ConstDeclarationNode<Parsed>,
        expected: TypeId,
        location: Location,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        assert_eq!(expected, TypeId::VOID);
        let expression_location = const_declaration_node.expr.location();
        let expression = self.bind_node(*const_declaration_node.expr, TypeId::UNKNOWN, compiler);
        // let variable = &compiler[];
        let variable_location = const_declaration_node.identifier.location();
        let variable = compiler.intern_location(variable_location);

        let type_ = compiler.nodes.type_of(expression);
        let Some(variable) = self.register_variable(variable_location, variable, type_) else {
            let previous = self
                .look_up_variable_by_name(variable)
                .expect("should exist at this point");
            compiler.diagnostics.report_cannot_redeclare_variable(
                const_declaration_node.identifier.location(),
                previous.location(),
            );
            dbg!("Failed registering variable");
            return SyntaxNode::<Bound>::error(const_declaration_node.identifier.location(), id);
        };
        let value = const_evaluator::evaluate(expression, compiler).unwrap_or_else(|| {
            compiler
                .diagnostics
                .report_non_const_value_in_const_declaration(expression_location);
            Value::Error
        });
        self.register_constant(variable, value.clone());
        SyntaxNode::<Bound>::const_declaration(location, variable, value, id)
    }

    fn register_variable(
        &mut self,
        location: Location,
        variable_name: StringId,
        type_: TypeId,
    ) -> Option<VariableId> {
        self.variables
            .register(location, &self.namespaces, variable_name, type_, true)
    }

    fn register_constant(&mut self, variable: VariableId, value: Value) {
        self.constants.insert(variable, value);
    }

    fn bind_program(
        &mut self,
        program_node: ProgramNode<Parsed>,
        expected: TypeId,
        location: Location,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        assert_eq!(expected, TypeId::VOID);
        let statements: Vec<BoundId> = program_node
            .top_level_statements
            .into_iter()
            .map(|s| self.bind_node(s, TypeId::VOID, compiler))
            .collect();
        SyntaxNode::<Bound>::program(statements, location, id)
    }

    fn bind_binary(
        &mut self,
        location: Location,
        binary_node: BinaryNode<Parsed>,
        expected: TypeId,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        assert!(expected == TypeId::UNSIGNED_INTEGER_32 || expected == TypeId::UNKNOWN);
        let lhs = self.bind_node(*binary_node.lhs, TypeId::UNKNOWN, compiler);
        let rhs = self.bind_node(*binary_node.rhs, TypeId::UNKNOWN, compiler);
        let op = match binary_node.op.kind {
            TokenKind::Plus => BoundBinaryOperator::Addition,
            TokenKind::Minus => BoundBinaryOperator::Subtraction,
            TokenKind::Asterisk => BoundBinaryOperator::Multiplication,
            TokenKind::Slash => BoundBinaryOperator::Division,
            TokenKind::Percent => BoundBinaryOperator::Modulo,
            _ => unreachable!("Compiler error!"),
        };
        let (lhs_type, rhs_type, return_type) = op.resolve_types(
            compiler.nodes.type_of(lhs),
            compiler.nodes.type_of(rhs),
            &mut compiler.types,
        );
        let lhs = self.bind_conversion(lhs, lhs_type, compiler, ConversionKind::Implicit);
        let rhs = self.bind_conversion(rhs, rhs_type, compiler, ConversionKind::Implicit);
        if lhs_type != TypeId::ERROR && rhs_type != TypeId::ERROR && return_type == TypeId::ERROR {
            compiler
                .diagnostics
                .report_invalid_binary_operation(location, lhs_type, op, rhs_type)
        }
        SyntaxNode::<Bound>::binary(location, lhs, op, rhs, return_type, id)
    }

    fn look_up_variable_by_name(&self, name: StringId) -> Option<&VariableDeclaration> {
        self.variables.find_by_name(name)
    }

    fn look_up_constant(&self, id: VariableId) -> Option<Value> {
        self.constants.get(&id).cloned()
    }

    fn bind_array_literal(
        &mut self,
        array_literal_node: ArrayLiteralNode<Parsed>,
        expected: TypeId,
        location: Location,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        let entries: Vec<BoundId> = array_literal_node
            .entries
            .into_iter()
            .map(|e|
            // TODO: Use correct expected Type here!
            self.bind_node(e, TypeId::UNKNOWN, compiler))
            .collect();
        let inner_type = entries
            .iter()
            .map(|e| compiler.nodes.type_of(*e))
            .fold(TypeId::UNKNOWN, |acc, cur| acc);
        let length = entries.len();
        let type_ = compiler.types.register(Type::Array(inner_type, length));
        assert!(type_ == expected || expected == TypeId::UNKNOWN);
        SyntaxNode::<Bound>::array_literal(entries, type_, location, id)
    }

    fn bind_expression_statement(
        &mut self,
        expression_statement_node: ExpressionStatementNode<Parsed>,
        expected: TypeId,
        location: Location,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        let expression = self.bind_node(
            *expression_statement_node.expression,
            TypeId::UNKNOWN,
            compiler,
        );
        let has_semicolon = expression_statement_node.semicolon.is_some();
        let type_ = if has_semicolon {
            TypeId::VOID
        } else {
            compiler.nodes.type_of(expression)
        };
        SyntaxNode::<Bound>::expression_statement(expression, location, has_semicolon, id, type_)
    }

    fn bind_block_expression(
        &mut self,
        block_expression_node: BlockExpressionNode<Parsed>,
        expected: TypeId,
        location: Location,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        let statements: Vec<BoundId> = block_expression_node
            .body
            .into_iter()
            .map(|e| self.bind_node(e, TypeId::UNKNOWN, compiler))
            .collect();
        let type_ = statements
            .last()
            .map(|s| compiler.nodes.type_of(*s))
            .unwrap_or(TypeId::VOID);
        SyntaxNode::<Bound>::block_expression(statements, location, type_, id)
    }

    fn bind_function_declaration(
        &mut self,
        function_declaration_node: FunctionDeclarationNode<Parsed>,
        expected: TypeId,
        location: Location,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        let is_comp = function_declaration_node.comp_keyword.is_some();
        let identifier_location = function_declaration_node.identifier.location();
        let identifier = compiler.intern_location(identifier_location);
        let generic_parameters: Vec<(Location, StringId, TypeId)> = function_declaration_node
            .head
            .generics
            .as_ref()
            .map(|g| {
                g.parameters
                    .iter()
                    .filter_map(|p| {
                        if let Some((_, t)) = &p.type_ {
                            Some((
                                p.location,
                                compiler.intern_location(p.identifier.location()),
                                self.bind_type_identifier(t.clone(), compiler),
                            ))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        // generic_parameters
        //     .clone()
        //     .into_iter()
        //     .map(|(l, n, t)| {
        //         let n = self.register_variable(n, t).expect("No failure!");
        //         ParameterNode::<Bound>::new(l, n, t)
        //     })
        //     .collect::<Vec<ParameterNode<Bound>>>();

        let body = unsafe { compiler.nodes.prepare_id() };
        let generics = function_declaration_node.head.generics.map(|g| {
            let parameters = g
                .parameters
                .into_iter()
                .map(|p| self.bind_generic_parameter(p, compiler, body))
                .collect();
            GenericParameterHeaderNode::<Bound>::new(g.location, parameters)
        });

        let parameters_bound: Vec<_> = function_declaration_node
            .head
            .parameters
            .into_iter()
            .map(|p| self.bind_parameter(p, compiler))
            .collect();
        let parameters = self.creates_scope(compiler, |b, c| {
            // let generic_parameters = generic_parameters
            //     .into_iter()
            //     .map(|(l, n, t)| {
            //         let n = b.register_variable(n, t).expect("No failure!");
            //         ParameterNode::<Bound>::new(l, n, t)
            //     })
            //     .collect::<Vec<ParameterNode<Bound>>>();
            let parameters = parameters_bound
                .iter()
                .map(|&(l, n, t)| {
                    let n = b.register_variable(l, n, t).expect("No failure!");
                    ParameterNode::<Bound>::new(l, n, t)
                })
                .collect();
            b.bind_node_with_id(*function_declaration_node.body, TypeId::UNKNOWN, c, body);
            parameters
        });
        let return_type = function_declaration_node
            .head
            .return_type
            .map(|(_, t)| self.bind_type_identifier(t, compiler))
            .unwrap_or(TypeId::VOID);
        let type_ = Type::FunctionType(
            parameters_bound.iter().map(|(.., t)| *t).collect(),
            return_type,
        );
        let type_ = compiler.types.register(type_);
        let identifier = self
            .register_variable(identifier_location, identifier, type_)
            .expect("no duplicate!");

        self.register_constant(identifier, Value::CompileTimeFunction(body));
        // let generics = GenericParameterHeaderNode::new(generic_parameters, generi;
        SyntaxNode::<Bound>::function_declaration(
            generics,
            identifier,
            location,
            parameters,
            body,
            return_type,
            is_comp,
            id,
        )
    }

    fn bind_type_identifier(&mut self, t: TypeIdentifier, compiler: &mut Compiler) -> TypeId {
        match t {
            TypeIdentifier::Error(_) => TypeId::ERROR,
            TypeIdentifier::Named(name) => {
                let name = compiler.intern_location(name.location());
                match compiler.types.find_by_name(name) {
                    Some(it) => it,
                    None => {
                        compiler
                            .diagnostics
                            .report_cannot_find_type(t.location(), name);
                        TypeId::ERROR
                    }
                }
            }
            TypeIdentifier::Array(array_type_identifier) => {
                let length_location = array_type_identifier.length.location();
                let length = self.bind_node(
                    *array_type_identifier.length,
                    TypeId::UNSIGNED_INTEGER_16,
                    compiler,
                );
                let inner = self.bind_type_identifier(*array_type_identifier.type_, compiler);
                let Some(length) = const_evaluator::evaluate(length, compiler) else {
                    compiler
                        .diagnostics
                        .report_non_const_value_in_type(length_location);
                    return TypeId::ERROR;
                };
                let Some(length) = length.as_usize() else {
                    return TypeId::ERROR;
                };
                compiler.types.register(Type::Array(inner, length as _))
            }
        }
    }

    fn bind_parameter(
        &mut self,
        p: ParameterNode<Parsed>,
        compiler: &mut Compiler,
    ) -> (Location, StringId, TypeId) {
        let location = p.location;
        let name = compiler.intern_location(p.identifier.location());
        let type_ = self.bind_type_identifier(p.type_, compiler);
        (location, name, type_)
    }

    fn creates_scope<U>(
        &mut self,
        compiler: &mut Compiler,
        c: impl FnOnce(&mut Binder, &mut Compiler) -> U,
    ) -> U {
        self.variables.start_scope();
        let result = c(self, compiler);
        self.variables.end_scope();
        result
    }

    fn bind_function_call(
        &mut self,
        function_call_node: FunctionCallNode<Parsed>,
        expected: TypeId,
        location: Location,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        let base_location = function_call_node.base.location();
        let base = self.bind_node(*function_call_node.base, TypeId::UNKNOWN, compiler);
        let arguments = function_call_node
            .arguments
            .into_iter()
            .map(|a| self.bind_node(a, TypeId::UNKNOWN, compiler))
            .collect();
        let type_ = compiler.nodes.type_of(base);
        if type_ == TypeId::ERROR {
            return SyntaxNode::<Bound>::error(location, id);
        }
        let Some(type_) = compiler.types.return_type_of(type_) else {
            compiler
                .diagnostics
                .report_invalid_function_type(base_location, type_);
            return SyntaxNode::<Bound>::error(location, id);
        };
        SyntaxNode::<Bound>::function_call(location, base, arguments, type_, id)
    }

    fn bind_assignment_statement(
        &mut self,
        assignment_statement_node: AssignmentStatementNode<Parsed>,
        expected: TypeId,
        location: Location,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        let lhs = self.bind_node(*assignment_statement_node.lhs, expected, compiler);
        let value = self.bind_node(*assignment_statement_node.value, expected, compiler);
        SyntaxNode::<Bound>::assignment_statement(lhs, value, location, id)
    }

    fn bind_generic_parameter(
        &mut self,
        p: GenericParameterNode<Parsed>,
        compiler: &mut Compiler,
        body: BoundId,
    ) -> GenericParameterNode<Bound> {
        let is_out = p.out.is_some();
        let identifier = compiler.intern_location(p.identifier.location());
        let type_ = p.type_.map(|(_, t)| self.bind_type_identifier(t, compiler));
        let identifier_type = type_.unwrap_or(TypeId::TYPE);
        let identifier = self
            .register_variable(p.identifier.location(), identifier, identifier_type)
            .expect("Success");
        if is_out {
            assert!(type_.is_some());
            self.register_constant(identifier, Value::DependentOn(body));
        }
        GenericParameterNode::<Bound>::new(is_out, identifier, type_, p.location)
    }

    fn bind_struct_declaration(
        &mut self,
        struct_declaration_node: StructDeclarationNode<Parsed>,
        expected: TypeId,
        location: Location,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        let name = compiler.intern_location(struct_declaration_node.identifier.location());
        let fields: Vec<(Location, StringId, TypeId)> = struct_declaration_node
            .fields
            .into_iter()
            .map(|f| self.bind_parameter(f, compiler))
            .collect();
        let Some(identifier) = self.register_variable(
            struct_declaration_node.identifier.location(),
            name,
            TypeId::TYPE,
        ) else {
            todo!("Error handling")
        };

        // let fields_bound = fields
        //     .iter()
        //     .map(|f| ParameterNode::<Bound>::new(f.0, f.1, f.2))
        //     .collect();
        let layout = StructLayout::from_fields(&fields, &compiler.types);
        let type_ = compiler.types.register(Type::Struct(StructType {
            name,
            identifier,
            fields,
            layout,
        }));
        self.register_constant(identifier, Value::Type(type_));

        // TODO: Keep struct declaration similarly to const or function declaration!
        unsafe { SyntaxNode::empty(id) }
        // SyntaxNode::<Bound>::struct_declaration(location, identifier, fields_bound, type_, id)
    }

    fn bind_struct_literal(
        &mut self,
        struct_literal_node: StructLiteralNode<Parsed>,
        expected: TypeId,
        location: Location,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        let identifier = compiler.intern_location(struct_literal_node.identifier.location());
        let identifier = self
            .look_up_variable_by_name(identifier)
            .expect("error handling")
            .id;
        let type_ = self
            .look_up_constant(identifier)
            .expect("should be constant")
            .as_type()
            .expect("should be type id");
        let struct_type = compiler
            .types
            .as_struct_type(type_)
            .expect("should be struct")
            .clone();
        let fields: Vec<_> = struct_literal_node
            .fields
            .into_iter()
            .map(|f| self.bind_field_initializer(f, &struct_type, compiler))
            .collect();
        // TODO: Ensure all fields are initialized!
        SyntaxNode::<Bound>::struct_literal(location, identifier, fields, type_, id)
    }

    fn bind_field_initializer(
        &mut self,
        f: FieldInitilizationNode<Parsed>,
        struct_type: &StructType,
        compiler: &mut Compiler,
    ) -> FieldInitilizationNode<Bound> {
        let identifier = compiler.intern_location(f.identifier.location());
        let expected = struct_type
            .get_field_type_by_name(identifier)
            .unwrap_or(TypeId::ERROR);
        let expression = self.bind_node(*f.expression, expected, compiler);
        FieldInitilizationNode::<Bound>::new(f.location, identifier, expression)
    }

    fn bind_field_access(
        &mut self,
        field_access_node: FieldAccessNode<Parsed>,
        expected: TypeId,
        location: Location,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        let base = self.bind_node(*field_access_node.base, TypeId::UNKNOWN, compiler);
        let field_identifier = compiler.intern_location(field_access_node.field.location());
        let base_type = compiler.nodes.type_of(base);
        match compiler.types.field_type(base_type, field_identifier) {
            Some(type_) => {
                SyntaxNode::<Bound>::field_access(location, base, field_identifier, type_, id)
            }
            None => {
                if base_type != TypeId::ERROR {
                    compiler
                        .diagnostics
                        .report_cannot_find_field_with_this_name(
                            location,
                            base_type,
                            field_identifier,
                        );
                }
                SyntaxNode::<Bound>::error(location, id)
            }
        }
    }

    fn bind_impl_block(
        &mut self,
        impl_block_node: ImplBlockNode<Parsed>,
        expected: TypeId,
        location: Location,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        let identifier = compiler.intern_location(impl_block_node.identifier.location());
        let struct_type = self.look_up_variable_by_name(identifier).unwrap();
        let type_ = self
            .look_up_constant(struct_type.id)
            .unwrap()
            .as_type()
            .unwrap();
        let struct_type = compiler.types.as_struct_type(type_).unwrap();
        self.push_namespace(identifier);
        let functions: Vec<BoundId> = impl_block_node
            .body
            .into_iter()
            .map(|f| self.bind_node(f, TypeId::VOID, compiler))
            .collect();
        for f in functions {
            let f = compiler.nodes[f]
                .kind
                .as_function_declaration()
                .expect("Only supported for now!");
            dbg!(&f.identifier, f.body);
        }
        self.pop_namespace();
        dbg!(&self.constants);
        todo!()
    }

    fn push_namespace(&mut self, namespace: StringId) {
        self.namespaces.push(namespace);
    }

    fn pop_namespace(&mut self) {
        self.namespaces.pop();
    }
}
