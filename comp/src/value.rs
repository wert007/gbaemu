use crate::BoundId;

#[derive(Debug, Clone)]
pub enum Value {
    UnsignedInteger32(u32),
    Bool(bool),
    Array(Vec<Value>),
    CompileTimeFunction(BoundId),
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
            Value::CompileTimeFunction(i) => Some(*i),
            _ => None,
        }
    }
}
