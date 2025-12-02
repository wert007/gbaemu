use crate::{Compiler, syntax_tree::*, value::Value};

pub(crate) fn evaluate(expression: &mut SyntaxNode<Bound>, compiler: &mut Compiler) {
    if expression.stage.constant_value.is_some() {
        return;
    }
    let value = evaluate_expression(expression, compiler);
    expression.stage.constant_value = value;
}

fn evaluate_expression(expression: &SyntaxNode<Bound>, compiler: &mut Compiler) -> Option<Value> {
    if let Some(value) = expression.stage.constant_value.clone() {
        return Some(value);
    }
    match &expression.kind {
        SyntaxNodeKind::Binary(binary_node) => evaluate_binary(binary_node, compiler),
        SyntaxNodeKind::ArrayLiteral(array_literal_node) => {
            evaluate_array_literal(array_literal_node, compiler)
        }
        _ => None,
    }
}

fn evaluate_array_literal(
    array_literal_node: &ArrayLiteralNode<Bound>,
    compiler: &mut Compiler,
) -> Option<Value> {
    let entries: Option<Vec<Value>> = array_literal_node
        .entries
        .iter()
        .map(|e| evaluate_expression(e, compiler))
        .collect();
    let entries = entries?;
    Some(Value::Array(entries))
}

fn evaluate_binary(binary_node: &BinaryNode<Bound>, compiler: &mut Compiler) -> Option<Value> {
    let lhs = evaluate_expression(&binary_node.lhs, compiler)?;
    let rhs = evaluate_expression(&binary_node.rhs, compiler)?;
    Some(match binary_node.op {
        crate::bind::BoundBinaryOperator::Addition => {
            Value::Integer(lhs.as_int()?.wrapping_add(rhs.as_int()?))
        }
        crate::bind::BoundBinaryOperator::Subtraction => {
            Value::Integer(lhs.as_int()?.wrapping_sub(rhs.as_int()?))
        }
        crate::bind::BoundBinaryOperator::Multiplication => {
            Value::Integer(lhs.as_int()?.wrapping_mul(rhs.as_int()?))
        }
        crate::bind::BoundBinaryOperator::Division => {
            Value::Integer(lhs.as_int()?.wrapping_div(rhs.as_int()?))
        }
        crate::bind::BoundBinaryOperator::Modulo => {
            Value::Integer(lhs.as_int()?.wrapping_rem(rhs.as_int()?))
        }
    })
}
