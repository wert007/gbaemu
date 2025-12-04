use std::{collections::HashMap, ops::Index};

use crate::{Location, StringId, StringInterner, bind::VariableId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(usize);

impl TypeId {
    pub const ERROR: TypeId = TypeId(0);
    pub const UNKNOWN: TypeId = TypeId(1);
    pub const VOID: TypeId = TypeId(2);
    pub const TYPE: TypeId = TypeId(3);
    pub const POINTER: TypeId = TypeId(4);
    pub const UNSIGNED_INTEGER_8: TypeId = TypeId(5);
    pub const UNSIGNED_INTEGER_16: TypeId = TypeId(6);
    pub const UNSIGNED_INTEGER_32: TypeId = TypeId(7);
    pub const BOOL: TypeId = TypeId(8);
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Error,
    Unknown,
    Void,
    Type,
    Pointer,
    UnsignedInteger8,
    UnsignedInteger16,
    UnsignedInteger32,
    Bool,
    Array(TypeId, usize),
    FunctionType(Vec<TypeId>, TypeId),
    Struct(StructType),
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructType {
    pub name: StringId,
    pub identifier: VariableId,
    pub fields: Vec<(Location, StringId, TypeId)>,
    pub layout: StructLayout,
}
impl StructType {
    pub(crate) fn get_field_type_by_name(&self, identifier: StringId) -> Option<TypeId> {
        self.fields.iter().find(|f| f.1 == identifier).map(|f| f.2)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructLayout {
    size: usize,
    offsets: HashMap<StringId, usize>,
}
impl StructLayout {
    pub(crate) fn size(&self) -> usize {
        self.size
    }

    pub(crate) fn offset_of(&self, field: StringId) -> Option<usize> {
        self.offsets.get(&field).copied()
    }

    pub(crate) fn from_fields(
        fields: &[(Location, StringId, TypeId)],
        types: &Types,
    ) -> StructLayout {
        if fields.is_empty() {
            return StructLayout::empty();
        }
        let mut fields: Vec<_> = fields
            .iter()
            .map(|(_, s, t)| (s, types.size_of(*t)))
            .collect();
        fields.sort_unstable_by_key(|f| f.1);
        assert!(fields.first().unwrap().1 >= fields.last().unwrap().1);
        let mut size = 0;
        let mut offsets = HashMap::new();
        for (name, field_size) in fields {
            offsets.insert(*name, size);
            size += field_size
        }
        StructLayout { size, offsets }
    }

    fn empty() -> StructLayout {
        Self {
            size: 0,
            offsets: HashMap::new(),
        }
    }
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
    pub fn new(compiler: &mut StringInterner) -> Self {
        let names = type_names!(compiler =>
            VOID, "void",
            UNSIGNED_INTEGER_8, "u8",
            UNSIGNED_INTEGER_16, "u16",
            UNSIGNED_INTEGER_32, "u32",
            BOOL, "bool",
            POINTER, "ptr",
        );
        let result = Self {
            types: vec![
                Type::Error,
                Type::Unknown,
                Type::Void,
                Type::Type,
                Type::Pointer,
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
        assert_eq!(result[TypeId::POINTER], Type::Pointer);
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

    pub(crate) fn fmt_type(
        &self,
        type_: TypeId,
        f: &mut std::fmt::Formatter<'_>,
        strings: &StringInterner,
    ) -> Result<(), std::fmt::Error> {
        match &self[type_] {
            Type::Error => write!(f, "#error"),
            Type::Unknown => write!(f, "?unknown"),
            Type::Void => write!(f, "void"),
            Type::Type => write!(f, "type"),
            Type::Pointer => write!(f, "ptr"),
            Type::UnsignedInteger8 => write!(f, "u8"),
            Type::UnsignedInteger16 => write!(f, "u16"),
            Type::UnsignedInteger32 => write!(f, "u32"),
            Type::Bool => write!(f, "bool"),
            Type::Array(type_, len) => {
                write!(f, "[")?;
                self.fmt_type(*type_, f, strings)?;
                write!(f, "; {len}]")
            }
            Type::FunctionType(parameter, return_type) => {
                write!(f, "Fn<(")?;
                for (i, parameter) in parameter.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    self.fmt_type(*parameter, f, strings)?;
                }
                write!(f, "), ")?;
                self.fmt_type(*return_type, f, strings)?;
                write!(f, ">")
            }
            Type::Struct(struct_) => {
                write!(f, "{}", &strings[struct_.name])
            }
        }
    }

    pub(crate) fn as_struct_type(&self, id: TypeId) -> Option<&StructType> {
        match &self[id] {
            Type::Struct(it) => Some(it),
            _ => None,
        }
    }

    fn size_of(&self, id: TypeId) -> usize {
        match &self[id] {
            Type::Error => 0,
            Type::Unknown => 0,
            Type::Void => 0,
            Type::Type => 0,
            Type::Pointer => 4,
            Type::UnsignedInteger8 => 1,
            Type::UnsignedInteger16 => 2,
            Type::UnsignedInteger32 => 4,
            Type::Bool => 1,
            Type::Array(type_id, len) => self.size_of(*type_id) * len,
            Type::FunctionType(type_ids, type_id) => 0,
            Type::Struct(struct_type) => struct_type.layout.size(),
        }
    }

    pub(crate) fn field_type(
        &self,
        base_type: TypeId,
        field_identifier: StringId,
    ) -> Option<TypeId> {
        match &self[base_type] {
            Type::Struct(struct_type) => struct_type.get_field_type_by_name(field_identifier),
            _ => None,
        }
    }
}

impl Index<TypeId> for Types {
    type Output = Type;

    fn index(&self, index: TypeId) -> &Self::Output {
        &self.types[index.0]
    }
}
