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
    GenericType(GenericType),
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

    pub(crate) fn is_enum(&self) -> bool {
        match self {
            Type::Enum(_) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FunctionType {
    // pub name: StringId,
    pub id: TypeId,
    pub identifier: StringId,
    pub parameters: Vec<VariableId>,
    pub parameter_types: Vec<TypeId>,
    pub return_type: TypeId,
}

impl PartialEq for FunctionType {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            || (self.identifier == other.identifier
                && self.parameters == other.parameters
                && self.parameter_types == other.parameter_types
                && self.return_type == other.return_type)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructType {
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
        _variants: &[VariableId],
        _types: &Types,
        _variables: &Variables,
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
        fields.sort_unstable_by_key(|f| usize::MAX - f.1);
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
    active_generic_types: usize,
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
            active_generic_types: 0,
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
            Type::GenericType(generic_type) => {
                write!(f, "{}", &strings[generic_type.name])
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
            Type::IntegerLiteral(_) | Type::ArrayUnknownLength(_) | Type::GenericType(..) => {
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

    pub(crate) fn create_generic_type(&mut self, name: StringId) -> TypeId {
        let t = self.register(Type::GenericType(GenericType {
            name,
            id: self.active_generic_types,
        }));
        self.active_generic_types += 1;
        self.names.insert(t, name);
        t
    }

    pub(crate) fn is_generic(&self, id: TypeId) -> bool {
        !self.is_concrete(id)
    }

    pub(crate) fn make_concrete(&self, id: TypeId) -> TypeId {
        match self[id] {
            Type::IntegerLiteral(_) => TypeId::UNSIGNED_INTEGER_32,
            _ => id,
        }
    }

    pub(crate) fn register_function_type(
        &mut self,
        identifier: StringId,
        parameters: Vec<VariableId>,
        parameter_types: Vec<TypeId>,
        return_type: TypeId,
    ) -> &FunctionType {
        let id = unsafe { self.reserve() };
        let type_ = Type::FunctionType(FunctionType {
            id,
            identifier,
            parameters,
            parameter_types,
            return_type,
        });
        let id = if let Some(position) = self.types.iter().position(|t| t == &type_) {
            self.types.pop();
            TypeId(position)
        } else {
            // let id = unsafe { self.reserve() };
            unsafe { self.set(id, type_) }
            id
        };
        self.as_function_type(id).expect("Was just set to one!")
    }

    pub(crate) fn instantiate(
        &mut self,
        function_type: FunctionType,
        generic_types: HashMap<TypeId, TypeId>,
    ) -> &FunctionType {
        // let parameters = function_type.parameters.into_iter().map(|p| p/)
        let parameter_types = function_type
            .parameter_types
            .into_iter()
            .map(|p| {
                if self.is_generic(p) {
                    generic_types[&p]
                } else {
                    p
                }
            })
            .collect();
        let return_type = if self.is_generic(function_type.return_type) {
            generic_types[&function_type.return_type]
        } else {
            function_type.return_type
        };
        let instantiated = self.register_function_type(
            function_type.identifier,
            function_type.parameters,
            parameter_types,
            return_type,
        );
        instantiated
    }

    pub(crate) fn is_concrete(&self, id: TypeId) -> bool {
        match &self[id] {
            Type::Error
            | Type::Unknown
            | Type::Void
            | Type::IntegerLiteral(_)
            | Type::GenericType(_)
            | Type::ArrayUnknownLength(_)
            | Type::Type => false,
            Type::Pointer
            | Type::UnsignedInteger8
            | Type::UnsignedInteger16
            | Type::Bool
            | Type::UnsignedInteger32 => true,
            Type::Reference(type_id) | Type::Array(type_id, _) => self.is_concrete(*type_id),
            Type::FunctionType(function_type) => {
                function_type
                    .parameter_types
                    .iter()
                    .all(|t| self.is_concrete(*t))
                    && self.is_concrete(function_type.return_type)
            }
            Type::Struct(struct_type) => struct_type
                .fields
                .iter()
                .all(|(.., t)| self.is_concrete(*t)),
            Type::Enum(_enum_type) => true,
        }
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

#[derive(Debug, Clone, PartialEq)]
pub struct GenericType {
    name: StringId,
    id: usize,
}
