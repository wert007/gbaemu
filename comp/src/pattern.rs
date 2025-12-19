use crate::value::Value;

#[derive(Debug, Clone, Copy)]
pub enum Pattern {
    Ignore,
    Constant(Value),
}
impl Pattern {
    pub(crate) fn matches(&self, expression: Value) -> bool {
        match self {
            Pattern::Ignore => true,
            Pattern::Constant(value) => value == &expression,
        }
    }
}
