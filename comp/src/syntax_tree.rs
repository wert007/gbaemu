#![allow(dead_code)]
use crate::bind::{BoundBinaryOperator, VariableId};
use crate::{HasLocation, Location, lexer::Token, typing::TypeId, value::Value};
use std::fmt::Debug;

#[derive(Debug, Clone)]
pub struct SyntaxTree<S: Stage> {
    pub node: SyntaxNode<S>,
}

#[derive(Debug, Clone, Copy)]
pub struct Parsed;
#[derive(Debug, Clone)]
pub struct Bound {
    pub type_: TypeId,
    pub constant_value: Option<Value>,
}

pub trait Stage: Debug + Clone {
    type Variable: Debug + Clone;
    type Token: Debug + Clone;
    type Value: Debug + Clone;
    type BinaryOp: Debug + Clone;
}

impl Stage for Parsed {
    type Variable = Token;
    type Token = Token;
    type Value = Box<SyntaxNode<Parsed>>;
    type BinaryOp = Token;
}

impl Stage for Bound {
    type Variable = VariableId;
    type Token = ();
    type Value = Value;
    type BinaryOp = BoundBinaryOperator;
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
    pub fn const_declaration(location: Location, variable: VariableId, value: Value) -> Self {
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
                type_: TypeId::VOID,
                constant_value: None,
            },
        }
    }
    pub fn literal(literal: Token, value: Value, type_: TypeId) -> Self {
        Self {
            location: literal.location(),
            kind: SyntaxNodeKind::Literal(literal),
            stage: Bound {
                type_,
                constant_value: Some(value),
            },
        }
    }

    pub fn error(location: Location) -> Self {
        Self {
            location,
            kind: SyntaxNodeKind::Error,
            stage: Bound {
                type_: TypeId::ERROR,
                constant_value: None,
            },
        }
    }

    pub(crate) fn program(
        top_level_statements: Vec<SyntaxNode<Bound>>,
        location: Location,
    ) -> SyntaxNode<Bound> {
        Self {
            location,
            kind: SyntaxNodeKind::Program(ProgramNode {
                top_level_statements,
                eof: (),
            }),
            stage: Bound {
                type_: TypeId::VOID,
                constant_value: None,
            },
        }
    }

    pub(crate) fn binary(
        lhs: SyntaxNode<Bound>,
        op: BoundBinaryOperator,
        rhs: SyntaxNode<Bound>,
        type_: TypeId,
    ) -> SyntaxNode<Bound> {
        let location = lhs.location().combine(rhs.location());
        Self {
            location,
            kind: SyntaxNodeKind::Binary(BinaryNode {
                lhs: Box::new(lhs),
                op,
                rhs: Box::new(rhs),
            }),
            stage: Bound {
                type_,
                constant_value: None,
            },
        }
    }

    pub(crate) fn variable(
        location: Location,
        variable: &crate::bind::VariableDeclaration,
        constant_value: Option<Value>,
    ) -> SyntaxNode<Bound> {
        Self {
            location,
            kind: SyntaxNodeKind::Identifier(variable.id),
            stage: Bound {
                type_: variable.type_,
                constant_value,
            },
        }
    }

    pub(crate) fn array_literal(
        entries: Vec<SyntaxNode<Bound>>,
        type_: TypeId,
        location: Location,
    ) -> SyntaxNode<Bound> {
        Self {
            location,
            kind: SyntaxNodeKind::ArrayLiteral(ArrayLiteralNode {
                lbracket: (),
                entries,
                rbracket: (),
            }),
            stage: Bound {
                type_,
                constant_value: None,
            },
        }
    }
}

impl SyntaxNode<Parsed> {
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
}

#[derive(Debug, Clone)]
pub enum SyntaxNodeKind<S: Stage> {
    Error,
    Program(ProgramNode<S>),
    ConstDeclaration(ConstDeclarationNode<S>),
    Literal(Token),
    Identifier(S::Variable),
    Binary(BinaryNode<S>),
    CommaedExpression((Box<SyntaxNode<S>>, Option<Token>)),
    ArrayLiteral(ArrayLiteralNode<S>),
}

impl<S: Stage> SyntaxNodeKind<S> {
    pub fn ends_with_comma(&self) -> bool {
        match self {
            SyntaxNodeKind::CommaedExpression((_, c)) => c.is_some(),
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArrayLiteralNode<S: Stage> {
    lbracket: S::Token,
    pub entries: Vec<SyntaxNode<S>>,
    rbracket: S::Token,
}

#[derive(Debug, Clone)]
pub struct BinaryNode<S: Stage> {
    pub lhs: Box<SyntaxNode<S>>,
    pub op: S::BinaryOp,
    pub rhs: Box<SyntaxNode<S>>,
}

#[derive(Debug, Clone)]
pub struct ProgramNode<S: Stage> {
    pub top_level_statements: Vec<SyntaxNode<S>>,
    eof: S::Token,
}

#[derive(Debug, Clone)]
pub struct ConstDeclarationNode<S: Stage> {
    const_keyword: S::Token,
    pub identifier: S::Variable,
    equals: S::Token,
    pub expr: S::Value,
    semicolon: S::Token,
}
