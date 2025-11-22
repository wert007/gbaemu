use std::fmt::Debug;

use super::*;
use crate::memory::MemoryPlugin;

pub struct SerialCommunication {
    data: [u16; 4],
    control_register: u16,
    data_2: u16,
    mode_select_general_purpose: u16,
    bus_control: u16,
    bus_receive_data: u32,
    bus_transmit_data: u32,
    bus_receive_status: u16,
}

impl Default for SerialCommunication {
    fn default() -> Self {
        Self {
            data: [0xffff; 4],
            control_register: Default::default(),
            data_2: 0xffff,
            mode_select_general_purpose: Default::default(),
            bus_control: Default::default(),
            bus_receive_data: Default::default(),
            bus_transmit_data: Default::default(),
            bus_receive_status: Default::default(),
        }
    }
}
impl SerialCommunication {
    fn read_data(&self, relative_address: usize) -> u8 {
        if relative_address > 8 {
            read_byte_from_half_word(self.data_2, relative_address - 0xa)
        } else {
            let index = relative_address / 2;
            let byte_index = relative_address % 2;
            read_byte_from_half_word(self.data[index], byte_index)
        }
    }

    fn write_data(&mut self, relative_address: usize, byte: u8) {
        if relative_address > 8 {
            write_byte_to_half_word(&mut self.data_2, relative_address - 0xa, byte);
        } else {
            let index = relative_address / 2;
            let byte_index = relative_address % 2;
            write_byte_to_half_word(&mut self.data[index], byte_index, byte);
        }
    }
}

impl Debug for SerialCommunication {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SerialCommunication")
            .field("data", &self.data)
            .field(
                "control_register",
                &format_args!("{:#x}", self.control_register),
            )
            .field("data_2", &format_args!("{:#x}", self.data_2))
            .field(
                "mode_select_general_purpose",
                &format_args!("{:#x}", self.mode_select_general_purpose),
            )
            .field("bus_control", &format_args!("{:#x}", self.bus_control))
            .field("bus_receive_data", &self.bus_receive_data)
            .field("bus_transmit_data", &self.bus_transmit_data)
            .field(
                "bus_receive_status",
                &format_args!("{:#x}", self.bus_receive_status),
            )
            .finish()
    }
}

impl MemoryPlugin for SerialCommunication {
    fn claims_address(&self, address: usize) -> bool {
        (0x4000120..0x4000130).contains(&address) || (0x4000134..0x4000200).contains(&address)
    }

    fn read_byte(&self, address: usize) -> u8 {
        let relative_address = address & !0x4000000;
        match relative_address {
            0x120..0x128 => self.read_data(relative_address - 0x120),
            0x128..0x12a => {
                read_byte_from_half_word(self.control_register, relative_address - 0x128)
            }
            0x12a..0x12c => self.read_data(relative_address - 0x120),
            0x134..0x136 => {
                // todo!("Where are we?");
                read_byte_from_half_word(self.mode_select_general_purpose, relative_address - 0x134)
            }
            0x12c..0x130 => 0,
            0x138..0x140 => 0,
            0x140..0x142 => read_byte_from_half_word(self.bus_control, relative_address - 0x140),
            0x142..0x150 => 0,
            0x150..0x154 => read_byte_from_word(self.bus_receive_data, relative_address - 0x150),
            0x154..0x158 => read_byte_from_word(self.bus_transmit_data, relative_address - 0x154),
            0x158..0x15a => {
                read_byte_from_half_word(self.bus_receive_status, relative_address - 0x158)
            }
            0x15a..0x200 => 0,
            _ => unreachable!("Implement read for {relative_address:x}"),
        }
    }

    fn write_byte(&mut self, address: usize, byte: u8) {
        let relative_address = address & !0x4000000;
        match relative_address {
            0x120..0x128 => self.write_data(relative_address - 0x120, byte),
            0x128..0x12a => {
                write_byte_to_half_word(&mut self.control_register, relative_address - 0x128, byte);
                if self.control_register & 0x40 > 0 {
                    todo!("IRQ expected??")
                }
            }
            0x12a..0x12c => self.write_data(relative_address - 0x120, byte),
            0x12c..0x130 => {}
            0x134..0x136 => write_byte_to_half_word(
                &mut self.mode_select_general_purpose,
                relative_address - 0x134,
                byte,
            ),
            0x136..0x138 => {
                // NOTE: Here would be the infrared sensor!
            }
            0x138..0x140 => {}
            0x140..0x142 => {
                write_byte_to_half_word(&mut self.bus_control, relative_address - 0x140, byte);
            }
            0x142..0x150 => {}
            0x150..0x154 => {
                write_byte_to_word(&mut self.bus_receive_data, relative_address - 0x150, byte);
            }
            0x154..0x158 => {
                write_byte_to_word(&mut self.bus_transmit_data, relative_address - 0x154, byte);
            }
            0x158..0x15a => {
                // todo!("Is write actually implemented for this??")
            }
            0x15a..0x200 => {}
            _ => unreachable!("Implement write ({byte:x}) for {relative_address:x}"),
        }
    }
}
