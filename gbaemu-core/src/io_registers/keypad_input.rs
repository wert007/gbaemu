use crate::memory::MemoryPlugin;

use super::{read_byte_from_half_word, write_byte_to_half_word};

#[derive(Debug, Default)]
pub struct KeypadInput {
    status: u16,
    interrupt_control: u16,
}

impl MemoryPlugin for KeypadInput {
    fn claims_address(&self, address: usize) -> bool {
        (0x4000130..0x4000134).contains(&address)
    }

    fn read_byte(&self, address: usize) -> u8 {
        let relative_address = address & !0x4000000;
        match relative_address {
            0x130..0x132 => read_byte_from_half_word(self.status, relative_address - 0x130),
            0x132..0x134 => {
                read_byte_from_half_word(self.interrupt_control, relative_address - 0x132)
            }
            _ => unreachable!("Implement read for {relative_address:x}"),
        }
    }

    fn write_byte(&mut self, address: usize, byte: u8) {
        let relative_address = address & !0x4000000;
        match relative_address {
            0x130..0x132 => {
                // TODO: Read-only
            }
            0x132..0x134 => {
                write_byte_to_half_word(
                    &mut self.interrupt_control,
                    relative_address - 0x132,
                    byte,
                );
            }
            _ => unreachable!("Implement write ({byte:x}) for {relative_address:x}"),
        }
    }

    fn reset(&mut self) {}
}
