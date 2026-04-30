use dma::Dma;
use interrupt_waitstate::InterruptWaitstate;
use keypad_input::KeypadInput;
use lcd::Lcd;
use serial_communication::SerialCommunication;
use sound::Sound;
use timer::Timer;

use crate::{interrupts::Interrupt, lcd::PixelFormat, memory::MemoryPlugin};

pub mod dma;
pub mod interrupt_waitstate;
pub mod keypad_input;
pub mod lcd;
pub mod serial_communication;
pub mod sound;
pub mod timer;

#[derive(Debug)]
pub struct GbaIo {
    pub lcd: Lcd,
    sound: Sound,
    dma: Dma,
    timer: Timer,
    serial_com: SerialCommunication,
    keypad: KeypadInput,
    pub interrupt: InterruptWaitstate,
}

impl GbaIo {
    pub(crate) fn new(lcd: Lcd) -> Self {
        Self {
            lcd,
            sound: Default::default(),
            dma: Default::default(),
            timer: Default::default(),
            serial_com: Default::default(),
            keypad: Default::default(),
            interrupt: Default::default(),
        }
    }

    pub fn load_tiles(&self, format: PixelFormat) -> Vec<Vec<u8>> {
        self.lcd.load_tiles(format)
    }

    pub fn load_palette(&self) -> Vec<u32> {
        self.lcd.load_palette()
    }

    pub fn run_cycle(&mut self, tick: usize) {
        let mut result = Vec::with_capacity(8);

        result.append(&mut self.lcd.run_cycle(tick));
        result.append(&mut self.serial_com.run_cycle(tick));
        for interrupt in result {
            self.interrupt.queue(interrupt);
        }
        // result
    }

    pub fn find_for_address(&self, address: usize) -> Option<&dyn MemoryPlugin> {
        if self.lcd.claims_address(address) {
            Some(&self.lcd)
        } else if self.interrupt.claims_address(address) {
            Some(&self.interrupt)
        } else if self.sound.claims_address(address) {
            Some(&self.sound)
        } else if self.dma.claims_address(address) {
            Some(&self.dma)
        } else if self.timer.claims_address(address) {
            Some(&self.timer)
        } else if self.serial_com.claims_address(address) {
            Some(&self.serial_com)
        } else if self.keypad.claims_address(address) {
            Some(&self.keypad)
        } else {
            None
        }
    }

    pub fn find_for_address_mut(&mut self, address: usize) -> Option<&mut dyn MemoryPlugin> {
        if self.lcd.claims_address(address) {
            Some(&mut self.lcd)
        } else if self.interrupt.claims_address(address) {
            Some(&mut self.interrupt)
        } else if self.sound.claims_address(address) {
            Some(&mut self.sound)
        } else if self.dma.claims_address(address) {
            Some(&mut self.dma)
        } else if self.timer.claims_address(address) {
            Some(&mut self.timer)
        } else if self.serial_com.claims_address(address) {
            Some(&mut self.serial_com)
        } else if self.keypad.claims_address(address) {
            Some(&mut self.keypad)
        } else {
            None
        }
    }
}

impl MemoryPlugin for GbaIo {
    fn claims_address(&self, address: usize) -> bool {
        // assert_eq!(self.find_for_address(address).is_some(), unsafe {
        //     (self as *const Self as *mut Self)
        //         .as_mut()
        //         .unwrap()
        //         .find_for_address_mut(address)
        //         .is_some()
        // });
        self.find_for_address(address).is_some()
    }

    fn read_byte(&self, address: usize) -> u8 {
        self.find_for_address(address)
            .map(|p| p.read_byte(address))
            .expect("Invalid address")
    }

    fn write_byte(&mut self, address: usize, byte: u8) {
        self.find_for_address_mut(address)
            .map(|p| p.write_byte(address, byte))
            .expect("Invalid address")
    }
}

fn read_byte_from_half_word(source: u16, byte_index: usize) -> u8 {
    (match byte_index {
        0 => source & 0x00ff,
        1 => (source & 0xff00) >> 8,
        _ => unreachable!("byte_index must be below 2!"),
    }) as u8
}

fn read_byte_from_word(source: u32, byte_index: usize) -> u8 {
    (match byte_index {
        0 => source & 0x00ff,
        1 => (source & 0xff00) >> 8,
        2 => (source & 0xff0000) >> 16,
        3 => (source & 0xff000000) >> 24,
        _ => unreachable!("byte_index must be below 4!"),
    }) as u8
}

fn write_byte_to_half_word(destination: &mut u16, byte_index: usize, byte: u8) {
    let byte = byte as u16;
    match byte_index {
        0 => *destination = (*destination & 0xff00) | byte,
        1 => *destination = (*destination & 0x00ff) | (byte << 8),
        _ => unreachable!("byte_index must be below 2!"),
    }
}

fn write_byte_to_word(destination: &mut u32, byte_index: usize, byte: u8) {
    let byte = byte as u32;
    match byte_index {
        0 => *destination = (*destination & 0xffff_ff00) | byte,
        1 => *destination = (*destination & 0xffff_00ff) | (byte << 8),
        2 => *destination = (*destination & 0xff00_ffff) | (byte << 16),
        3 => *destination = (*destination & 0x00ff_ffff) | (byte << 24),
        _ => unreachable!("byte_index must be below 4!"),
    }
}
