use std::collections::HashMap;

use crate::{
    Compiler, HasLocation, Location, SourceTextId, StringId, const_evaluator,
    lexer::{Token, TokenKind},
    parser::Parser,
    syntax_tree::*,
    typing::{Type, TypeId, Types},
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

#[derive(Debug)]
pub struct VariableDeclaration {
    pub id: VariableId,
    pub name: StringId,
    pub type_: TypeId,
}

#[derive(Debug)]
pub struct Variables {
    variables: Vec<VariableDeclaration>,
    scopes: Vec<usize>,
}

impl Variables {
    pub fn new() -> Self {
        Self {
            variables: Vec::new(),
            scopes: Vec::new(),
        }
    }

    pub fn register(&mut self, name: StringId, type_: TypeId) -> Option<VariableId> {
        let scope_start = self.scopes.last().copied().unwrap_or_default();
        if self.variables[scope_start..].iter().any(|v| v.name == name) {
            None
        } else {
            let index = self.variables.len();
            self.variables.push(VariableDeclaration {
                id: VariableId(index),
                name,
                type_,
            });
            Some(VariableId(index))
        }
    }

    fn find_by_name(&self, name: StringId) -> Option<&VariableDeclaration> {
        self.variables.iter().find(|v| v.name == name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VariableId(usize);

#[derive(Debug)]
pub struct Binder {
    file: SourceTextId,
    types: Types,
    variables: Variables,
    constants: HashMap<VariableId, Value>,
}

impl Binder {
    pub fn new(file: SourceTextId) -> Binder {
        Self {
            file,
            types: Types::new(),
            variables: Variables::new(),
            constants: HashMap::new(),
        }
    }

    pub fn bind(&mut self, compiler: &mut Compiler) -> SyntaxTree<Bound> {
        let tree = Parser::new(self.file).parse(compiler);
        let node = self.bind_node(tree.node, TypeId::VOID, compiler);
        SyntaxTree { node }
    }

    fn bind_node(
        &mut self,
        node: SyntaxNode<Parsed>,
        expected: TypeId,
        compiler: &mut Compiler,
    ) -> SyntaxNode<Bound> {
        let location = node.location();
        match node.kind {
            SyntaxNodeKind::Error => SyntaxNode::error(node.location()),
            SyntaxNodeKind::Program(program_node) => {
                self.bind_program(program_node, expected, location, compiler)
            }
            SyntaxNodeKind::ConstDeclaration(const_declaration_node) => {
                self.bind_const_declaration(const_declaration_node, expected, location, compiler)
            }
            SyntaxNodeKind::Literal(token) => self.bind_literal(token, expected, compiler),
            SyntaxNodeKind::Binary(binary_node) => {
                self.bind_binary(binary_node, expected, compiler)
            }
            SyntaxNodeKind::Identifier(identifier) => {
                self.bind_identifier(identifier, expected, compiler)
            }
            SyntaxNodeKind::CommaedExpression((e, _)) => self.bind_node(*e, expected, compiler),
            SyntaxNodeKind::ArrayLiteral(array_literal_node) => {
                self.bind_array_literal(array_literal_node, expected, location, compiler)
            }
        }
    }

    fn bind_literal(
        &mut self,
        token: Token,
        expected: TypeId,
        compiler: &mut Compiler,
    ) -> SyntaxNode<Bound> {
        match token.kind {
            TokenKind::Error => SyntaxNode::error(token.location()),
            TokenKind::Integer => {
                let value = compiler[token.location()].parse().expect("Error handling!");
                SyntaxNode::<Bound>::literal(token, Value::Integer(value), TypeId::INTEGER)
            }
            TokenKind::FalseKeyword | TokenKind::TrueKeyword => {
                let value = token.kind == TokenKind::TrueKeyword;
                SyntaxNode::<Bound>::literal(token, Value::Bool(value), TypeId::BOOL)
            }
            _ => todo!("Parse error!"),
        }
    }

    fn bind_identifier(
        &mut self,
        token: Token,
        expected: TypeId,
        compiler: &mut Compiler,
    ) -> SyntaxNode<Bound> {
        let name = compiler.intern_location(token.location());
        if let Some(variable) = self.look_up_variable_by_name(name) {
            assert!(expected == variable.type_ || expected == TypeId::UNKNOWN);
            SyntaxNode::<Bound>::variable(
                token.location(),
                variable,
                self.look_up_constant(variable.id),
            )
        } else {
            todo!("Error handling!")
        }
    }

    fn bind_const_declaration(
        &mut self,
        const_declaration_node: ConstDeclarationNode<Parsed>,
        expected: TypeId,
        location: Location,
        compiler: &mut Compiler,
    ) -> SyntaxNode<Bound> {
        assert_eq!(expected, TypeId::VOID);
        let mut expression =
            self.bind_node(*const_declaration_node.expr, TypeId::UNKNOWN, compiler);
        // let variable = &compiler[];
        let variable = compiler.intern_location(const_declaration_node.identifier.location());

        let Some(variable) = self.register_variable(variable, expression.stage.type_) else {
            return SyntaxNode::error(const_declaration_node.identifier.location());
        };
        const_evaluator::evaluate(&mut expression, compiler);
        let value = expression.stage.constant_value.expect("Is not constant??");
        self.register_constant(variable, value.clone());
        // todo!();
        SyntaxNode::<Bound>::const_declaration(location, variable, value)
        // SyntaxNode::<Bound>::const_declaration(variable, expression)
    }

    fn register_variable(&mut self, variable_name: StringId, type_: TypeId) -> Option<VariableId> {
        self.variables.register(variable_name, type_)
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
    ) -> SyntaxNode<Bound> {
        assert_eq!(expected, TypeId::VOID);
        let statements: Vec<SyntaxNode<Bound>> = program_node
            .top_level_statements
            .into_iter()
            .map(|s| self.bind_node(s, TypeId::VOID, compiler))
            .collect();
        SyntaxNode::<Bound>::program(statements, location)
    }

    fn bind_binary(
        &mut self,
        binary_node: BinaryNode<Parsed>,
        expected: TypeId,
        compiler: &mut Compiler,
    ) -> SyntaxNode<Bound> {
        assert!(expected == TypeId::INTEGER || expected == TypeId::UNKNOWN);
        let lhs = self.bind_node(*binary_node.lhs, TypeId::UNKNOWN, compiler);
        let rhs = self.bind_node(*binary_node.rhs, TypeId::UNKNOWN, compiler);
        let op = match binary_node.op.kind {
            TokenKind::Plus => BoundBinaryOperator::Addition,
            TokenKind::Minus => BoundBinaryOperator::Subtraction,
            TokenKind::Star => BoundBinaryOperator::Multiplication,
            TokenKind::Slash => BoundBinaryOperator::Division,
            TokenKind::Percent => BoundBinaryOperator::Modulo,
            _ => unreachable!("Compiler error!"),
        };
        SyntaxNode::<Bound>::binary(lhs, op, rhs, TypeId::INTEGER)
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
    ) -> SyntaxNode<Bound> {
        let entries: Vec<SyntaxNode<Bound>> = array_literal_node
            .entries
            .into_iter()
            .map(|e|
            // TODO: Use correct expected Type here!
            self.bind_node(e, TypeId::UNKNOWN, compiler))
            .collect();
        let inner_type = entries
            .iter()
            .map(|e| e.stage.type_)
            .fold(TypeId::UNKNOWN, |acc, cur| acc);
        let length = entries.len();
        let type_ = self.register_type(Type::Array(inner_type, length));
        assert!(type_ == expected || expected == TypeId::UNKNOWN);
        SyntaxNode::<Bound>::array_literal(entries, type_, location)
    }

    fn register_type(&mut self, type_: Type) -> TypeId {
        self.types.register(type_)
    }
}
