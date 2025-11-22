use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Function {
    pub(super) name: Option<String>,
    pub(super) start: u32,
    pub(super) end: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DebugSymbols {
    pub(super) functions: Vec<Function>,
}
