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

pub(crate) fn evaluate(
    expression: BoundId,
    compiler: &mut Compiler,
    known_constants: Option<&HashMap<VariableId, Value>>,
) -> Option<Value> {
    let mut evaluator = ConstEvaluator::new();
    if let Some(constants) = known_constants {
        for (i, v) in constants {
            evaluator.assign_variable(*i, v.clone());
        }
    }
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
        SyntaxNodeKind::EnumDeclaration(_) => None,
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
        SyntaxNodeKind::PartialCapture(partial_capture_node) => {
            evaluate_partial_capture(&partial_capture_node, compiler, evaluator)
        }
    };

    compiler.nodes.set_constant_value(expression, value.clone());
    value
}

fn evaluate_partial_capture(
    partial_capture_node: &PartialCaptureNode<Bound>,
    compiler: &mut Compiler,
    evaluator: &mut ConstEvaluator,
) -> Option<Value> {
    let base = evaluator.read(partial_capture_node.identifier)?;
    let arguments: Option<Vec<_>> = partial_capture_node
        .arguments
        .iter()
        .map(|a| evaluate_expression(*a, compiler, evaluator))
        .collect();
    let arguments = arguments?;
    let size = arguments.iter().map(|a| a.size()).sum::<usize>() + base.size();
    let ptr = compiler.const_memory.allocate(size);
    let mut wptr = ptr + base.size();
    compiler.const_memory.write_value(ptr, base);
    for argument in arguments {
        let size = argument.size();
        compiler.const_memory.write_value(wptr, argument);
        wptr += size;
    }
    Some(Value::Pointer(ptr))
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
        (ConversionKind::Implicit, base @ Value::Pointer(_), t)
            if compiler.types.as_inner_array_type(t).is_some() =>
        {
            Some(base)
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
        Type::IntegerLiteral(_) | Type::ArrayUnknownLength(_) => {
            unreachable!("Should be resolved!")
        }
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
        Type::Enum(_) => Value::UnsignedInteger32(buf_u32 as _),
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
        let Some(offset) = type_.layout.offset_of(field) else {
            // TODO: Should we clean up after ourselves?
            return Some(Value::Error);
        };
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
        SyntaxNodeKind::Error => {}
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
    let mut arguments = arguments?;
    if base.is_error() || arguments.iter().any(|v| v.is_error()) {
        Some(Value::Error)
    } else {
        let function_type = compiler
            .types
            .as_function_type(type_)
            .expect("Function type");
        match base {
            Value::CompileTimeFunction(id) => {
                assert_eq!(function_type.parameters.len(), arguments.len());
                for (p, a) in function_type.parameters.iter().zip(arguments) {
                    evaluator.assign_variable(*p, a);
                }
                evaluate_expression(id, compiler, evaluator)
            }
            Value::Pointer(ptr) => {
                let mut base = [0; 4];
                compiler.const_memory.read(ptr, &mut base);
                let id = unsafe { BoundId::from_raw(u32::from_le_bytes(base) as usize) };
                let missing = function_type.parameter_types.len() - arguments.len();
                let mut wptr = ptr + 4;
                for t in &function_type.parameter_types[..missing] {
                    let value = read_value(&compiler.const_memory, &mut wptr, *t, &compiler.types);
                    arguments.insert(0, value);
                }
                assert_eq!(function_type.parameters.len(), arguments.len());
                for (p, a) in function_type.parameters.iter().zip(arguments) {
                    evaluator.assign_variable(*p, a);
                }
                evaluate_expression(id, compiler, evaluator)
            }
            _ => unreachable!(),
        }
    }
}

fn read_value(
    const_memory: &crate::memory::Memoryblock<crate::memory::Bound>,
    wptr: &mut usize,
    t: TypeId,
    types: &crate::typing::Types,
) -> Value {
    let size = types.size_of(t);
    let value = match size {
        0 => todo!(),
        1 => {
            let mut buffer = [0];
            const_memory.read(*wptr, &mut buffer);
            match t {
                TypeId::BOOL => Value::Bool(buffer[0] == 1),
                TypeId::UNSIGNED_INTEGER_8 => Value::UnsignedInteger8(buffer[0]),
                _ => unreachable!(),
            }
        }
        2 => {
            let mut buffer = [0; 2];
            const_memory.read(*wptr, &mut buffer);
            match t {
                TypeId::UNSIGNED_INTEGER_16 => Value::UnsignedInteger16(u16::from_le_bytes(buffer)),
                _ => unreachable!(),
            }
        }
        4 => {
            let mut buffer = [0; 4];
            const_memory.read(*wptr, &mut buffer);
            match t {
                TypeId::POINTER => Value::Pointer(u32::from_le_bytes(buffer) as usize),
                TypeId::UNSIGNED_INTEGER_32 => Value::UnsignedInteger32(u32::from_le_bytes(buffer)),
                t if types.as_inner_array_type(t).is_some()
                    || types.as_struct_type(t).is_some() =>
                {
                    Value::Pointer(u32::from_le_bytes(buffer) as usize)
                }
                t if types.as_function_type(t).is_some() => Value::CompileTimeFunction(unsafe {
                    BoundId::from_raw(u32::from_le_bytes(buffer) as _)
                }),
                t if types[t].is_reference() => Value::Pointer(u32::from_le_bytes(buffer) as usize),
                _ => unreachable!(),
            }
        }
        _ => unreachable!(),
    };
    *wptr += size;
    value
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
