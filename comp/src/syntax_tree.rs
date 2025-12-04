#![allow(dead_code)]
use crate::StringId;
use crate::bind::conversion::ConversionKind;
use crate::bind::{BoundBinaryOperator, VariableId};
use crate::{BoundId, HasLocation, Location, lexer::Token, typing::TypeId, value::Value};
use std::fmt::Debug;
use std::marker::PhantomData;

#[derive(Debug, Clone)]
pub struct SyntaxTree<S: Stage> {
    pub node: SyntaxNode<S>,
}

#[derive(Debug, Clone, Copy)]
pub struct Parsed;
#[derive(Debug, Clone)]
pub struct Bound {
    pub id: BoundId,
    pub type_: TypeId,
    pub constant_value: Option<Value>,
}

pub trait Stage: Debug + Clone {
    type IdentifierUnscoped: Debug + Clone;
    type Identifier: Debug + Clone;
    type Token: Debug + Clone;
    type Value: Debug + Clone;
    type BinaryOp: Debug + Clone;
    type Type: Debug + Clone;
    type ChildNodeBoxed: Debug + Clone;
    type ChildNode: Debug + Clone;
}

impl Stage for Parsed {
    type IdentifierUnscoped = Token;
    type Identifier = Token;
    type Token = Token;
    type Value = Box<SyntaxNode<Parsed>>;
    type BinaryOp = Token;
    type Type = TypeIdentifier;
    type ChildNodeBoxed = Box<Self::ChildNode>;
    type ChildNode = SyntaxNode<Parsed>;
}

impl Stage for Bound {
    type IdentifierUnscoped = StringId;
    type Identifier = VariableId;
    type Token = ();
    type Value = Value;
    type BinaryOp = BoundBinaryOperator;
    type Type = TypeId;
    type ChildNode = BoundId;
    type ChildNodeBoxed = BoundId;
}

#[derive(Debug, Clone)]
pub enum TypeIdentifier {
    Error(Location),
    Named(Token),
    Array(ArrayTypeIdentifier),
}

impl HasLocation for TypeIdentifier {
    fn location(&self) -> Location {
        match self {
            TypeIdentifier::Error(location) => *location,
            TypeIdentifier::Named(token) => token.location(),
            TypeIdentifier::Array(array_type_identifier) => array_type_identifier
                .lbracket
                .location()
                .combine(array_type_identifier.rbracket.location()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArrayTypeIdentifier {
    pub lbracket: Token,
    pub type_: Box<TypeIdentifier>,
    pub semicolon: Token,
    pub length: Box<SyntaxNode<Parsed>>,
    pub rbracket: Token,
}

#[derive(Debug, Clone)]
pub struct SyntaxNode<S: Stage> {
    pub location: Location,
    pub kind: SyntaxNodeKind<S>,
    pub stage: S,
}

impl<S: Stage> HasLocation for SyntaxNode<S> {
    fn location(&self) -> Location {
        self.location
    }
}

impl SyntaxNode<Bound> {
    pub fn const_declaration(
        location: Location,
        variable: VariableId,
        value: Value,
        id: BoundId,
    ) -> Self {
        Self {
            location,
            kind: SyntaxNodeKind::ConstDeclaration(ConstDeclarationNode {
                const_keyword: (),
                identifier: variable,
                equals: (),
                expr: value,
                semicolon: (),
            }),
            stage: Bound {
                id,
                type_: TypeId::VOID,
                constant_value: None,
            },
        }
    }
    pub fn literal(literal: Token, value: Value, type_: TypeId, id: BoundId) -> Self {
        Self {
            location: literal.location(),
            kind: SyntaxNodeKind::Literal(literal),
            stage: Bound {
                id,
                type_,
                constant_value: Some(value),
            },
        }
    }

    pub fn error(location: Location, id: BoundId) -> Self {
        Self {
            location,
            kind: SyntaxNodeKind::Error,
            stage: Bound {
                id,
                type_: TypeId::ERROR,
                constant_value: Some(Value::Error),
            },
        }
    }

    pub(crate) fn program(
        top_level_statements: Vec<BoundId>,
        location: Location,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        Self {
            location,
            kind: SyntaxNodeKind::Program(ProgramNode {
                top_level_statements,
                eof: (),
            }),
            stage: Bound {
                id,
                type_: TypeId::VOID,
                constant_value: None,
            },
        }
    }

    pub(crate) fn binary(
        location: Location,
        lhs: BoundId,
        op: BoundBinaryOperator,
        rhs: BoundId,
        type_: TypeId,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        Self {
            location,
            kind: SyntaxNodeKind::Binary(BinaryNode { lhs, op, rhs }),
            stage: Bound {
                id,
                type_,
                constant_value: None,
            },
        }
    }

    pub(crate) fn variable(
        location: Location,
        variable: &crate::bind::VariableDeclaration,
        constant_value: Option<Value>,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        Self {
            location,
            kind: SyntaxNodeKind::Identifier(variable.id),
            stage: Bound {
                id,
                type_: variable.type_,
                constant_value,
            },
        }
    }

    pub(crate) fn array_literal(
        entries: Vec<BoundId>,
        type_: TypeId,
        location: Location,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        Self {
            location,
            kind: SyntaxNodeKind::ArrayLiteral(ArrayLiteralNode {
                lbracket: (),
                entries,
                rbracket: (),
            }),
            stage: Bound {
                id,
                type_,
                constant_value: None,
            },
        }
    }

    pub(crate) fn expression_statement(
        expression: BoundId,
        location: Location,
        has_semicolon: bool,
        id: BoundId,
        type_: TypeId,
    ) -> SyntaxNode<Bound> {
        Self {
            location,
            kind: SyntaxNodeKind::ExpressionStatement(ExpressionStatementNode {
                expression,
                semicolon: to_option(has_semicolon),
            }),
            stage: Bound {
                id,
                type_,
                constant_value: None,
            },
        }
    }

    pub(crate) fn block_expression(
        body: Vec<BoundId>,
        location: Location,
        type_: TypeId,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        Self {
            location,
            kind: SyntaxNodeKind::BlockExpression(BlockExpressionNode {
                lbrace: (),
                body,
                rbrace: (),
            }),
            stage: Bound {
                id,
                type_,
                constant_value: None,
            },
        }
    }

    pub(crate) fn function_declaration(
        generics: Option<GenericParameterHeaderNode<Bound>>,
        identifier: VariableId,
        location: Location,
        parameters: Vec<ParameterNode<Bound>>,
        body: BoundId,
        return_type: TypeId,
        is_comp: bool,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        Self {
            location,
            kind: SyntaxNodeKind::FunctionDeclaration(FunctionDeclarationNode {
                comp_keyword: to_option(is_comp),
                fn_keyword: (),
                identifier,
                head: FunctionHeaderNode {
                    generics,
                    location,
                    lparen: (),
                    parameters,
                    rparen: (),
                    return_type: Some(((), return_type)),
                    _marker: Default::default(),
                },
                body,
            }),
            stage: Bound {
                id,
                type_: TypeId::VOID,
                constant_value: None,
            },
        }
    }

    pub(crate) fn function_call(
        location: Location,
        base: BoundId,
        arguments: Vec<BoundId>,
        type_: TypeId,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        Self {
            location,
            kind: SyntaxNodeKind::FunctionCall(FunctionCallNode {
                base,
                lparen: (),
                arguments,
                rparen: (),
            }),
            stage: Bound {
                id,
                type_,
                constant_value: None,
            },
        }
    }

    pub(crate) fn assignment_statement(
        lhs: BoundId,
        value: BoundId,
        location: Location,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        Self {
            location,
            kind: SyntaxNodeKind::AssignmentStatement(AssignmentStatementNode {
                lhs,
                equals: (),
                value,
                semicolon: (),
            }),
            stage: Bound {
                id,
                type_: TypeId::VOID,
                constant_value: None,
            },
        }
    }

    pub(crate) fn conversion(
        location: Location,
        base: BoundId,
        expected: TypeId,
        conversion_kind: ConversionKind,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        Self {
            location,
            kind: SyntaxNodeKind::Conversion(ConversionNode {
                base,
                conversion_kind,
            }),
            stage: Bound {
                id,
                type_: expected,
                constant_value: None,
            },
        }
    }

    pub unsafe fn empty(id: BoundId) -> SyntaxNode<Bound> {
        Self {
            location: unsafe { Location::zero() },
            kind: SyntaxNodeKind::BlockExpression(BlockExpressionNode {
                lbrace: (),
                body: Vec::new(),
                rbrace: (),
            }),
            stage: Bound {
                id,
                type_: TypeId::VOID,
                constant_value: None,
            },
        }
    }

    pub(crate) fn struct_literal(
        location: Location,
        identifier: VariableId,
        fields: Vec<FieldInitilizationNode<Bound>>,
        type_: TypeId,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        Self {
            location,
            kind: SyntaxNodeKind::StructLiteral(StructLiteralNode {
                identifier,
                lbrace: (),
                fields,
                rbrace: (),
            }),
            stage: Bound {
                id,
                type_,
                constant_value: None,
            },
        }
    }
}

fn to_option(value: bool) -> Option<()> {
    if value { Some(()) } else { None }
}

impl SyntaxNode<Parsed> {
    pub fn error(location: Location) -> Self {
        Self {
            location,
            kind: SyntaxNodeKind::Error,
            stage: Parsed,
        }
    }

    pub fn program(top_level_statements: Vec<SyntaxNode<Parsed>>, eof: Token) -> Self {
        let location = eof
            .location()
            .combine(Location::from_vec(&top_level_statements));
        Self {
            stage: Parsed,
            location,
            kind: SyntaxNodeKind::Program(ProgramNode {
                top_level_statements,
                eof,
            }),
        }
    }

    pub(crate) fn const_declaration(
        const_keyword: Token,
        identifier: Token,
        equals: Token,
        expression: SyntaxNode<Parsed>,
        semicolon: Token,
    ) -> SyntaxNode<Parsed> {
        let location = const_keyword.location().combine(semicolon.location());
        Self {
            stage: Parsed,
            location,
            kind: SyntaxNodeKind::ConstDeclaration(ConstDeclarationNode {
                const_keyword,
                identifier,
                equals,
                expr: Box::new(expression),
                semicolon,
            }),
        }
    }

    pub(crate) fn function_declaration(
        comp_keyword: Option<Token>,
        fn_keyword: Token,
        identifier: Token,
        function_header: FunctionHeaderNode<Parsed>,
        body: SyntaxNode<Parsed>,
    ) -> SyntaxNode<Parsed> {
        let location = fn_keyword
            .location()
            .combine(comp_keyword.map(|l| l.location()))
            .combine(body.location());
        Self {
            location,
            kind: SyntaxNodeKind::FunctionDeclaration(FunctionDeclarationNode {
                comp_keyword,
                fn_keyword,
                identifier,
                head: function_header,
                body: Box::new(body),
            }),
            stage: Parsed,
        }
    }

    pub(crate) fn struct_declaration(
        comp_keyword: Option<Token>,
        struct_keyword: Token,
        identifier: Token,
        lbrace: Token,
        fields: Vec<ParameterNode<Parsed>>,
        rbrace: Token,
    ) -> SyntaxNode<Parsed> {
        let location = struct_keyword
            .location()
            .combine(comp_keyword.map(|l| l.location()))
            .combine(rbrace.location());
        Self {
            location,
            kind: SyntaxNodeKind::StructDeclaration(StructDeclarationNode {
                comp_keyword,
                struct_keyword,
                identifier,
                lbrace,
                fields,
                rbrace,
            }),
            stage: Parsed,
        }
    }

    pub(crate) fn assignment_statement(
        lhs: SyntaxNode<Parsed>,
        equals: Token,
        value: SyntaxNode<Parsed>,
        semicolon: Token,
    ) -> SyntaxNode<Parsed> {
        let location = lhs.location().combine(semicolon.location());
        Self {
            location,
            kind: SyntaxNodeKind::AssignmentStatement(AssignmentStatementNode {
                lhs: Box::new(lhs),
                equals,
                value: Box::new(value),
                semicolon,
            }),
            stage: Parsed,
        }
    }

    pub(crate) fn function_call(
        base: SyntaxNode<Parsed>,
        lparen: Token,
        arguments: Vec<SyntaxNode<Parsed>>,
        rparen: Token,
    ) -> SyntaxNode<Parsed> {
        let location = base.location().combine(rparen.location());
        Self {
            location,
            kind: SyntaxNodeKind::FunctionCall(FunctionCallNode {
                base: Box::new(base),
                lparen,
                arguments,
                rparen,
            }),
            stage: Parsed,
        }
    }

    pub(crate) fn struct_literal(
        identifier: Token,
        lbrace: Token,
        fields: Vec<FieldInitilizationNode<Parsed>>,
        rbrace: Token,
    ) -> SyntaxNode<Parsed> {
        let location = identifier.location().combine(rbrace.location());
        Self {
            location,
            kind: SyntaxNodeKind::StructLiteral(StructLiteralNode {
                identifier,
                lbrace,
                fields,
                rbrace,
            }),
            stage: Parsed,
        }
    }

    pub(crate) fn literal(literal: Token) -> SyntaxNode<Parsed> {
        Self {
            location: literal.location(),
            kind: SyntaxNodeKind::Literal(literal),
            stage: Parsed,
        }
    }

    pub(crate) fn variable(variable: Token) -> SyntaxNode<Parsed> {
        Self {
            location: variable.location(),
            kind: SyntaxNodeKind::Identifier(variable),
            stage: Parsed,
        }
    }

    pub(crate) fn binary(
        lhs: SyntaxNode<Parsed>,
        op: Token,
        rhs: SyntaxNode<Parsed>,
    ) -> SyntaxNode<Parsed> {
        let location = lhs.location().combine(rhs.location());
        Self {
            location,
            kind: SyntaxNodeKind::Binary(BinaryNode {
                lhs: Box::new(lhs),
                op,
                rhs: Box::new(rhs),
            }),
            stage: Parsed,
        }
    }

    pub(crate) fn commaed_expression(
        expression: SyntaxNode<Parsed>,
        comma: Option<Token>,
    ) -> SyntaxNode<Parsed> {
        let location = expression.location.combine(comma.map(|c| c.location()));
        Self {
            location,
            kind: SyntaxNodeKind::CommaedExpression((Box::new(expression), comma)),
            stage: Parsed,
        }
    }

    pub(crate) fn array_literal(
        lbracket: Token,
        entries: Vec<SyntaxNode<Parsed>>,
        rbracket: Token,
    ) -> SyntaxNode<Parsed> {
        let location = lbracket.location().combine(rbracket.location());
        Self {
            location,
            kind: SyntaxNodeKind::ArrayLiteral(ArrayLiteralNode {
                lbracket,
                entries,
                rbracket,
            }),
            stage: Parsed,
        }
    }

    pub(crate) fn expression_statement(
        expression: SyntaxNode<Parsed>,
        semicolon: Option<Token>,
    ) -> SyntaxNode<Parsed> {
        let location = expression
            .location()
            .combine(semicolon.map(|l| l.location()));
        Self {
            location,
            kind: SyntaxNodeKind::ExpressionStatement(ExpressionStatementNode {
                expression: Box::new(expression),
                semicolon,
            }),
            stage: Parsed,
        }
    }

    pub(crate) fn block_expression(
        lbrace: Token,
        body: Vec<SyntaxNode<Parsed>>,
        rbrace: Token,
    ) -> SyntaxNode<Parsed> {
        let location = lbrace.location().combine(rbrace.location());
        Self {
            location,
            kind: SyntaxNodeKind::BlockExpression(BlockExpressionNode {
                lbrace,
                body,
                rbrace,
            }),
            stage: Parsed,
        }
    }
}

#[derive(Debug, Clone)]
pub enum SyntaxNodeKind<S: Stage> {
    Error,
    Program(ProgramNode<S>),
    ConstDeclaration(ConstDeclarationNode<S>),
    Literal(Token),
    Identifier(S::Identifier),
    Binary(BinaryNode<S>),
    CommaedExpression((S::ChildNodeBoxed, Option<Token>)),
    ArrayLiteral(ArrayLiteralNode<S>),
    ExpressionStatement(ExpressionStatementNode<S>),
    BlockExpression(BlockExpressionNode<S>),
    FunctionDeclaration(FunctionDeclarationNode<S>),
    FunctionCall(FunctionCallNode<S>),
    AssignmentStatement(AssignmentStatementNode<S>),
    Conversion(ConversionNode<S>),
    StructDeclaration(StructDeclarationNode<S>),
    StructLiteral(StructLiteralNode<S>),
}

#[derive(Debug, Clone)]
pub struct ConversionNode<S: Stage> {
    base: S::ChildNodeBoxed,
    conversion_kind: ConversionKind,
}

#[derive(Debug, Clone)]
pub struct FunctionCallNode<S: Stage> {
    pub base: S::ChildNodeBoxed,
    lparen: S::Token,
    pub arguments: Vec<S::ChildNode>,
    rparen: S::Token,
}

#[derive(Debug, Clone)]
pub struct StructLiteralNode<S: Stage> {
    pub identifier: S::Identifier,
    lbrace: S::Token,
    pub fields: Vec<FieldInitilizationNode<S>>,
    rbrace: S::Token,
}
#[derive(Debug, Clone)]
pub struct AssignmentStatementNode<S: Stage> {
    pub lhs: S::ChildNodeBoxed,
    equals: S::Token,
    pub value: S::ChildNodeBoxed,
    semicolon: S::Token,
}

#[derive(Debug, Clone)]
pub struct FunctionDeclarationNode<S: Stage> {
    pub comp_keyword: Option<S::Token>,
    fn_keyword: S::Token,
    pub identifier: S::Identifier,
    pub head: FunctionHeaderNode<S>,
    pub body: S::ChildNodeBoxed,
}

#[derive(Debug, Clone)]
pub struct StructDeclarationNode<S: Stage> {
    pub comp_keyword: Option<S::Token>,
    struct_keyword: S::Token,
    pub identifier: S::Identifier,
    lbrace: S::Token,
    pub fields: Vec<ParameterNode<S>>,
    rbrace: S::Token,
}

#[derive(Debug, Clone)]
pub struct BlockExpressionNode<S: Stage> {
    lbrace: S::Token,
    pub body: Vec<S::ChildNode>,
    rbrace: S::Token,
}

#[derive(Debug, Clone)]
pub struct ExpressionStatementNode<S: Stage> {
    pub expression: S::ChildNodeBoxed,
    pub semicolon: Option<S::Token>,
}

#[derive(Debug, Clone)]
pub struct FunctionHeaderNode<S: Stage> {
    pub location: Location,
    lparen: S::Token,
    pub parameters: Vec<ParameterNode<S>>,
    rparen: S::Token,
    pub return_type: Option<(S::Token, S::Type)>,
    _marker: PhantomData<S>,
    pub generics: Option<GenericParameterHeaderNode<S>>,
}
impl FunctionHeaderNode<Parsed> {
    pub(crate) fn new(
        generics: Option<GenericParameterHeaderNode<Parsed>>,
        lparen: Token,
        parameters: Vec<ParameterNode<Parsed>>,
        rparen: Token,
        return_type: Option<(Token, TypeIdentifier)>,
    ) -> Self {
        let location = lparen
            .location()
            .combine(rparen.location())
            .combine(return_type.as_ref().map(|(_, t)| t.location()))
            .combine(generics.as_ref().map(|g| g.location()));
        Self {
            location,
            generics,
            lparen,
            parameters,
            rparen,
            return_type,
            _marker: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GenericParameterHeaderNode<S: Stage> {
    pub location: Location,
    pub less_than: S::Token,
    pub parameters: Vec<GenericParameterNode<S>>,
    pub greater_than: S::Token,
}

impl GenericParameterHeaderNode<Bound> {
    pub fn new(location: Location, parameters: Vec<GenericParameterNode<Bound>>) -> Self {
        Self {
            location,
            less_than: (),
            parameters,
            greater_than: (),
        }
    }
}

impl GenericParameterHeaderNode<Parsed> {
    pub(crate) fn new(
        less_than: Token,
        parameters: Vec<GenericParameterNode<Parsed>>,
        greater_than: Token,
    ) -> Self {
        let location = less_than.location().combine(greater_than.location());
        Self {
            location,
            less_than,
            parameters,
            greater_than,
        }
    }
}

impl<S: Stage> HasLocation for GenericParameterHeaderNode<S> {
    fn location(&self) -> Location {
        self.location
    }
}

#[derive(Debug, Clone)]
pub struct GenericParameterNode<S: Stage> {
    pub location: Location,
    pub out: Option<S::Token>,
    pub identifier: S::Identifier,
    pub type_: Option<(S::Token, S::Type)>,
    pub comma: Option<S::Token>,
}

impl GenericParameterNode<Bound> {
    pub fn new(
        is_out: bool,
        identifier: VariableId,
        type_: Option<TypeId>,
        location: Location,
    ) -> Self {
        Self {
            location,
            out: to_option(is_out),
            identifier,
            type_: type_.map(|t| ((), t)),
            comma: None,
        }
    }
}

impl GenericParameterNode<Parsed> {
    pub(crate) fn new(
        out: Option<Token>,
        identifier: Token,
        type_: Option<(Token, TypeIdentifier)>,
        comma: Option<Token>,
    ) -> Self {
        let location = identifier
            .location()
            .combine(out.map(|l| l.location()))
            .combine(type_.as_ref().map(|(_, t)| t.location()))
            .combine(comma.map(|l| l.location()));
        Self {
            location,
            out,
            identifier,
            type_,
            comma,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FieldInitilizationNode<S: Stage> {
    pub location: Location,
    pub identifier: S::IdentifierUnscoped,
    colon: S::Token,
    pub expression: S::ChildNodeBoxed,
    comma: Option<S::Token>,
    _marker: PhantomData<S>,
}

impl FieldInitilizationNode<Bound> {
    pub(crate) fn new(
        location: Location,
        identifier: crate::StringId,
        expression: BoundId,
    ) -> Self {
        Self {
            location,
            identifier,
            colon: (),
            expression,
            comma: None,
            _marker: Default::default(),
        }
    }
}

impl FieldInitilizationNode<Parsed> {
    pub(crate) fn new(
        identifier: Token,
        colon: Token,
        expression: SyntaxNode<Parsed>,
        comma: Option<Token>,
    ) -> Self {
        let location = identifier
            .location()
            .combine(comma.map(|c| c.location()))
            .combine(expression.location());
        Self {
            location,
            identifier,
            colon,
            expression: Box::new(expression),
            comma,
            _marker: Default::default(),
        }
    }
}

impl<S: Stage> HasLocation for FieldInitilizationNode<S> {
    fn location(&self) -> Location {
        self.location
    }
}

#[derive(Debug, Clone)]
pub struct ParameterNode<S: Stage> {
    pub location: Location,
    pub identifier: S::Identifier,
    colon: S::Token,
    pub type_: S::Type,
    comma: Option<S::Token>,
    _marker: PhantomData<S>,
}

impl ParameterNode<Bound> {
    pub fn new(location: Location, identifier: VariableId, type_: TypeId) -> Self {
        Self {
            location,
            identifier,
            colon: (),
            type_,
            comma: None,
            _marker: Default::default(),
        }
    }
}

impl ParameterNode<Parsed> {
    pub(crate) fn new(
        identifier: Token,
        colon: Token,
        type_identifier: TypeIdentifier,
        comma: Option<Token>,
    ) -> Self {
        let location = identifier
            .location()
            .combine(type_identifier.location())
            .combine(comma.map(|c| c.location()));
        Self {
            location,
            identifier,
            colon,
            type_: type_identifier,
            comma,
            _marker: Default::default(),
        }
    }

    pub fn ends_with_comma(&self) -> bool {
        self.comma.is_some()
    }
}

impl<S: Stage> SyntaxNodeKind<S> {
    pub fn ends_with_comma(&self) -> bool {
        match self {
            SyntaxNodeKind::CommaedExpression((_, c)) => c.is_some(),
            // SyntaxNodeKind::Parameter(p) => p.comma.is_some(),
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArrayLiteralNode<S: Stage> {
    lbracket: S::Token,
    pub entries: Vec<S::ChildNode>,
    rbracket: S::Token,
}

#[derive(Debug, Clone)]
pub struct BinaryNode<S: Stage> {
    pub lhs: S::ChildNodeBoxed,
    pub op: S::BinaryOp,
    pub rhs: S::ChildNodeBoxed,
}

#[derive(Debug, Clone)]
pub struct ProgramNode<S: Stage> {
    pub top_level_statements: Vec<S::ChildNode>,
    eof: S::Token,
}

#[derive(Debug, Clone)]
pub struct ConstDeclarationNode<S: Stage> {
    const_keyword: S::Token,
    pub identifier: S::Identifier,
    equals: S::Token,
    pub expr: S::Value,
    semicolon: S::Token,
}
