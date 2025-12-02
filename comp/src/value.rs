use crate::BoundId;

#[derive(Debug, Clone)]
pub enum Value {
    UnsignedInteger8(u8),
    UnsignedInteger16(u16),
    UnsignedInteger32(u32),
    Bool(bool),
    Array(Vec<Value>),
    CompileTimeFunction(BoundId),
    DependentOn(BoundId),
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
}
