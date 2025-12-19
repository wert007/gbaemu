#![allow(dead_code)]
use crate::StringId;
use crate::bind::BoundBinaryOperator;
use crate::bind::conversion::ConversionKind;
use crate::pattern::Pattern;
use crate::variables::VariableId;
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
    type BinaryOp: Debug + Clone;
    type Type: Debug + Clone;
    type ChildNodeBoxed: Debug + Clone;
    type ChildNode: Debug + Clone;
    type Pattern: Debug + Clone;
}

impl Stage for Parsed {
    type IdentifierUnscoped = Token;
    type Identifier = NamespacedIdentifier;
    type Token = Token;
    type BinaryOp = Token;
    type Type = TypeIdentifier;
    type ChildNodeBoxed = Box<Self::ChildNode>;
    type ChildNode = SyntaxNode<Parsed>;
    type Pattern = NamespacedIdentifier;
}

impl Stage for Bound {
    type IdentifierUnscoped = StringId;
    type Identifier = VariableId;
    type Token = ();
    type BinaryOp = BoundBinaryOperator;
    type Type = TypeId;
    type ChildNode = BoundId;
    type ChildNodeBoxed = BoundId;
    type Pattern = crate::pattern::Pattern;
}

#[derive(Debug, Clone)]
pub enum TypeIdentifier {
    Error(Location),
    Named(NamedTypeIdentifier),
    Array(ArrayTypeIdentifier),
    This(ThisTypeIdentifier),
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
            TypeIdentifier::This(this) => this.location,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ThisTypeIdentifier {
    pub ampersand: Option<Token>,
    pub location: Location,
}

#[derive(Debug, Clone)]
pub struct NamedTypeIdentifier {
    pub ampersand: Option<Token>,
    pub identifier: NamespacedIdentifier,
}

impl HasLocation for NamedTypeIdentifier {
    fn location(&self) -> Location {
        self.identifier
            .location()
            .combine(self.ampersand.map(|a| a.location()))
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
        value: BoundId,
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
        variable: &crate::variables::VariableDeclaration,
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

    pub(crate) fn field_access(
        location: Location,
        base: BoundId,
        field: StringId,
        type_: TypeId,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        Self {
            location,
            kind: SyntaxNodeKind::FieldAccess(FieldAccessNode {
                base,
                period: (),
                field,
            }),
            stage: Bound {
                id,
                type_,
                constant_value: None,
            },
        }
    }

    pub(crate) fn partial_capture(
        location: Location,
        identifier: BoundId,
        arguments: Vec<BoundId>,
        type_: TypeId,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        Self {
            location,
            kind: SyntaxNodeKind::PartialCapture(PartialCaptureNode {
                identifier,
                arguments,
            }),
            stage: Bound {
                id,
                type_,
                constant_value: None,
            },
        }
    }

    pub(crate) fn match_expression(
        location: Location,
        expression: BoundId,
        arms: Vec<MatchArmNode<Bound>>,
        type_: TypeId,
        id: BoundId,
    ) -> SyntaxNode<Bound> {
        Self {
            location,
            stage: Bound {
                id,
                type_,
                constant_value: None,
            },
            kind: SyntaxNodeKind::MatchExpression(MatchExpressionNode {
                match_keyword: (),
                expression,
                lbrace: (),
                arms,
                rbrace: (),
            }),
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
                identifier: NamespacedIdentifier::without_namespace(identifier),
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
                identifier: NamespacedIdentifier::without_namespace(identifier),
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
                identifier: NamespacedIdentifier::without_namespace(identifier),
                lbrace,
                fields,
                rbrace,
            }),
            stage: Parsed,
        }
    }

    pub(crate) fn enum_declaration(
        enum_keyword: Token,
        identifier: Token,
        lbrace: Token,
        variants: Vec<EnumVariantNode<Parsed>>,
        rbrace: Token,
    ) -> SyntaxNode<Parsed> {
        let location = enum_keyword.location().combine(rbrace.location());
        Self {
            location,
            stage: Parsed,
            kind: SyntaxNodeKind::EnumDeclaration(EnumDeclarationNode {
                enum_keyword,
                identifier: NamespacedIdentifier::without_namespace(identifier),
                lbrace,
                variants,
                rbrace,
            }),
        }
    }

    pub(crate) fn impl_block(
        impl_keyword: Token,
        identifier: NamespacedIdentifier,
        lbrace: Token,
        body: Vec<SyntaxNode<Parsed>>,
        rbrace: Token,
    ) -> SyntaxNode<Parsed> {
        let location = impl_keyword.location().combine(rbrace.location());
        Self {
            location,
            stage: Parsed,
            kind: SyntaxNodeKind::ImplBlock(ImplBlockNode {
                impl_keyword,
                identifier,
                lbrace,
                body,
                rbrace,
            }),
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

    pub(crate) fn field_access(
        base: SyntaxNode<Parsed>,
        period: Token,
        field: Token,
    ) -> SyntaxNode<Parsed> {
        let location = base.location().combine(field.location());
        Self {
            location,
            stage: Parsed,
            kind: SyntaxNodeKind::FieldAccess(FieldAccessNode {
                base: Box::new(base),
                period,
                field,
            }),
        }
    }

    pub(crate) fn struct_literal(
        identifier: NamespacedIdentifier,
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

    pub(crate) fn variable(variable: NamespacedIdentifier) -> SyntaxNode<Parsed> {
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

    pub(crate) fn match_expression(
        match_keyword: Token,
        expression: SyntaxNode<Parsed>,
        lbrace: Token,
        arms: Vec<MatchArmNode<Parsed>>,
        rbrace: Token,
    ) -> SyntaxNode<Parsed> {
        let location = match_keyword.location().combine(rbrace.location());
        Self {
            location,
            kind: SyntaxNodeKind::MatchExpression(MatchExpressionNode {
                match_keyword,
                expression: Box::new(expression),
                lbrace,
                arms,
                rbrace,
            }),
            stage: Parsed,
        }
    }
}

#[derive(Debug, Clone, strum::IntoStaticStr)]
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
    FieldAccess(FieldAccessNode<S>),
    ImplBlock(ImplBlockNode<S>),
    PartialCapture(PartialCaptureNode<S>),
    EnumDeclaration(EnumDeclarationNode<S>),
    MatchExpression(MatchExpressionNode<S>),
}

#[derive(Debug, Clone)]
pub struct MatchExpressionNode<S: Stage> {
    match_keyword: S::Token,
    pub expression: S::ChildNodeBoxed,
    lbrace: S::Token,
    pub arms: Vec<MatchArmNode<S>>,
    rbrace: S::Token,
}

#[derive(Debug, Clone)]
pub struct MatchArmNode<S: Stage> {
    pub pattern: S::Pattern,
    fat_arrow: S::Token,
    pub body: S::ChildNodeBoxed,
    comma: Option<S::Token>,
}

impl MatchArmNode<Parsed> {
    pub fn new(
        pattern: NamespacedIdentifier,
        fat_arrow: Token,
        body: SyntaxNode<Parsed>,
        comma: Option<Token>,
    ) -> Self {
        Self {
            pattern,
            fat_arrow,
            body: Box::new(body),
            comma,
        }
    }
}

impl MatchArmNode<Bound> {
    pub fn new(pattern: Pattern, body: BoundId) -> Self {
        Self {
            pattern,
            fat_arrow: (),
            body,
            comma: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NamespacedIdentifier {
    pub location: Location,
    pub namespaces: Vec<(Token, Token)>,
    pub identifier: Token,
}

impl NamespacedIdentifier {
    pub fn new(namespaces: Vec<(Token, Token)>, identifier: Token) -> Self {
        let location = identifier
            .location()
            .combine(namespaces.first().map(|n| n.0.location()));
        Self {
            location,
            namespaces,
            identifier,
        }
    }

    pub fn without_namespace(identifier: Token) -> NamespacedIdentifier {
        Self {
            location: identifier.location(),
            namespaces: Vec::new(),
            identifier,
        }
    }
}

impl HasLocation for NamespacedIdentifier {
    fn location(&self) -> Location {
        self.location
    }
}

#[derive(Debug, Clone)]
pub struct ConversionNode<S: Stage> {
    pub base: S::ChildNodeBoxed,
    pub conversion_kind: ConversionKind,
}

#[derive(Debug, Clone)]
pub struct FunctionCallNode<S: Stage> {
    pub base: S::ChildNodeBoxed,
    lparen: S::Token,
    pub arguments: Vec<S::ChildNode>,
    rparen: S::Token,
}

#[derive(Debug, Clone)]
pub struct FieldAccessNode<S: Stage> {
    pub base: S::ChildNodeBoxed,
    period: S::Token,
    pub field: S::IdentifierUnscoped,
}

#[derive(Debug, Clone)]
pub struct PartialCaptureNode<S: Stage> {
    pub identifier: S::ChildNodeBoxed,
    pub arguments: Vec<S::ChildNode>,
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
pub struct ImplBlockNode<S: Stage> {
    impl_keyword: S::Token,
    pub identifier: S::Identifier,
    lbrace: S::Token,
    pub body: Vec<S::ChildNode>,
    rbrace: S::Token,
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
pub struct EnumDeclarationNode<S: Stage> {
    enum_keyword: S::Token,
    pub identifier: S::Identifier,
    lbrace: S::Token,
    pub variants: Vec<EnumVariantNode<S>>,
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
            identifier: NamespacedIdentifier::without_namespace(identifier),
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
pub struct EnumVariantNode<S: Stage> {
    pub location: Location,
    pub identifier: S::Identifier,
    pub comma: Option<S::Token>,
}

impl EnumVariantNode<Parsed> {
    pub(crate) fn new(identifier: Token, comma: Option<Token>) -> Self {
        let location = identifier.location().combine(comma.map(|c| c.location()));
        Self {
            location,
            identifier: NamespacedIdentifier::without_namespace(identifier),
            comma,
        }
    }
}

impl<S: Stage> EnumVariantNode<S> {
    pub fn ends_with_comma(&self) -> bool {
        self.comma.is_some()
    }
}

#[derive(Debug, Clone)]
pub struct ParameterNode<S: Stage> {
    pub location: Location,
    pub identifier: S::Identifier,
    colon: Option<S::Token>,
    pub type_: S::Type,
    comma: Option<S::Token>,
    _marker: PhantomData<S>,
}

impl ParameterNode<Bound> {
    pub fn new(location: Location, identifier: VariableId, type_: TypeId) -> Self {
        Self {
            location,
            identifier,
            colon: None,
            type_,
            comma: None,
            _marker: Default::default(),
        }
    }
}

impl ParameterNode<Parsed> {
    pub(crate) fn new(
        identifier: Token,
        colon: Option<Token>,
        type_identifier: TypeIdentifier,
        comma: Option<Token>,
    ) -> Self {
        let location = identifier
            .location()
            .combine(type_identifier.location())
            .combine(comma.map(|c| c.location()));
        Self {
            location,
            identifier: NamespacedIdentifier::without_namespace(identifier),
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

    pub(crate) fn as_function_declaration(&self) -> Option<&FunctionDeclarationNode<S>> {
        match self {
            Self::FunctionDeclaration(it) => Some(it),
            _ => None,
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
    pub expr: S::ChildNodeBoxed,
    semicolon: S::Token,
}
