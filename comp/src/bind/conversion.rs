use crate::{
    StringInterner,
    typing::{Type, TypeId, Types},
};

#[derive(Debug, Clone, Copy)]
pub enum ConversionKind {
    Implicit,
    Instantiation,
}
impl ConversionKind {
    pub(crate) fn convert(&self, base_type: TypeId, expected: TypeId, types: &mut Types) -> bool {
        match self {
            ConversionKind::Implicit => implicit_conversion(base_type, expected, types),
            ConversionKind::Instantiation => instantiation(base_type, expected, types),
        }
    }
}

fn instantiation(base_type: TypeId, expected: TypeId, types: &mut Types) -> bool {
    if base_type == expected || base_type == TypeId::ERROR || expected == TypeId::UNKNOWN {
        true
    } else {
        types.is_generic(base_type) && types.is_concrete(expected)
    }
}

fn implicit_conversion(base_type: TypeId, expected: TypeId, types: &mut Types) -> bool {
    if base_type == expected || base_type == TypeId::ERROR || expected == TypeId::UNKNOWN {
        true
    } else {
        match &types[base_type] {
            &Type::IntegerLiteral(value) => match expected {
                TypeId::UNSIGNED_INTEGER_8 if value <= u8::MAX as usize => true,
                TypeId::UNSIGNED_INTEGER_16 if value <= u16::MAX as usize => true,
                TypeId::UNSIGNED_INTEGER_32 if value <= u32::MAX as usize => true,
                _ => false,
            },
            Type::Array(inner, _) => types.as_inner_array_type(expected) == Some(*inner),
            _ => false,
        }
    }
}
