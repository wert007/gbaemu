use std::collections::HashMap;

use crate::{
    BoundId, Compiler, StringId, bind::VariableId, syntax_tree::*, typing::TypeId, value::Value,
};

#[derive(Debug)]
pub struct ConstEvaluator {
    variables: HashMap<VariableId, Value>,
}
impl ConstEvaluator {
    fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    fn assign_variable(&mut self, variable: VariableId, value: Value) {
        self.variables.insert(variable, value);
    }

    fn read(&self, identifier: VariableId) -> Option<Value> {
        self.variables.get(&identifier).cloned()
    }
}

pub(crate) fn evaluate(expression: BoundId, compiler: &mut Compiler) -> Option<Value> {
    let mut evaluator = ConstEvaluator::new();
    let mut value = evaluate_expression(expression, compiler, &mut evaluator);
    if let Some(Value::DependentOn(node)) = value {
        compiler.nodes.set_constant_value(expression, None);
        compiler.nodes.set_constant_value(node, None);
        evaluate_expression(node, compiler, &mut evaluator);
        value = evaluate_expression(expression, compiler, &mut evaluator);
    }
    compiler.nodes.set_constant_value(expression, value.clone());
    value
}

fn evaluate_expression(
    expression: BoundId,
    compiler: &mut Compiler,
    evaluator: &mut ConstEvaluator,
) -> Option<Value> {
    if let Some(value) = compiler.nodes.constant_value(expression) {
        return Some(value.clone());
    }
    match compiler.nodes[expression].kind.clone() {
        SyntaxNodeKind::Binary(binary_node) => evaluate_binary(&binary_node, compiler, evaluator),
        SyntaxNodeKind::ArrayLiteral(array_literal_node) => {
            evaluate_array_literal(&array_literal_node, compiler, evaluator)
        }
        SyntaxNodeKind::FunctionCall(function_call_node) => {
            evaluate_function_call(&function_call_node, compiler, evaluator)
        }
        SyntaxNodeKind::BlockExpression(block_expression_node) => {
            evaluate_block_expression(&block_expression_node, compiler, evaluator)
        }
        SyntaxNodeKind::ExpressionStatement(expression_statement_node) => {
            evaluate_expression_statement(&expression_statement_node, compiler, evaluator)
        }
        SyntaxNodeKind::AssignmentStatement(assignment_statement_node) => {
            evaluate_assignment_statement(&assignment_statement_node, compiler, evaluator)
        }
        SyntaxNodeKind::Identifier(identifier) => {
            evaluate_identifier(identifier, compiler, evaluator)
        }
        SyntaxNodeKind::Error => Some(Value::Error),
        SyntaxNodeKind::Program(_) => None,
        SyntaxNodeKind::ConstDeclaration(_) => None,
        SyntaxNodeKind::Literal(_) => unreachable!(),
        SyntaxNodeKind::CommaedExpression((expression, _)) => {
            evaluate_expression(expression, compiler, evaluator)
        }
        SyntaxNodeKind::FunctionDeclaration(_) => None,
        SyntaxNodeKind::Conversion(conversion_node) => todo!(),
        SyntaxNodeKind::StructDeclaration(_) => None,
        SyntaxNodeKind::StructLiteral(struct_literal_node) => evaluate_struct_literal(
            &struct_literal_node,
            compiler.nodes.type_of(expression),
            compiler,
            evaluator,
        ),
    }
}

fn evaluate_struct_literal(
    struct_literal_node: &StructLiteralNode<Bound>,
    type_: TypeId,
    compiler: &mut Compiler,
    evaluator: &mut ConstEvaluator,
) -> Option<Value> {
    let fields: Option<Vec<(StringId, Value)>> = struct_literal_node
        .fields
        .iter()
        .map(|f| {
            let value = evaluate_expression(f.expression, compiler, evaluator)?;
            Some((f.identifier, value))
        })
        .collect();
    let type_ = compiler.types.as_struct_type(type_).unwrap();
    let fields = fields?;
    let base = compiler.const_memory.allocate(type_.layout.size());
    for (field, value) in fields {
        let offset = type_.layout.offset_of(field).unwrap();
        compiler.const_memory.write_value(base + offset, value);
    }
    Some(Value::Pointer(base))
}

fn evaluate_identifier(
    identifier: VariableId,
    compiler: &mut Compiler,
    evaluator: &mut ConstEvaluator,
) -> Option<Value> {
    evaluator.read(identifier)
}

fn evaluate_assignment_statement(
    assignment_statement_node: &AssignmentStatementNode<Bound>,
    compiler: &mut Compiler,
    evaluator: &mut ConstEvaluator,
) -> Option<Value> {
    let value = evaluate_expression(assignment_statement_node.value, compiler, evaluator)?;
    evaluate_assign_to_lhs(assignment_statement_node.lhs, compiler, evaluator, value);
    None
}

fn evaluate_assign_to_lhs(
    lhs: BoundId,
    compiler: &mut Compiler,
    evaluator: &mut ConstEvaluator,
    value: Value,
) {
    match &compiler.nodes[lhs].kind {
        SyntaxNodeKind::Identifier(identifier) => {
            evaluator.assign_variable(*identifier, value);
        }
        unexpected => unreachable!("unexpected lhs for assignmen: {unexpected:#?}"),
    }
}

fn evaluate_expression_statement(
    expression_statement_node: &ExpressionStatementNode<Bound>,
    compiler: &mut Compiler,
    evaluator: &mut ConstEvaluator,
) -> Option<Value> {
    let value = evaluate_expression(expression_statement_node.expression, compiler, evaluator)?;
    if expression_statement_node.semicolon.is_some() {
        None
    } else {
        Some(value)
    }
}

fn evaluate_block_expression(
    block_expression_node: &BlockExpressionNode<Bound>,
    compiler: &mut Compiler,
    evaluator: &mut ConstEvaluator,
) -> Option<Value> {
    let mut result = None;
    for e in &block_expression_node.body {
        result = evaluate_expression(*e, compiler, evaluator);
    }
    result
}

fn evaluate_function_call(
    function_call_node: &FunctionCallNode<Bound>,
    compiler: &mut Compiler,
    evaluator: &mut ConstEvaluator,
) -> Option<Value> {
    let base = evaluate_expression(function_call_node.base, compiler, evaluator)?;
    let arguments: Option<Vec<Value>> = function_call_node
        .arguments
        .iter()
        .copied()
        .map(|a| evaluate_expression(a, compiler, evaluator))
        .collect();
    let arguments = arguments?;
    if base.is_error() || arguments.iter().any(|v| v.is_error()) {
        Some(Value::Error)
    } else {
        assert_eq!(arguments.len(), 0);
        evaluate_expression(base.as_bound_id().unwrap(), compiler, evaluator)
    }
}

fn evaluate_array_literal(
    array_literal_node: &ArrayLiteralNode<Bound>,
    compiler: &mut Compiler,
    evaluator: &mut ConstEvaluator,
) -> Option<Value> {
    let entries: Option<Vec<Value>> = array_literal_node
        .entries
        .iter()
        .copied()
        .map(|e| evaluate_expression(e, compiler, evaluator))
        .collect();
    let entries = entries?;
    if entries.iter().any(|v| v.is_error()) {
        Some(Value::Error)
    } else {
        let size = entries.iter().map(|v| v.size()).sum();
        let ptr = compiler.const_memory.allocate(size);
        let mut writing_ptr = ptr;
        for entry in entries {
            let size = entry.size();
            compiler.const_memory.write_value(writing_ptr, entry);
            writing_ptr += size;
        }
        Some(Value::Pointer(ptr))
    }
}

fn evaluate_binary(
    binary_node: &BinaryNode<Bound>,
    compiler: &mut Compiler,
    evaluator: &mut ConstEvaluator,
) -> Option<Value> {
    let lhs = evaluate_expression(binary_node.lhs, compiler, evaluator)?;
    let rhs = evaluate_expression(binary_node.rhs, compiler, evaluator)?;
    if lhs.is_error() || rhs.is_error() {
        return Some(Value::Error);
    }
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
