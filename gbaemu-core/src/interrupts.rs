#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum Interrupt {
    Vblank = 0,
}
impl Interrupt {
    pub const COUNT: usize = 1;
    pub(crate) fn to_bit_index(&self) -> u16 {
        *self as u8 as u16
    }

    pub(crate) fn min_fire_cooldown(&self) -> usize {
        match self {
            Interrupt::Vblank => 279666,
        }
    }
}
