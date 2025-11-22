use crate::memory::MemoryPlugin;

use super::*;
#[derive(Debug, Default)]
pub struct Timer {
    timer: [SingleTimer; 4],
}

#[derive(Debug, Default)]
struct SingleTimer {
    counter_reload: u16,
    control: u16,
}

impl MemoryPlugin for SingleTimer {
    fn claims_address(&self, address: usize) -> bool {
        (0x4000100..0x4000120).contains(&address)
    }

    fn read_byte(&self, address: usize) -> u8 {
        let relative_address = address & !0x4000000;
        match relative_address {
            0x0..0x2 => read_byte_from_half_word(self.counter_reload, relative_address - 0x0),
            0x2..0x4 => read_byte_from_half_word(self.control, relative_address - 0x2),
            _ => unreachable!("Implement read for {relative_address:x}"),
        }
    }

    fn write_byte(&mut self, address: usize, byte: u8) {
        let relative_address = address & !0x4000000;
        match relative_address {
            0x0..0x2 => {
                write_byte_to_half_word(&mut self.counter_reload, relative_address - 0x0, byte)
            }
            0x2..0x4 => write_byte_to_half_word(&mut self.control, relative_address - 0x2, byte),
            _ => unreachable!("Implement write ({byte:x}) for {relative_address:x}"),
        }
    }
}

impl MemoryPlugin for Timer {
    fn claims_address(&self, address: usize) -> bool {
        (0x4000100..0x4000120).contains(&address)
    }

    fn read_byte(&self, address: usize) -> u8 {
        if (0x4000110..0x4000120).contains(&address) {
            return 0;
        }
        let relative_address = address & !0x4000100;
        let index = relative_address / 4;
        self.timer[index].read_byte(relative_address % 4)
    }

    fn write_byte(&mut self, address: usize, byte: u8) {
        if (0x4000110..0x4000120).contains(&address) {
            return;
        }
        let relative_address = address & !0x4000100;
        let index = relative_address / 4;

        self.timer[index].write_byte(relative_address % 4, byte);
    }
}
