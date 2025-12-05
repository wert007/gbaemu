use crate::typing::{Type, TypeId, Types};

#[derive(Debug, Clone, Copy)]
pub enum ConversionKind {
    Implicit,
}
impl ConversionKind {
    pub(crate) fn convert(&self, base_type: TypeId, expected: TypeId, types: &mut Types) -> bool {
        if base_type == expected || base_type == TypeId::ERROR || expected == TypeId::UNKNOWN {
            true
        } else {
            if let &Type::IntegerLiteral(value) = &types[base_type] {
                match expected {
                    TypeId::UNSIGNED_INTEGER_8 if value <= u8::MAX as usize => true,
                    TypeId::UNSIGNED_INTEGER_16 if value <= u16::MAX as usize => true,
                    TypeId::UNSIGNED_INTEGER_32 if value <= u32::MAX as usize => true,
                    _ => false,
                }
            } else {
                false
            }
        }
    }
}
