use crate::{
    Compiler,
    bind::{BoundId, BoundTree},
    syntax_tree::*,
    value::Value,
};

pub(crate) fn evaluate(
    expression: BoundId,
    compiler: &mut Compiler,
    tree: &mut BoundTree,
) -> Option<Value> {
    if let Some(value) = tree.constant_value(expression) {
        return Some(value.clone());
    }
    let value = evaluate_expression(expression, compiler, &*tree);
    tree.set_constant_value(expression, value.clone());
    value
}

fn evaluate_expression(
    expression: BoundId,
    compiler: &mut Compiler,
    tree: &BoundTree,
) -> Option<Value> {
    if let Some(value) = tree.constant_value(expression) {
        return Some(value.clone());
    }
    match &tree[expression].kind {
        SyntaxNodeKind::Binary(binary_node) => evaluate_binary(binary_node, compiler, tree),
        SyntaxNodeKind::ArrayLiteral(array_literal_node) => {
            evaluate_array_literal(array_literal_node, compiler, tree)
        }
        SyntaxNodeKind::FunctionCall(function_call_node) => {
            evaluate_function_call(function_call_node, compiler, tree)
        }
        SyntaxNodeKind::BlockExpression(block_expression_node) => {
            evaluate_block_expression(block_expression_node, compiler, tree)
        }
        SyntaxNodeKind::ExpressionStatement(expression_statement_node) => {
            evaluate_expression_statement(expression_statement_node, compiler, tree)
        }
        _ => None,
    }
}

fn evaluate_expression_statement(
    expression_statement_node: &ExpressionStatementNode<Bound>,
    compiler: &mut Compiler,
    tree: &BoundTree,
) -> Option<Value> {
    let value = evaluate_expression(expression_statement_node.expression, compiler, tree)?;
    if expression_statement_node.semicolon.is_some() {
        None
    } else {
        Some(value)
    }
}

fn evaluate_block_expression(
    block_expression_node: &BlockExpressionNode<Bound>,
    compiler: &mut Compiler,
    tree: &BoundTree,
) -> Option<Value> {
    let mut result = None;
    for e in &block_expression_node.body {
        result = evaluate_expression(*e, compiler, tree);
    }
    result
}

fn evaluate_function_call(
    function_call_node: &FunctionCallNode<Bound>,
    compiler: &mut Compiler,
    tree: &BoundTree,
) -> Option<Value> {
    let base = evaluate_expression(function_call_node.base, compiler, tree)?;
    let arguments: Option<Vec<Value>> = function_call_node
        .arguments
        .iter()
        .copied()
        .map(|a| evaluate_expression(a, compiler, tree))
        .collect();
    let arguments = arguments?;
    assert_eq!(arguments.len(), 0);
    evaluate_expression(base.as_bound_id().unwrap(), compiler, tree)
}

fn evaluate_array_literal(
    array_literal_node: &ArrayLiteralNode<Bound>,
    compiler: &mut Compiler,
    tree: &BoundTree,
) -> Option<Value> {
    let entries: Option<Vec<Value>> = array_literal_node
        .entries
        .iter()
        .copied()
        .map(|e| evaluate_expression(e, compiler, tree))
        .collect();
    let entries = entries?;
    Some(Value::Array(entries))
}

fn evaluate_binary(
    binary_node: &BinaryNode<Bound>,
    compiler: &mut Compiler,
    tree: &BoundTree,
) -> Option<Value> {
    let lhs = evaluate_expression(binary_node.lhs, compiler, tree)?;
    let rhs = evaluate_expression(binary_node.rhs, compiler, tree)?;
    Some(match binary_node.op {
        crate::bind::BoundBinaryOperator::Addition => {
            Value::UnsignedInteger32(lhs.as_u32()?.wrapping_add(rhs.as_u32()?))
        }
        crate::bind::BoundBinaryOperator::Subtraction => {
            Value::UnsignedInteger32(lhs.as_u32()?.wrapping_sub(rhs.as_u32()?))
        }
        crate::bind::BoundBinaryOperator::Multiplication => {
            Value::UnsignedInteger32(lhs.as_u32()?.wrapping_mul(rhs.as_u32()?))
        }
        crate::bind::BoundBinaryOperator::Division => {
            Value::UnsignedInteger32(lhs.as_u32()?.wrapping_div(rhs.as_u32()?))
        }
        crate::bind::BoundBinaryOperator::Modulo => {
            Value::UnsignedInteger32(lhs.as_u32()?.wrapping_rem(rhs.as_u32()?))
        }
    })
}
