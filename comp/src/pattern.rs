use crate::{
    BoundId, Compiler, bind::Binder, intern_namespaced_identifier, syntax_tree::*, typing::TypeId,
    value::Value,
};

#[derive(Debug, Clone, Copy)]
pub enum Pattern {
    Error,
    Ignore,
    Constant(Value),
}
impl Pattern {
    pub(crate) fn matches(&self, expression: Value) -> bool {
        match self {
            Pattern::Error => false,
            Pattern::Ignore => true,
            Pattern::Constant(value) => value == &expression,
        }
    }
}

pub(crate) fn bind(
    pattern: SyntaxNode<Parsed>,
    type_: TypeId,
    compiler: &mut Compiler,
    binder: &mut Binder,
) -> Pattern {
    match pattern.kind {
        SyntaxNodeKind::Program(_)
        | SyntaxNodeKind::ConstDeclaration(_)
        | SyntaxNodeKind::EnumDeclaration(_)
        | SyntaxNodeKind::Conversion(_)
        | SyntaxNodeKind::MatchExpression(_)
        | SyntaxNodeKind::BlockExpression(_)
        | SyntaxNodeKind::ImplBlock(_)
        | SyntaxNodeKind::FunctionDeclaration(_)
        | SyntaxNodeKind::ExpressionStatement(_)
        | SyntaxNodeKind::FieldAccess(_)
        | SyntaxNodeKind::AssignmentStatement(_)
        | SyntaxNodeKind::StructDeclaration(_)
        | SyntaxNodeKind::Error => Pattern::Error,
        SyntaxNodeKind::Literal(token) => bind_literal(token, type_, compiler, binder),
        SyntaxNodeKind::Identifier(namespaced_identifier) => {
            bind_identifier(namespaced_identifier, type_, compiler, binder)
        }
        SyntaxNodeKind::Binary(binary_node) => todo!(),
        SyntaxNodeKind::CommaedExpression(_) => todo!(),
        SyntaxNodeKind::ArrayLiteral(array_literal_node) => todo!(),
        SyntaxNodeKind::FunctionCall(function_call_node) => todo!(),
        SyntaxNodeKind::StructLiteral(struct_literal_node) => todo!(),
        SyntaxNodeKind::PartialCapture(partial_capture_node) => todo!(),
    }
}

fn bind_literal(
    token: crate::lexer::Token,
    type_: TypeId,
    compiler: &mut Compiler,
    binder: &mut Binder,
) -> Pattern {
    let value = binder
        .bind_literal(token, type_, compiler, BoundId(usize::MAX))
        .stage
        .constant_value
        .expect("literal!");
    Pattern::Constant(value)
}

fn bind_identifier(
    namespaced_identifier: NamespacedIdentifier,
    type_: TypeId,
    compiler: &mut Compiler,
    binder: &mut Binder,
) -> Pattern {
    let (namespaces, name) = intern_namespaced_identifier(namespaced_identifier, compiler);
    if let Some(variable) = compiler.variables.find_by_name(&namespaces, name) {
        if let Some(value) = binder.look_up_constant(variable.id) {
            Pattern::Constant(value)
        } else {
            todo!("Support error reporting? Or variable declaration, not sure tbh..")
        }
    } else {
        todo!("Support variable declaration!")
    }
}
