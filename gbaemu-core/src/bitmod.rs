pub trait BitModifications {
    fn bit(&self, bit: usize) -> bool;
}

impl BitModifications for u32 {
    fn bit(&self, bit: usize) -> bool {
        (self & 1 << bit) > 0
    }
}

impl BitModifications for u16 {
    fn bit(&self, bit: usize) -> bool {
        (self & 1 << bit) > 0
    }
}
