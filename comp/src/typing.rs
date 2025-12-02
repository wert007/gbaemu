use std::{collections::HashMap, ops::Index};

use crate::{Compiler, StringId, StringInterner};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(usize);

impl TypeId {
    pub const ERROR: TypeId = TypeId(0);
    pub const UNKNOWN: TypeId = TypeId(1);
    pub const VOID: TypeId = TypeId(2);
    pub const TYPE: TypeId = TypeId(3);
    pub const UNSIGNED_INTEGER_8: TypeId = TypeId(4);
    pub const UNSIGNED_INTEGER_16: TypeId = TypeId(5);
    pub const UNSIGNED_INTEGER_32: TypeId = TypeId(6);
    pub const BOOL: TypeId = TypeId(7);
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Error,
    Unknown,
    Void,
    Type,
    UnsignedInteger8,
    UnsignedInteger16,
    UnsignedInteger32,
    Bool,
    Array(TypeId, usize),
    FunctionType(Vec<TypeId>, TypeId),
}

#[derive(Debug)]
pub struct Types {
    types: Vec<Type>,
    names: HashMap<TypeId, StringId>,
}

macro_rules! type_names {
    ($compiler:expr => $($id:ident, $name:literal),*$(,)?) => {
        [$((TypeId::$id, $compiler.intern($name))),*].into_iter().collect()
    };
}

impl Types {
    pub fn new(compiler: &mut Compiler) -> Self {
        let names = type_names!(compiler =>
            VOID, "void",
            UNSIGNED_INTEGER_8, "u8",
            UNSIGNED_INTEGER_16, "u16",
            UNSIGNED_INTEGER_32, "u32",
            BOOL, "bool",
        );
        let result = Self {
            types: vec![
                Type::Error,
                Type::Unknown,
                Type::Void,
                Type::Type,
                Type::UnsignedInteger8,
                Type::UnsignedInteger16,
                Type::UnsignedInteger32,
                Type::Bool,
            ],
            names,
        };
        assert_eq!(result[TypeId::ERROR], Type::Error);
        assert_eq!(result[TypeId::UNKNOWN], Type::Unknown);
        assert_eq!(result[TypeId::VOID], Type::Void);
        assert_eq!(result[TypeId::TYPE], Type::Type);
        assert_eq!(result[TypeId::UNSIGNED_INTEGER_32], Type::UnsignedInteger32);
        assert_eq!(result[TypeId::BOOL], Type::Bool);
        result
    }

    pub fn register(&mut self, type_: Type) -> TypeId {
        if let Some(position) = self.types.iter().position(|t| t == &type_) {
            TypeId(position)
        } else {
            let index = self.types.len();
            self.types.push(type_);
            TypeId(index)
        }
    }

    pub(crate) fn find_by_name(&self, name: crate::StringId) -> Option<TypeId> {
        self.names
            .iter()
            .find(|(_, n)| **n == name)
            .map(|(t, _)| *t)
    }

    pub(crate) fn return_type_of(&self, type_: TypeId) -> Option<TypeId> {
        match self[type_] {
            Type::FunctionType(_, type_) => Some(type_),
            _ => None,
        }
    }

    pub fn display(&self, type_: TypeId) -> String {
        match &self[type_] {
            Type::Error => "#error".into(),
            Type::Unknown => "?unknown".into(),
            Type::Void => "void".into(),
            Type::Type => "type".into(),
            Type::UnsignedInteger8 => "u8".into(),
            Type::UnsignedInteger16 => "u16".into(),
            Type::UnsignedInteger32 => "u32".into(),
            Type::Bool => "bool".into(),
            Type::Array(type_, len) => format!("[{}; {len}]", self.display(*type_)),
            Type::FunctionType(parameter, return_type) => {
                let parameter = parameter.iter().copied().map(|p| self.display(p)).fold(
                    String::new(),
                    |acc, cur| {
                        if acc.is_empty() {
                            cur
                        } else {
                            format!("{acc}, {cur}")
                        }
                    },
                );
                format!("Fn<({parameter}), {}>", self.display(*return_type))
            }
        }
    }
}

impl Index<TypeId> for Types {
    type Output = Type;

    fn index(&self, index: TypeId) -> &Self::Output {
        &self.types[index.0]
    }
}
