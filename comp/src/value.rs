use crate::{BoundId, typing::TypeId};

#[derive(Debug, Clone)]
pub enum Value {
    Error,
    UnsignedInteger8(u8),
    UnsignedInteger16(u16),
    UnsignedInteger32(u32),
    Bool(bool),
    Pointer(usize),
    CompileTimeFunction(BoundId),
    DependentOn(BoundId),
    Type(TypeId),
}
impl Value {
    pub(crate) fn as_u32(&self) -> Option<u32> {
        match self {
            Value::UnsignedInteger32(i) => Some(*i),
            _ => None,
        }
    }

    pub(crate) fn as_bound_id(&self) -> Option<BoundId> {
        match self {
            Value::CompileTimeFunction(i) | Value::DependentOn(i) => Some(*i),
            _ => None,
        }
    }

    pub(crate) fn as_usize(&self) -> Option<usize> {
        match self {
            Value::UnsignedInteger8(i) => Some(*i as usize),
            Value::UnsignedInteger16(i) => Some(*i as usize),
            Value::UnsignedInteger32(i) => Some(*i as usize),
            _ => None,
        }
    }

    pub(crate) fn is_error(&self) -> bool {
        matches!(self, Value::Error)
    }

    pub(crate) fn size(&self) -> usize {
        match self {
            Value::Error => 0,
            Value::UnsignedInteger8(_) => 1,
            Value::UnsignedInteger16(_) => 2,
            Value::UnsignedInteger32(_) => 4,
            Value::Bool(_) => 1,
            Value::Pointer(_) => 4,
            Value::CompileTimeFunction(_) => 0,
            Value::DependentOn(_) => 0,
            Value::Type(_) => 0,
        }
    }

    pub(crate) fn as_type(&self) -> Option<TypeId> {
        match self {
            Value::Type(it) => Some(*it),
            _ => None,
        }
    }

    pub(crate) fn as_ptr(&self) -> Option<usize> {
        match self {
            Value::Pointer(it) => Some(*it),
            _ => None,
        }
    }
}
