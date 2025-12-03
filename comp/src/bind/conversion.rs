use crate::typing::{TypeId, Types};

#[derive(Debug, Clone, Copy)]
pub enum ConversionKind {
    Implicit,
}
impl ConversionKind {
    pub(crate) fn convert(&self, base_type: TypeId, expected: TypeId, types: &mut Types) -> bool {
        if base_type == expected || base_type == TypeId::ERROR || expected == TypeId::UNKNOWN {
            true
        } else {
            false
        }
    }
}
