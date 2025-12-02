#[derive(Debug, Clone)]
pub enum Value {
    Integer(i64),
}
impl Value {
    pub(crate) fn as_int(&self) -> Option<i64> {
        match self {
            Value::Integer(i) => Some(*i),
            _ => None,
        }
    }
}
