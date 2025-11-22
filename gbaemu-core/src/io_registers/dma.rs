use super::write_byte_to_half_word;
use crate::{
    io_registers::{read_byte_from_half_word, write_byte_to_word},
    memory::MemoryPlugin,
};

#[derive(Debug, Default)]
pub struct Dma {
    channels: [DmaChannel; 4],
}

impl Dma {
    fn address_to_index(address: usize) -> usize {
        match address {
            0x40000B0..0x40000BC => 0,
            0x40000BC..0x40000C8 => 1,
            0x40000C8..0x40000D4 => 2,
            0x40000D4..0x40000E0 => 3,
            _ => unreachable!("Invalid dma address! {address:x}"),
        }
    }
}

impl MemoryPlugin for Dma {
    fn claims_address(&self, address: usize) -> bool {
        (0x40000B0..0x4000100).contains(&address)
    }

    fn read_byte(&self, address: usize) -> u8 {
        if (0x40000e0..0x4000100).contains(&address) {
            0
        } else {
            self.channels[Self::address_to_index(address)].read_byte(address)
        }
    }

    fn write_byte(&mut self, address: usize, byte: u8) {
        if (0x40000e0..0x4000100).contains(&address) {
            return;
        }
        self.channels[Self::address_to_index(address)].write_byte(address, byte);
    }
}

#[derive(Debug, Default)]
pub struct DmaChannel {
    source_address: u32,
    destination_address: u32,
    word_count: u16,
    control: u16,
}

impl DmaChannel {
    fn normalize_address(address: usize) -> usize {
        match address {
            0x40000B0..0x40000BC => address,
            0x40000BC..0x40000C8 => address - 0x40000BC + 0x40000B0,
            0x40000C8..0x40000D4 => address - 0x40000C8 + 0x40000B0,
            0x40000D4..0x40000E0 => address - 0x40000D4 + 0x40000B0,
            _ => unreachable!("Invalid dma address! {address:x}"),
        }
    }
}

impl MemoryPlugin for DmaChannel {
    fn claims_address(&self, address: usize) -> bool {
        (0x40000B0..0x4000100).contains(&address)
    }

    fn read_byte(&self, address: usize) -> u8 {
        let address = Self::normalize_address(address) & !0x4000000;
        match address {
            0xb0..0xba => 0,
            0xBA..0xBC => read_byte_from_half_word(self.control, address - 0xBA),
            _ => todo!("{address:x}"),
        }
    }

    fn write_byte(&mut self, address: usize, byte: u8) {
        let address = Self::normalize_address(address) & !0x4000000;
        match address {
            0xB0..0xb4 => write_byte_to_word(&mut self.source_address, address - 0xb0, byte),
            0xb4..0xb8 => write_byte_to_word(&mut self.destination_address, address - 0xb4, byte),
            0xB8..0xBA => write_byte_to_half_word(&mut self.word_count, address - 0xB8, byte),
            0xBA..0xBC => write_byte_to_half_word(&mut self.control, address - 0xBA, byte),
            0xe0..0x100 => {}
            _ => todo!("{address:x}"),
        }
    }
}
