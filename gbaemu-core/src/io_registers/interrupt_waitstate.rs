use std::fmt::Debug;

use crate::{interrupts::Interrupt, memory::MemoryPlugin};

use super::{read_byte_from_half_word, write_byte_to_half_word};

///4000206h       -    -         Not used
///400020Ah       -    -         Not used
///4000302h       -    -         Not used
///4000410h  ?    ?    ?         Undocumented - Purpose Unknown / Bug ??? 0FFh
///4000411h       -    -         Not used
///4000804h       -    -         Not used
#[derive(Default)]
#[allow(dead_code)]
pub struct InterruptWaitstate {
    ///4000200h  2    R/W  IE        Interrupt Enable Register
    interrupt_enable: u16,
    ///4000202h  2    R/W  IF        Interrupt Request Flags / IRQ Acknowledge
    interrupt_request_flags: u16,
    ///4000204h  2    R/W  WAITCNT   Game Pak Waitstate Control
    game_pak_waitstate: u16,
    ///4000208h  2    R/W  IME       Interrupt Master Enable Register
    interrupt_master_enable: u16,
    ///4000300h  1    R/W  POSTFLG   Undocumented - Post Boot Flag
    post_boot_flag: u8,
    ///4000301h  1    W    HALTCNT   Undocumented - Power Down Control
    power_down_control: u8,
    ///4000800h  4    R/W  ?         Undocumented - Internal Memory Control (R/W)
    ///4xx0800h  4    R/W  ?         Mirrors of 4000800h (repeated each 64K)
    internal_memory_control: u32,
}

impl InterruptWaitstate {
    pub(crate) fn should_raise_interrupt(&self, interrupt: Interrupt) -> bool {
        if self.interrupt_master_enable & 1 == 0 {
            return false;
        }
        (self.interrupt_enable & (1 << interrupt.to_bit_index())) > 0
    }
}

impl Debug for InterruptWaitstate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InterruptWaitstate")
            .field(
                "interrupt_enable",
                &format_args!("{:#x}", self.interrupt_enable),
            )
            .field(
                "interrupt_request_flags",
                &format_args!("{:#x}", self.interrupt_request_flags),
            )
            .field(
                "game_pak_waitstate",
                &format_args!("{:#x}", self.game_pak_waitstate),
            )
            .field(
                "interrupt_master_enable",
                &format_args!("{:#x}", self.interrupt_master_enable),
            )
            .field(
                "post_boot_flag",
                &format_args!("{:#x}", self.post_boot_flag),
            )
            .field(
                "power_down_control",
                &format_args!("{:#x}", self.power_down_control),
            )
            .field(
                "internal_memory_control",
                &format_args!("{:#x}", self.internal_memory_control),
            )
            .finish()
    }
}

impl MemoryPlugin for InterruptWaitstate {
    fn claims_address(&self, address: usize) -> bool {
        (0x4000200..=0x4700000).contains(&address)
    }

    fn read_byte(&self, address: usize) -> u8 {
        let relative_address = address & !0x4000000;
        match relative_address {
            0x200..0x202 => {
                read_byte_from_half_word(self.interrupt_enable, relative_address - 0x200)
            }
            0x202..0x204 => {
                read_byte_from_half_word(self.interrupt_request_flags, relative_address - 0x202)
            }

            0x204..0x206 => {
                read_byte_from_half_word(self.game_pak_waitstate, relative_address - 0x204)
            }
            0x300 => self.post_boot_flag,
            _ => todo!("Interrupt read_byte at {address:x}"),
        }
    }

    fn write_byte(&mut self, address: usize, byte: u8) {
        let relative_address = address & !0x4000000;
        match relative_address {
            0x200..0x202 => {
                write_byte_to_half_word(&mut self.interrupt_enable, relative_address - 0x200, byte)
            }
            0x202..0x204 => write_byte_to_half_word(
                &mut self.interrupt_request_flags,
                relative_address - 0x202,
                byte,
            ),
            0x204..0x206 => write_byte_to_half_word(
                &mut self.game_pak_waitstate,
                relative_address - 0x204,
                byte,
            ),
            0x206..0x208 => {}
            0x208..0x20A => write_byte_to_half_word(
                &mut self.interrupt_master_enable,
                relative_address - 0x208,
                byte,
            ),
            0x20A..0x300 => {}
            0x300 => self.post_boot_flag = byte,
            0x301 => self.power_down_control = byte,
            0x410 => {
                // Ignore, this is probably a bug in the gba.
            }
            _ => todo!("Interrupt write_byte at {address:x}, with byte = {byte:x}"),
        }
    }
}
