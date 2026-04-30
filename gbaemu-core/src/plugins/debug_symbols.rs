use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::registers::RegisterIndex;

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum ArmMode {
    Arm,
    Thumb,
    ThumbToArm,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Location {
    Register(RegisterIndex),
    Memory(RegisterIndex, i32),
}

#[derive(Debug, Serialize, Deserialize, Clone, strum::Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum Type {
    #[strum(serialize = "{0}")]
    Symbol(String),
    #[strum(serialize = "{0}*")]
    Pointer(Box<Type>),
    I16,
    U32,
    U16,
    U8,
    // Like floating point, but not floating.
    D16,
    Bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Parameter {
    pub location: Location,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: Type,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Function {
    pub name: Option<String>,
    pub start: u32,
    pub end: u32,
    pub mode: ArmMode,
    #[serde(default)]
    pub params: Vec<Parameter>,
    #[serde(default, rename = "return")]
    pub return_params: Vec<Parameter>,
}

impl Function {
    pub fn name(&self) -> Cow<'_, str> {
        self.name
            .as_ref()
            .map(|n| n.into())
            .unwrap_or_else(|| format!("unknown_{:04x}", self.start).into())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DebugSymbols {
    pub functions: Vec<Function>,
}

impl DebugSymbols {
    pub fn find_function_for_address(&self, address: u32) -> Option<&Function> {
        self.functions
            .iter()
            .find(|f| f.start <= address && address < f.end)
    }
}
