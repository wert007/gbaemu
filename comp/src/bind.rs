pub mod conversion;

use std::collections::HashMap;

use crate::{
    BoundId, Compiler, HasLocation, Location, SourceTextId, StringId,
    bind::conversion::ConversionKind,
    const_evaluator,
    lexer::{Token, TokenKind},
    parser::Parser,
    pattern::Pattern,
    syntax_tree::*,
    traits::{Trait, TraitFunction},
    typing::{EnumType, FunctionType, StructLayout, StructType, Type, TypeId, Types},
    value::Value,
    variables::VariableId,
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
        expected: TypeId,
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
pub struct Binder {
    file: SourceTextId,
    constants: HashMap<VariableId, Value>,
    this_type: Option<TypeId>,
}

impl Binder {
    pub fn new(file: SourceTextId) -> Binder {
        Self {
            file,
            constants: HashMap::new(),
            this_type: None,
        }
    }

    pub fn bind(mut self, compiler: &mut Compiler) -> BoundId {
        let tree = Parser::new(self.file).parse(compiler);
        crate::debug::dump_parse_tree(&tree, compiler);
        let node = self.bind_node(tree.node, TypeId::VOID, compiler);
        crate::debug::dump_bound_tree(node, &compiler);
        compiler.types.dump(&compiler.strings);
        compiler.variables.dump(&compiler.strings, &compiler.types);
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
            SyntaxNodeKind::Error => SyntaxNode::<Bound>::error(node.location(), id),
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
                self.bind_identifier(identifier, compiler, id)
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
            SyntaxNodeKind::FunctionDeclaration(function_declaration_node) => {
                self.bind_function_declaration(function_declaration_node, location, compiler, id)
            }
            SyntaxNodeKind::StructDeclaration(struct_declaration_node) => {
                self.bind_struct_declaration(struct_declaration_node, location, compiler, id)
            }
            SyntaxNodeKind::EnumDeclaration(enum_declaration_node) => {
                self.bind_enum_declaration(enum_declaration_node, location, compiler, id)
            }
            SyntaxNodeKind::ImplBlock(impl_block_node) => {
                self.bind_impl_block(impl_block_node, expected, location, compiler, id)
            }
            SyntaxNodeKind::FunctionCall(function_call_node) => {
                self.bind_function_call(function_call_node, location, compiler, id)
            }
            SyntaxNodeKind::FieldAccess(field_access_node) => {
                self.bind_field_access(field_access_node, location, compiler, id)
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
                self.bind_struct_literal(struct_literal_node, location, compiler, id)
            }
            SyntaxNodeKind::PartialCapture(_partial_capture_node) => unreachable!(),
            SyntaxNodeKind::MatchExpression(match_expression) => {
                self.bind_match_expression(match_expression, expected, location, compiler, id)
            }
        };
        unsafe {
            compiler.nodes.set(id, node.clone());
        }
        let base_id = unsafe { compiler.nodes.prepare_id() };
        unsafe {
            compiler.nodes.silent_set(base_id, node);
        }
        let mut did_conversion = false;
        self.bind_conversion_with_id(
            base_id,
            expected,
            compiler,
            ConversionKind::Implicit,
            id,
            &mut did_conversion,
        );
        if !did_conversion {
            unsafe {
                compiler.nodes.free_last();
            }
        }
        id
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
                let (value, type_) = match expected {
                    TypeId::UNSIGNED_INTEGER_8 => {
                        let lexeme = &compiler[token.location()];
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
                        let lexeme = &compiler[token.location()];
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
                    TypeId::UNSIGNED_INTEGER_32 => {
                        let lexeme = &compiler[token.location()];
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
                    TypeId::UNKNOWN | _ => {
                        let lexeme = &compiler[token.location()];
                        let value = lexeme.parse::<u32>().unwrap_or(u32::MAX);
                        let expected = compiler.types.register(Type::IntegerLiteral(value as _));
                        let lexeme = &compiler[token.location()];
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
        if base_type == expected
            || base_type == TypeId::ERROR
            || expected == TypeId::UNKNOWN
            || expected == TypeId::ERROR
            || (compiler
                .types
                .as_inner_array_type(base_type)
                .is_some_and(|b| compiler.types.as_inner_array_type(expected) == Some(b)))
        {
            return base_id;
        }
        let id = unsafe { compiler.nodes.prepare_id() };
        let mut did_conversion = false;
        self.bind_conversion_with_id(
            base_id,
            expected,
            compiler,
            conversion_kind,
            id,
            &mut did_conversion,
        )
    }

    fn bind_conversion_with_id(
        &mut self,
        base_id: BoundId,
        expected: TypeId,
        compiler: &mut Compiler,
        conversion_kind: ConversionKind,
        id: BoundId,
        did_conversion: &mut bool,
    ) -> BoundId {
        *did_conversion = true;
        let base_type = compiler.nodes.type_of(base_id);
        let location = compiler.nodes.location_of(base_id);
        if base_type == expected
            || base_type == TypeId::ERROR
            || expected == TypeId::UNKNOWN
            || expected == TypeId::ERROR
        {
            *did_conversion = false;
            return base_id;
        }
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
        namespaced_identifier: NamespacedIdentifier,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        let location = namespaced_identifier.location();
        let (namespaces, identifier) =
            intern_namespaced_identifier(namespaced_identifier, compiler);
        if let Some(variable) = compiler.variables.find_by_name(&namespaces, identifier) {
            SyntaxNode::<Bound>::variable(
                location,
                variable,
                self.look_up_constant(variable.id),
                id,
            )
        } else {
            compiler.diagnostics.report_cannot_find_variable_by_name(
                location,
                &namespaces,
                identifier,
            );
            SyntaxNode::<Bound>::error(location, id)
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
        let expression = self.remove_integer_literal_type(expression, compiler);
        // let variable = &compiler[];
        let variable_location = const_declaration_node.identifier.location();
        let variable = compiler.intern_location(variable_location);

        let type_ = compiler.nodes.type_of(expression);
        let Some(variable) =
            self.register_global_variable(compiler, variable_location, variable, type_)
        else {
            let previous = compiler
                .variables
                .find_by_name(&[], variable)
                .expect("should exist at this point");
            compiler.diagnostics.report_cannot_redeclare_variable(
                const_declaration_node.identifier.location(),
                previous.location(),
            );
            return SyntaxNode::<Bound>::error(const_declaration_node.identifier.location(), id);
        };
        let value = const_evaluator::evaluate(expression, compiler, Some(&self.constants))
            .unwrap_or_else(|| {
                compiler
                    .diagnostics
                    .report_non_const_value_in_const_declaration(expression_location);
                Value::Error
            });
        self.register_constant(variable, value.clone());
        SyntaxNode::<Bound>::const_declaration(location, variable, expression, id)
    }

    fn register_variable(
        &self,
        compiler: &mut Compiler,
        location: Location,
        variable_name: StringId,
        type_: TypeId,
    ) -> Option<VariableId> {
        compiler
            .variables
            .register(location, variable_name, type_, true)
    }

    fn register_global_variable(
        &self,
        compiler: &mut Compiler,
        location: Location,
        variable_name: StringId,
        type_: TypeId,
    ) -> Option<VariableId> {
        compiler
            .variables
            .register(location, variable_name, type_, false)
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
            expected,
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
        let expected_inner = compiler
            .types
            .as_inner_array_type(expected)
            .unwrap_or(TypeId::UNKNOWN);
        let entries: Vec<BoundId> = array_literal_node
            .entries
            .into_iter()
            .map(|e| {
                let entry = self.bind_node(e, expected_inner, compiler);
                self.remove_integer_literal_type(entry, compiler)
            })
            .collect();
        let inner_type =
            entries
                .iter()
                .map(|e| compiler.nodes.type_of(*e))
                .fold(TypeId::UNKNOWN, |acc, cur| {
                    if acc == TypeId::UNKNOWN {
                        cur
                    } else if ConversionKind::Implicit.convert(acc, cur, &mut compiler.types) {
                        cur
                    } else if ConversionKind::Implicit.convert(cur, acc, &mut compiler.types) {
                        acc
                    } else {
                        acc
                    }
                });
        let length = entries.len();
        let entries = entries
            .into_iter()
            .map(|e| self.bind_conversion(e, inner_type, compiler, ConversionKind::Implicit))
            .collect();
        let type_ = compiler.types.register(Type::Array(inner_type, length));
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
        let expected = if expected == TypeId::VOID {
            TypeId::UNKNOWN
        } else {
            expected
        };
        let expression = self.bind_node(*expression_statement_node.expression, expected, compiler);
        let inner = compiler.nodes.type_of(expression);
        let has_semicolon = expression_statement_node.semicolon.is_some();
        if expected != TypeId::ERROR
            && inner != TypeId::ERROR
            && expected != TypeId::UNKNOWN
            && has_semicolon
            && ConversionKind::Implicit.convert(inner, expected, &mut compiler.types)
        {
            compiler.diagnostics.hint_remove_semicolon(
                expression_statement_node.semicolon.unwrap().location(),
                expected,
            );
        }
        let type_ = if has_semicolon { TypeId::VOID } else { inner };
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
        let body_len = block_expression_node.body.len();
        let expected_type = |i: usize| {
            if i == body_len - 1 {
                expected
            } else {
                TypeId::UNKNOWN
            }
        };
        let statements: Vec<BoundId> = block_expression_node
            .body
            .into_iter()
            .enumerate()
            .map(|(i, e)| self.bind_node(e, expected_type(i), compiler))
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
        location: Location,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        let is_comp = function_declaration_node.comp_keyword.is_some();
        let identifier_location = function_declaration_node.identifier.location();
        let identifier = compiler.intern_location(identifier_location);
        compiler.variables.push_namespace(identifier);
        let _generic_parameters: Vec<(Location, StringId, TypeId)> = function_declaration_node
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
                                self.bind_type_identifier(t.clone(), compiler, false),
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

        let return_type = function_declaration_node
            .head
            .return_type
            .as_ref()
            .map(|(_, t)| self.bind_type_identifier(t.clone(), compiler, true))
            .unwrap_or(TypeId::VOID);

        let parameters: Vec<ParameterNode<Bound>> = self.creates_scope(compiler, |b, c| {
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
                    let n = b.register_variable(c, l, n, t).expect("No failure!");
                    ParameterNode::<Bound>::new(l, n, t)
                })
                .collect();
            b.bind_node_with_id(*function_declaration_node.body, return_type, c, body);
            parameters
        });

        let return_type = function_declaration_node
            .head
            .return_type
            .map(|(_, t)| self.bind_type_identifier(t, compiler, false))
            .unwrap_or(TypeId::VOID);

        let body = self.bind_conversion(body, return_type, compiler, ConversionKind::Implicit);
        let type_ = Type::FunctionType(FunctionType {
            identifier,
            parameters: parameters.iter().map(|p| p.identifier).collect(),
            parameter_types: { parameters_bound.iter().map(|(.., t)| *t).collect() },
            return_type,
        });
        let type_ = compiler.types.register(type_);
        compiler.variables.pop_namespace();

        let Some(identifier) =
            self.register_global_variable(compiler, identifier_location, identifier, type_)
        else {
            compiler
                .diagnostics
                // TODO find actual previous declaration!
                .report_cannot_redeclare_variable(location, location);
            return SyntaxNode::<Bound>::error(location, id);
        };

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

    fn bind_type_identifier(
        &mut self,
        t: TypeIdentifier,
        compiler: &mut Compiler,
        silent: bool,
    ) -> TypeId {
        let location = t.location();
        match t {
            TypeIdentifier::Error(_) => TypeId::ERROR,
            TypeIdentifier::Named(named) => {
                let is_reference = named.ampersand.is_some();
                let name = compiler.intern_location(named.identifier.location());
                let type_ = match compiler.types.find_by_name(name) {
                    Some(it) => it,
                    None => {
                        if silent {
                            TypeId::UNKNOWN
                        } else {
                            compiler
                                .diagnostics
                                .report_cannot_find_type(named.location(), name);
                            TypeId::ERROR
                        }
                    }
                };
                let type_ = if is_reference {
                    compiler.types.register(Type::Reference(type_))
                } else {
                    type_
                };
                type_
            }
            TypeIdentifier::Array(array_type_identifier) => {
                let length_location = array_type_identifier.length.location();
                let length = self.bind_node(
                    *array_type_identifier.length,
                    TypeId::UNSIGNED_INTEGER_16,
                    compiler,
                );
                let inner =
                    self.bind_type_identifier(*array_type_identifier.type_, compiler, silent);
                let Some(length) = const_evaluator::evaluate(length, compiler, None) else {
                    return if silent {
                        compiler.types.register(Type::ArrayUnknownLength(inner))
                    } else {
                        compiler
                            .diagnostics
                            .report_non_const_value_in_type(length_location);
                        TypeId::ERROR
                    };
                };
                let Some(length) = length.as_usize() else {
                    return if silent {
                        compiler.types.register(Type::ArrayUnknownLength(inner))
                    } else {
                        TypeId::ERROR
                    };
                };
                compiler.types.register(Type::Array(inner, length as _))
            }
            TypeIdentifier::This(this_type) => {
                let Some(type_) = self.this_type else {
                    return if silent {
                        TypeId::UNKNOWN
                    } else {
                        compiler
                            .diagnostics
                            .report_this_can_only_be_used_in_impl_block(location);
                        TypeId::ERROR
                    };
                };
                if this_type.ampersand.is_some() {
                    compiler.types.register(Type::Reference(type_))
                } else {
                    type_
                }
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
        let type_ = self.bind_type_identifier(p.type_, compiler, false);
        (location, name, type_)
    }

    fn creates_scope<U>(
        &mut self,
        compiler: &mut Compiler,
        c: impl FnOnce(&mut Binder, &mut Compiler) -> U,
    ) -> U {
        compiler.variables.start_scope();
        let result = c(self, compiler);
        compiler.variables.end_scope();
        result
    }

    fn bind_function_call(
        &mut self,
        function_call_node: FunctionCallNode<Parsed>,
        location: Location,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        let base_location = function_call_node.base.location();
        let base = self.bind_node(*function_call_node.base, TypeId::UNKNOWN, compiler);

        // .collect();
        let type_ = compiler.nodes.type_of(base);
        if type_ == TypeId::ERROR {
            return SyntaxNode::<Bound>::error(location, id);
        }
        let Some(function_type) = compiler.types.as_function_type(type_).cloned() else {
            compiler
                .diagnostics
                .report_invalid_function_type(base_location, type_);
            return SyntaxNode::<Bound>::error(location, id);
        };
        let type_ = function_type.return_type;
        let arguments = function_call_node
            .arguments
            .into_iter()
            .zip(&function_type.parameter_types)
            .map(|(a, t)| self.bind_node(a, *t, compiler))
            .collect();
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
        let type_ = p
            .type_
            .map(|(_, t)| self.bind_type_identifier(t, compiler, false));
        let identifier_type = type_.unwrap_or(TypeId::TYPE);
        let identifier = self
            .register_variable(
                compiler,
                p.identifier.location(),
                identifier,
                identifier_type,
            )
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
        let Some(identifier) = self.register_global_variable(
            compiler,
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
            identifier,
            fields,
            layout,
        }));
        self.register_constant(identifier, Value::Type(type_));

        // TODO: Keep struct declaration similarly to const or function declaration!
        unsafe { SyntaxNode::empty(id) }
        // SyntaxNode::<Bound>::struct_declaration(location, identifier, fields_bound, type_, id)
    }

    fn bind_enum_declaration(
        &mut self,
        enum_declaration_node: EnumDeclarationNode<Parsed>,
        location: Location,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        let name = compiler.intern_location(enum_declaration_node.identifier.location());
        let type_ = unsafe { compiler.types.reserve() };
        compiler.variables.push_namespace(name);
        let variants: Vec<VariableId> = enum_declaration_node
            .variants
            .into_iter()
            .enumerate()
            .map(|(i, f)| self.bind_variant(f, i as _, type_, compiler))
            .collect();
        compiler.variables.pop_namespace();
        let Some(identifier) = self.register_global_variable(
            compiler,
            enum_declaration_node.identifier.location(),
            name,
            TypeId::TYPE,
        ) else {
            todo!("Error handling")
        };

        // let fields_bound = fields
        //     .iter()
        //     .map(|f| ParameterNode::<Bound>::new(f.0, f.1, f.2))
        //     .collect();
        let layout = StructLayout::from_variants(&variants, &compiler.types, &compiler.variables);
        unsafe {
            compiler.types.set(
                type_,
                Type::Enum(EnumType {
                    identifier,
                    variants,
                    layout,
                }),
            )
        };
        self.register_constant(identifier, Value::Type(type_));

        // TODO: Keep enum declaration similarly to const or function declaration!
        unsafe { SyntaxNode::empty(id) }
        // SyntaxNode::<Bound>::struct_declaration(location, identifier, fields_bound, type_, id)
    }

    fn bind_struct_literal(
        &mut self,
        struct_literal_node: StructLiteralNode<Parsed>,
        location: Location,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        let (namespaces, identifier) =
            intern_namespaced_identifier(struct_literal_node.identifier, compiler);
        let identifier = compiler
            .variables
            .find_by_name(&namespaces, identifier)
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
        let mut assigned: HashMap<StringId, bool> =
            struct_type.fields.iter().map(|f| (f.1, false)).collect();
        for actual in fields.iter() {
            assigned.insert(actual.identifier, true);
        }
        let too_much: Vec<StringId> = assigned
            .iter()
            .filter(|(_, v)| **v)
            .filter_map(|(n, _)| {
                struct_type
                    .get_field_type_by_name(*n)
                    .is_none()
                    .then_some(*n)
            })
            .collect();
        let missing: Vec<(StringId, TypeId)> = assigned
            .into_iter()
            .filter(|(_, v)| !v)
            .map(|(n, _)| (n, struct_type.get_field_type_by_name(n).unwrap()))
            .collect();
        if !missing.is_empty() {
            compiler
                .diagnostics
                .report_missing_fields_in_struct_initialisation(
                    location,
                    type_,
                    missing,
                    compiler.variables[struct_type.identifier].location,
                );
        }
        if !too_much.is_empty() {
            compiler
                .diagnostics
                .report_unknown_fields_in_struct_initialisation(
                    location,
                    type_,
                    too_much,
                    compiler.variables[struct_type.identifier].location,
                )
        }
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
        location: Location,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        let base = self.bind_node(*field_access_node.base, TypeId::UNKNOWN, compiler);
        let field_identifier = compiler.intern_location(field_access_node.field.location());
        let base_type = compiler.nodes.type_of(base);
        let traits = compiler.trait_implementors.find_traits(base_type);
        for t in traits {
            let Some(function) = compiler.traits[*t].find_function_by_name(field_identifier) else {
                continue;
            };
            let type_ = function.type_;
            let vid = unsafe { compiler.nodes.prepare_id() };
            let identifier = SyntaxNode::<Bound>::variable(
                location,
                &compiler.variables[function.name],
                Some(Value::CompileTimeFunction(function.implementation)),
                vid,
            );
            unsafe { compiler.nodes.set(vid, identifier) };
            return SyntaxNode::<Bound>::partial_capture(location, vid, vec![base], type_, id);
        }
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
        // let namespaces =
        let (namespaces, identifier) =
            intern_namespaced_identifier(impl_block_node.identifier, compiler);
        // let identifier = compiler.intern_location(impl_block_node.identifier.location());
        let struct_type = compiler
            .variables
            .find_by_name(&namespaces, identifier)
            .unwrap()
            .clone();
        let type_ = self
            .look_up_constant(struct_type.id)
            .unwrap()
            .as_type()
            .unwrap();
        self.this_type = Some(type_);
        compiler.variables.push_namespace(identifier);
        let functions: Vec<BoundId> = impl_block_node
            .body
            .into_iter()
            .map(|f| self.bind_node(f, TypeId::VOID, compiler))
            .collect();
        let struct_trait = Trait {
            name: struct_type.id,
            functions: functions
                .iter()
                .map(|id| {
                    let f = compiler.nodes[*id]
                        .kind
                        .as_function_declaration()
                        .expect("Only supported for now!");
                    let body = f.body;
                    let variable = f.identifier;
                    let type_ = compiler.variables.type_of(variable);
                    TraitFunction {
                        type_,
                        name: f.identifier,
                        implementation: body,
                    }
                })
                .collect(),
        };
        let trait_id = compiler.traits.register(struct_trait);
        compiler.trait_implementors.register(type_, trait_id);
        // let struct_type = compiler.types.as_struct_type_mut(type_).unwrap();
        // for id in functions {
        //     if compiler.nodes.type_of(id) == TypeId::ERROR {
        //         continue;
        //     }
        //     let f = compiler.nodes[id]
        //         .kind
        //         .as_function_declaration()
        //         .expect("Only supported for now!");
        //     let variable = f.identifier;
        //     let type_ = compiler.variables.type_of(variable);
        //     // TODO: This should probably not live in the struct type!
        //     // struct_type.associated_functions.push((variable, type_));
        //     todo!();
        //     self.register_constant(variable, Value::CompileTimeFunction(f.body));
        // }
        compiler.variables.pop_namespace();
        // TODO: This can be better
        unsafe { SyntaxNode::<Bound>::empty(id) }
    }

    fn remove_integer_literal_type(
        &mut self,
        expression: BoundId,
        compiler: &mut Compiler,
    ) -> BoundId {
        if compiler.types[compiler.nodes.type_of(expression)].is_integer_literal() {
            self.bind_conversion(
                expression,
                TypeId::UNSIGNED_INTEGER_32,
                compiler,
                ConversionKind::Implicit,
            )
        } else {
            expression
        }
    }

    fn bind_variant(
        &mut self,
        variant: EnumVariantNode<Parsed>,
        i: u32,
        type_: TypeId,
        compiler: &mut Compiler,
    ) -> VariableId {
        let identifier = compiler.intern_location(variant.identifier.location());
        let Some(variant) =
            self.register_global_variable(compiler, variant.location, identifier, type_)
        else {
            todo!()
        };
        self.register_constant(variant, Value::UnsignedInteger32(i));
        variant
    }

    fn bind_match_expression(
        &mut self,
        match_expression: MatchExpressionNode<Parsed>,
        expected: TypeId,
        location: Location,
        compiler: &mut Compiler,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        let expression = self.bind_node(*match_expression.expression, TypeId::UNKNOWN, compiler);
        let expression_type = compiler.nodes.type_of(expression);
        let arms: Vec<MatchArmNode<Bound>> = match_expression
            .arms
            .into_iter()
            .map(|a| self.bind_match_arm(a, expression_type, expected, compiler))
            .collect();
        let type_ = arms
            .iter()
            .map(|a| compiler.nodes.type_of(a.body))
            .fold(TypeId::VOID, |a, c| if a == TypeId::VOID { c } else { a });
        let arms: Vec<_> = arms
            .into_iter()
            .map(|a| {
                let body = self.bind_conversion(a.body, type_, compiler, ConversionKind::Implicit);
                MatchArmNode::<Bound>::new(a.pattern, body)
            })
            .collect();
        SyntaxNode::<Bound>::match_expression(location, expression, arms, type_, id)
    }

    fn bind_match_arm(
        &mut self,
        arm: MatchArmNode<Parsed>,
        expression_type: TypeId,
        expected: TypeId,
        compiler: &mut Compiler,
    ) -> MatchArmNode<Bound> {
        let pattern = self.bind_pattern(arm.pattern, expression_type, compiler);
        let body = self.bind_node(*arm.body, expected, compiler);
        MatchArmNode::<Bound>::new(pattern, body)
    }

    fn bind_pattern(
        &mut self,
        pattern: <Parsed as Stage>::Pattern,
        type_: TypeId,
        compiler: &mut Compiler,
    ) -> Pattern {
        let fake_id = unsafe { BoundId::from_raw(usize::MAX) };
        let value = self
            .bind_identifier(pattern, compiler, fake_id)
            .stage
            .constant_value
            .expect("todo error handling!");
        Pattern::Constant(value)
    }
}

fn intern_namespaced_identifier(
    namespaced_identifier: NamespacedIdentifier,
    compiler: &mut Compiler,
) -> (Vec<StringId>, StringId) {
    let namespaces: Vec<_> = namespaced_identifier
        .namespaces
        .into_iter()
        .map(|n| compiler.intern_location(n.0.location()))
        .collect();
    let identifier = compiler.intern_location(namespaced_identifier.identifier.location());
    (namespaces, identifier)
}
