#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum Interrupt {
    Vblank = 0,
}
impl Interrupt {
    pub(crate) fn to_bit_index(&self) -> u16 {
        *self as u8 as u16
    }
}
