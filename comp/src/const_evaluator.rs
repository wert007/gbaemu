use std::collections::HashMap;

use crate::{
    BoundId, Compiler, StringId,
    bind::conversion::ConversionKind,
    syntax_tree::*,
    typing::{Type, TypeId},
    value::Value,
    variables::VariableId,
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
    let value = match compiler.nodes[expression].kind.clone() {
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
        SyntaxNodeKind::Identifier(identifier) => evaluate_identifier(identifier, evaluator),
        SyntaxNodeKind::Error => Some(Value::Error),
        SyntaxNodeKind::Program(_) => None,
        SyntaxNodeKind::ConstDeclaration(_) => None,
        SyntaxNodeKind::ImplBlock(_) => None,
        SyntaxNodeKind::Literal(_) => unreachable!(),
        SyntaxNodeKind::CommaedExpression((expression, _)) => {
            evaluate_expression(expression, compiler, evaluator)
        }
        SyntaxNodeKind::FunctionDeclaration(_) => None,
        SyntaxNodeKind::Conversion(conversion_node) => evaluate_conversion(
            conversion_node,
            compiler.nodes.type_of(expression),
            compiler,
            evaluator,
        ),
        SyntaxNodeKind::StructDeclaration(_) => None,
        SyntaxNodeKind::StructLiteral(struct_literal_node) => evaluate_struct_literal(
            &struct_literal_node,
            compiler.nodes.type_of(expression),
            compiler,
            evaluator,
        ),
        SyntaxNodeKind::FieldAccess(field_access_node) => evaluate_field_access(
            &field_access_node,
            compiler.nodes.type_of(expression),
            compiler,
            evaluator,
        ),
    };

    compiler.nodes.set_constant_value(expression, value.clone());
    value
}

fn evaluate_conversion(
    conversion_node: ConversionNode<Bound>,
    target_type: TypeId,
    compiler: &mut Compiler,
    evaluator: &mut ConstEvaluator,
) -> Option<Value> {
    let base = evaluate_expression(conversion_node.base, compiler, evaluator)?;
    match (conversion_node.conversion_kind, base, target_type) {
        (ConversionKind::Implicit, Value::UnsignedInteger32(v), TypeId::UNSIGNED_INTEGER_8) => {
            Some(Value::UnsignedInteger8(v as _))
        }
        (ConversionKind::Implicit, Value::UnsignedInteger32(v), TypeId::UNSIGNED_INTEGER_16) => {
            Some(Value::UnsignedInteger16(v as _))
        }
        (ConversionKind::Implicit, Value::UnsignedInteger32(v), TypeId::UNSIGNED_INTEGER_32) => {
            Some(Value::UnsignedInteger32(v as _))
        }
        _ => unreachable!("Impossible conversion!"),
    }
}

fn evaluate_field_access(
    field_access_node: &FieldAccessNode<Bound>,
    type_: TypeId,
    compiler: &mut Compiler,
    evaluator: &mut ConstEvaluator,
) -> Option<Value> {
    let base_type = compiler.nodes.type_of(field_access_node.base);
    let Some(base) = evaluate_expression(field_access_node.base, compiler, evaluator)?.as_ptr()
    else {
        return Some(Value::Error);
    };
    let Some(offset) = compiler
        .types
        .as_struct_type(base_type)
        .unwrap()
        .layout
        .offset_of(field_access_node.field)
    else {
        return Some(Value::Error);
    };
    let mut buffer = [0u8; 4];
    compiler.const_memory.read(base + offset, &mut buffer);
    let buf_u8 = buffer[0];
    let buf_u16 = u16::from_le_bytes(buffer.as_chunks::<2>().0[0]);
    let buf_u32 = u32::from_le_bytes(buffer);
    Some(match &compiler.types[type_] {
        Type::IntegerLiteral(_) => unreachable!("Should be resolved!"),
        Type::Error => Value::Error,
        Type::Unknown => Value::Error,
        Type::Void => Value::Error,
        Type::Type => Value::Error,
        Type::FunctionType(..) => Value::Error,
        Type::Reference(_) => Value::Error,
        Type::Bool => Value::Bool(buf_u8 == 1),
        Type::UnsignedInteger8 => Value::UnsignedInteger8(buf_u8),
        Type::UnsignedInteger16 => Value::UnsignedInteger16(buf_u16),
        Type::Array(..) | Type::Struct(_) | Type::Pointer => Value::Pointer(buf_u32 as _),
        Type::UnsignedInteger32 => Value::UnsignedInteger32(buf_u32 as _),
    })
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

fn evaluate_identifier(identifier: VariableId, evaluator: &mut ConstEvaluator) -> Option<Value> {
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
    let type_ = compiler.nodes.type_of(function_call_node.base);
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
        let function_type = compiler
            .types
            .as_function_type(type_)
            .expect("Function type");
        assert_eq!(function_type.parameters.len(), arguments.len());
        for (p, a) in function_type.parameters.iter().zip(arguments) {
            evaluator.assign_variable(*p, a);
        }
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
            match (
                compiler.nodes.type_of(binary_node.lhs),
                compiler.nodes.type_of(binary_node.rhs),
            ) {
                (TypeId::UNSIGNED_INTEGER_32, TypeId::UNSIGNED_INTEGER_32) => {
                    Value::UnsignedInteger32(lhs.as_u32()?.wrapping_add(rhs.as_u32()?))
                }
                (TypeId::UNSIGNED_INTEGER_16, TypeId::UNSIGNED_INTEGER_16) => {
                    Value::UnsignedInteger16(lhs.as_u16()?.wrapping_add(rhs.as_u16()?))
                }
                (TypeId::UNSIGNED_INTEGER_8, TypeId::UNSIGNED_INTEGER_8) => {
                    Value::UnsignedInteger8(lhs.as_u8()?.wrapping_add(rhs.as_u8()?))
                }
                _ => todo!("Unexpected operand types!"),
            }
        }
        crate::bind::BoundBinaryOperator::Subtraction => {
            match (
                compiler.nodes.type_of(binary_node.lhs),
                compiler.nodes.type_of(binary_node.rhs),
            ) {
                (TypeId::UNSIGNED_INTEGER_32, TypeId::UNSIGNED_INTEGER_32) => {
                    Value::UnsignedInteger32(lhs.as_u32()?.wrapping_sub(rhs.as_u32()?))
                }
                (TypeId::UNSIGNED_INTEGER_16, TypeId::UNSIGNED_INTEGER_16) => {
                    Value::UnsignedInteger16(lhs.as_u16()?.wrapping_sub(rhs.as_u16()?))
                }
                (TypeId::UNSIGNED_INTEGER_8, TypeId::UNSIGNED_INTEGER_8) => {
                    Value::UnsignedInteger8(lhs.as_u8()?.wrapping_sub(rhs.as_u8()?))
                }
                _ => todo!("Unexpected operand types!"),
            }
        }
        crate::bind::BoundBinaryOperator::Multiplication => {
            match (
                compiler.nodes.type_of(binary_node.lhs),
                compiler.nodes.type_of(binary_node.rhs),
            ) {
                (TypeId::UNSIGNED_INTEGER_32, TypeId::UNSIGNED_INTEGER_32) => {
                    Value::UnsignedInteger32(lhs.as_u32()?.wrapping_mul(rhs.as_u32()?))
                }
                (TypeId::UNSIGNED_INTEGER_16, TypeId::UNSIGNED_INTEGER_16) => {
                    Value::UnsignedInteger16(lhs.as_u16()?.wrapping_mul(rhs.as_u16()?))
                }
                (TypeId::UNSIGNED_INTEGER_8, TypeId::UNSIGNED_INTEGER_8) => {
                    Value::UnsignedInteger8(lhs.as_u8()?.wrapping_mul(rhs.as_u8()?))
                }
                _ => todo!("Unexpected operand types!"),
            }
        }
        crate::bind::BoundBinaryOperator::Division => {
            match (
                compiler.nodes.type_of(binary_node.lhs),
                compiler.nodes.type_of(binary_node.rhs),
            ) {
                (TypeId::UNSIGNED_INTEGER_32, TypeId::UNSIGNED_INTEGER_32) => {
                    Value::UnsignedInteger32(lhs.as_u32()?.wrapping_div(rhs.as_u32()?))
                }
                (TypeId::UNSIGNED_INTEGER_16, TypeId::UNSIGNED_INTEGER_16) => {
                    Value::UnsignedInteger16(lhs.as_u16()?.wrapping_div(rhs.as_u16()?))
                }
                (TypeId::UNSIGNED_INTEGER_8, TypeId::UNSIGNED_INTEGER_8) => {
                    Value::UnsignedInteger8(lhs.as_u8()?.wrapping_div(rhs.as_u8()?))
                }
                _ => todo!("Unexpected operand types!"),
            }
        }
        crate::bind::BoundBinaryOperator::Modulo => {
            match (
                compiler.nodes.type_of(binary_node.lhs),
                compiler.nodes.type_of(binary_node.rhs),
            ) {
                (TypeId::UNSIGNED_INTEGER_32, TypeId::UNSIGNED_INTEGER_32) => {
                    Value::UnsignedInteger32(lhs.as_u32()?.wrapping_rem(rhs.as_u32()?))
                }
                (TypeId::UNSIGNED_INTEGER_16, TypeId::UNSIGNED_INTEGER_16) => {
                    Value::UnsignedInteger16(lhs.as_u16()?.wrapping_rem(rhs.as_u16()?))
                }
                (TypeId::UNSIGNED_INTEGER_8, TypeId::UNSIGNED_INTEGER_8) => {
                    Value::UnsignedInteger8(lhs.as_u8()?.wrapping_rem(rhs.as_u8()?))
                }
                _ => todo!("Unexpected operand types!"),
            }
        }
    })
}
