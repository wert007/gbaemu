use std::{collections::HashMap, fmt::Display, ops::Index};

use crate::{
    Location, StringId, StringInterner,
    variables::{VariableId, Variables},
};

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

    pub(crate) unsafe fn from_raw(t: usize) -> TypeId {
        Self(t)
    }
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
    IntegerLiteral(usize),
    Bool,
    Array(TypeId, usize),
    ArrayUnknownLength(TypeId),
    FunctionType(FunctionType),
    Struct(StructType),
    Reference(TypeId),
    Enum(EnumType),
}
impl Type {
    pub(crate) fn is_integer_literal(&self) -> bool {
        match self {
            Type::IntegerLiteral(_) => true,
            _ => false,
        }
    }

    pub(crate) fn is_reference(&self) -> bool {
        match self {
            Type::Reference(_) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionType {
    // pub name: StringId,
    pub identifier: StringId,
    pub parameters: Vec<VariableId>,
    pub parameter_types: Vec<TypeId>,
    pub return_type: TypeId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructType {
    pub identifier: VariableId,
    pub fields: Vec<(Location, StringId, TypeId)>,
    pub layout: StructLayout,
    pub associated_functions: Vec<(VariableId, TypeId)>,
}
impl StructType {
    pub(crate) fn get_field_type_by_name(&self, identifier: StringId) -> Option<TypeId> {
        self.fields.iter().find(|f| f.1 == identifier).map(|f| f.2)
    }

    pub(crate) fn get_associated_function_by_name(
        &self,
        identifier: StringId,
    ) -> Option<(VariableId, TypeId)> {
        self.associated_functions
            .iter()
            .find(|f| f.0.1 == identifier)
            .copied()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumType {
    pub identifier: VariableId,
    pub variants: Vec<VariableId>,
    pub layout: StructLayout,
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

    pub fn from_variants(
        variants: &[VariableId],
        types: &Types,
        variables: &Variables,
    ) -> StructLayout {
        Self {
            size: 4,
            offsets: HashMap::new(),
        }
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
    pub(crate) types: Vec<Type>,
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
            Type::IntegerLiteral(_) => write!(f, "{{integer literal}}"),
            Type::Bool => write!(f, "bool"),
            Type::Array(type_, len) => {
                write!(f, "[")?;
                self.fmt_type(*type_, f, strings)?;
                write!(f, "; {len}]")
            }
            Type::ArrayUnknownLength(type_) => {
                write!(f, "[")?;
                self.fmt_type(*type_, f, strings)?;
                write!(f, "; ?]")
            }
            Type::FunctionType(ft) => {
                write!(f, "Fn<(")?;
                for (i, parameter) in ft.parameter_types.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    self.fmt_type(*parameter, f, strings)?;
                }
                write!(f, "), ")?;
                self.fmt_type(ft.return_type, f, strings)?;
                write!(f, ">")
            }
            Type::Struct(struct_) => {
                write!(f, "{}", &strings[struct_.identifier.1])
            }
            Type::Enum(enum_) => {
                write!(f, "{}", &strings[enum_.identifier.1])
            }
            Type::Reference(reference) => {
                write!(f, "&")?;
                self.fmt_type(*reference, f, strings)
            }
        }
    }

    pub(crate) fn as_struct_type(&self, id: TypeId) -> Option<&StructType> {
        match &self[id] {
            Type::Struct(it) => Some(it),
            Type::Reference(t) => self.as_struct_type(*t),
            _ => None,
        }
    }

    pub(crate) fn as_struct_type_mut(&mut self, id: TypeId) -> Option<&mut StructType> {
        match &mut self.types[id.0] {
            Type::Struct(it) => Some(it),
            _ => None,
        }
    }

    pub fn size_of(&self, id: TypeId) -> usize {
        match &self[id] {
            Type::Error => 0,
            Type::Unknown => 0,
            Type::Void => 0,
            Type::Type => 0,
            Type::Reference(_) | Type::Pointer => 4,
            Type::UnsignedInteger8 => 1,
            Type::UnsignedInteger16 => 2,
            Type::UnsignedInteger32 => 4,
            Type::Bool => 1,
            Type::Array(type_id, len) => self.size_of(*type_id) * len,
            Type::FunctionType(..) => 4,
            Type::Struct(struct_type) => struct_type.layout.size(),
            Type::Enum(enum_type) => enum_type.layout.size(),
            Type::IntegerLiteral(_) | Type::ArrayUnknownLength(_) => {
                unreachable!("This should be unreachable?")
            }
        }
    }

    pub(crate) fn field_type(
        &self,
        base_type: TypeId,
        field_identifier: StringId,
    ) -> Option<TypeId> {
        match &self[base_type] {
            Type::Reference(inner) => self.field_type(*inner, field_identifier),
            Type::Struct(struct_type) => struct_type.get_field_type_by_name(field_identifier),
            _ => None,
        }
    }

    pub(crate) fn associated_function_type(
        &self,
        base_type: TypeId,
        field_identifier: StringId,
    ) -> Option<(VariableId, TypeId)> {
        match &self[base_type] {
            Type::Reference(inner) => self.associated_function_type(*inner, field_identifier),
            Type::Struct(struct_type) => {
                struct_type.get_associated_function_by_name(field_identifier)
            }
            _ => None,
        }
    }

    pub(crate) fn as_function_type(&self, id: TypeId) -> Option<&FunctionType> {
        match &self[id] {
            Type::FunctionType(f) => Some(f),
            _ => None,
        }
    }

    pub fn to_string(&self, id: TypeId, strings: &StringInterner) -> String {
        format!(
            "{}",
            TypeToString {
                types: self,
                strings,
                id,
            }
        )
    }

    pub(crate) fn as_inner_array_type(&self, id: TypeId) -> Option<TypeId> {
        match &self[id] {
            Type::Array(t, _) => Some(*t),
            Type::ArrayUnknownLength(t) => Some(*t),
            _ => None,
        }
    }

    /// Can only be called once before Self::set. Otherwise ids will be
    /// duplicated.
    pub unsafe fn reserve(&mut self) -> TypeId {
        let id = TypeId(self.types.len());
        self.types.push(Type::Error);
        id
    }

    pub unsafe fn set(&mut self, id: TypeId, type_: Type) {
        self.types[id.0] = type_;
    }
}

impl Index<TypeId> for Types {
    type Output = Type;

    fn index(&self, index: TypeId) -> &Self::Output {
        &self.types[index.0]
    }
}

struct TypeToString<'a, 'b> {
    types: &'a Types,
    strings: &'b StringInterner,
    id: TypeId,
}

impl Display for TypeToString<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.types.fmt_type(self.id, f, self.strings)
    }
}
