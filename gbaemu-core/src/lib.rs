use std::{
    collections::VecDeque,
    io::{IsTerminal, Read},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use instructions::{Instruction, InstructionDecodeError, display::DisplayContext};
use io_registers::{GbaIo, lcd::Lcd};
use memory::{Memory, MemoryPlugin, SimpleMemory};
use registers::{Mode, RegisterIndex, RegisterList, Registers};

use crate::{
    interrupts::Interrupt,
    io_registers::lcd::PixelFormat,
    plugins::{Plugin, PluginWishes},
};

mod bitmod;
pub mod instructions;
pub mod interrupts;
pub mod io_registers;
pub mod memory;
pub mod plugins;
pub mod registers;
pub use crate::io_registers::lcd;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
pub struct Cartridge {
    raw: Vec<u8>,
}

impl Cartridge {
    pub fn new(mut reader: impl Read) -> Self {
        let mut raw = Vec::new();
        reader.read_to_end(&mut raw).unwrap();
        Self { raw }
    }

    pub fn normalize_address(mut address: usize) -> usize {
        while address >= 0x0A000000 {
            address -= 0x02000000;
        }
        address
    }
}

impl MemoryPlugin for Cartridge {
    fn claims_address(&self, address: usize) -> bool {
        (0x08000000..0x0A000000).contains(&address)
            || (0x0A000000..0x0C000000).contains(&address)
            || (0x0C000000..0x0E000000).contains(&address)
    }

    fn read_byte(&self, address: usize) -> u8 {
        let address = Self::normalize_address(address);
        let relative_address = address & !0x08000000;
        self.raw.get(relative_address).copied().unwrap_or_default()
    }

    fn write_byte(&mut self, address: usize, byte: u8) {
        let address = Self::normalize_address(address);
        let relative_address = address & !0x08000000;
        self.raw[relative_address] = byte;
    }

    fn reset(&mut self) {}
}

#[derive(Debug, Default, Clone)]
pub struct GbaArgs {
    pub trace_functions: bool,
    // pub silent: bool,
    pub watch_stack: bool,
    pub log_file: Option<PathBuf>,
    pub watch_registers: RegisterList,
}

// pub struct LcdInterface {
//     buffer: [u32; 160 * 240],
// }

// impl LcdInterface {
//     pub fn new(background_mode: DisplayMode, vram: Arc<Mutex<SimpleMemory>>) -> Self {
//         let buffer = Self::create_buffer(background_mode, vram);
//         Self { buffer }
//     }

//     pub fn buffer(&mut self) -> &[u32] {
//         &self.buffer
//     }

//     fn create_buffer(
//         background_mode: DisplayMode,
//         #[allow(dead_code)]

//         vram: Arc<Mutex<SimpleMemory>>,
//     ) -> [u32; 160 * 240] {
//         match background_mode {
//             // TODO: Implement BackgroundMode0
//             DisplayMode::DisplayMode0 => [0; 160 * 240],
//             err => todo!("{err:?}"),
//         }
//     }
// }

#[allow(dead_code)]
pub struct Gba {
    memory: Memory,
    registers: Registers,
    args: GbaArgs,
    ticks: usize,
    wait_ticks: usize,
    gba_io: Arc<Mutex<GbaIo>>,
    vram: Arc<Mutex<SimpleMemory>>,
    flag: bool,
    plugins: Vec<Box<dyn Plugin>>,
    disable_interrupts_since_nothing_is_being_executed: bool,
}

impl Gba {
    // V: 6
    const UNALLOC_MASK: u32 = 0x06F0FC00;
    const USER_MASK: u32 = 0xF80F0200;
    const PRIV_MASK: u32 = 0x000001DF;
    const STATE_MASK: u32 = 0x01000020;

    pub fn new(cartridge: Cartridge) -> Self {
        let vram = Arc::new(Mutex::new(SimpleMemory::new(0x06000000, 0x18000)));
        let color_ram = Arc::new(Mutex::new(SimpleMemory::new(0x05000000, 0x400)));
        let obj_ram = Arc::new(Mutex::new(SimpleMemory::new(0x07000000, 0x400)));
        let gba_io = Arc::new(Mutex::new(GbaIo::new(Lcd::new(
            vram.clone(),
            color_ram.clone(),
            obj_ram.clone(),
        ))));
        Self {
            memory: Memory::new()
                .with_plugin(cartridge.clone())
                .with_plugin(color_ram.clone())
                .with_plugin(vram.clone())
                .with_plugin(obj_ram.clone())
                .with_plugin(gba_io.clone()),
            registers: Registers::new(),
            args: Default::default(),
            ticks: 0,
            wait_ticks: 0,
            gba_io,
            vram,
            flag: false,
            plugins: Vec::new(),
            disable_interrupts_since_nothing_is_being_executed: false,
        }
    }

    pub fn read_memory(&self, at: u32, length: u32) -> Vec<u8> {
        self.memory.read_vec(at, length)
    }

    pub fn with_plugin(&mut self, mut plugin: impl Plugin + 'static) -> &mut Self {
        plugin.with_args(self.args.clone());
        self.plugins.push(Box::new(plugin));
        self
    }

    pub fn run_cycle(&mut self) {
        while self.wait_ticks > 0 {
            self.ticks += 1;
            self.wait_ticks -= 1;
            return;
        }
        self.ticks += 1;
        let registers = self.registers;
        self.gba_io.lock().unwrap().run_cycle(self.ticks);
        self.handle_interrupts();
        let mode = self.registers.cpsr().mode();
        let ip = self.registers.read_raw(RegisterIndex::Ip);
        let instruction = self.fetch().unwrap_or_else(|e| {
            // dbg!(self);
            panic!("Invalid Instruction at {ip}., {e}")
        });
        self.wait_ticks += instruction.tick_duration(self.registers.flags());
        // println!("pc: 0x{ip:08x}");
        let plugin_wishes = self
            .plugins
            .iter_mut()
            .map(|p| p.should_execute(&registers, mode, &instruction, ip, &self.memory))
            .fold(plugins::PluginWishes::default(), |acc, cur| {
                acc.combined_with(cur)
            });
        if plugin_wishes.pause_execution() {
            self.ticks -= 1;
            self.wait_ticks -= instruction.tick_duration(self.registers.flags());
            self.disable_interrupts_since_nothing_is_being_executed = !false;
            return;
        }
        self.disable_interrupts_since_nothing_is_being_executed = !true;

        let increment = if instruction.increments_instruction_pointer(self) {
            0
        } else if self.registers.cpsr().is_thumb() {
            2
        } else {
            4
        };
        instruction.execute(self);
        if !instruction.increments_instruction_pointer(self) {
            self.registers.write(RegisterIndex::Ip, ip + increment);
        }

        for plugin in &mut self.plugins {
            plugin.after_executing(&self.registers, mode, &instruction, ip, &mut self.memory);
        }
    }

    fn fetch(&mut self) -> Result<Instruction, InstructionDecodeError> {
        let ip = self.registers.read_raw(RegisterIndex::Ip);
        if self.registers.cpsr().is_thumb() {
            let ip = ip & !1;
            let half_word = self.memory.read_half_word_at(ip);
            let next_half_word = self.memory.read_half_word_at(ip + 2);
            // println!(
            //     "Thumb Instruction raw: 0x{half_word:04x} (0x{half_word:04x}{next_half_word:04x})"
            // );
            Instruction::decode_thumb(half_word, next_half_word)
        } else {
            let ip = ip & !3;
            let word = self.memory.read_word(ip);
            // println!("ARM Instruction raw: 0x{word:08x}");
            Instruction::decode_arm(word)
        }
    }

    pub fn dump_bios(&self) {
        let bios = include_bytes!("../../assets/gba_bios.bin");
        for (index, word) in bios.chunks_exact(4).enumerate() {
            let word = u32::from_le_bytes([word[0], word[1], word[2], word[3]]);
            let instruction = Instruction::decode_arm(word);
            match instruction {
                Ok(instruction) => println!(
                    "0x{:x}:\t{}",
                    index * 4,
                    instruction.display(DisplayContext {
                        is_tty: std::io::stdout().is_terminal(),
                        ..DisplayContext::default()
                    })
                ),
                Err(_) => println!("0x{:x}:\t0x{word:08x}", index * 4),
            }
        }
    }

    pub fn with_args(mut self, args: impl Into<GbaArgs>) -> Self {
        self.args = args.into();
        self
    }

    pub fn clone_gba_io(&self) -> Arc<Mutex<GbaIo>> {
        self.gba_io.clone()
    }

    pub fn reset(&mut self) {
        self.registers = Registers::new();
        self.gba_io.lock().unwrap().reset();
    }

    pub fn swap_buffers(&self, buffer: &mut [u32; 160 * 240]) {
        self.gba_io.lock().unwrap().lcd.swap_buffers(buffer)
    }

    pub fn load_palette(&self) -> Vec<u32> {
        self.gba_io.lock().unwrap().lcd.load_palette()
    }

    pub fn load_tiles(&self, format: PixelFormat) -> Vec<Vec<u8>> {
        self.gba_io.lock().unwrap().lcd.load_tiles(format)
    }

    pub fn load_tiles_obj(&self, format: PixelFormat) -> Vec<Vec<u8>> {
        self.gba_io.lock().unwrap().lcd.load_tiles_obj(format)
    }

    fn handle_interrupts(&mut self) {
        let Some(interrupt) = self.gba_io.lock().unwrap().interrupt.next(self.ticks) else {
            return;
        };
        let whishes = self
            .plugins
            .iter_mut()
            .filter_map(|p| p.interrupt_occured(interrupt))
            .fold(PluginWishes::default(), |a, c| a.combined_with(c));
        if whishes.pause_execution() {
            // Not quite right, but tbh, also not important right now!
            self.gba_io.lock().unwrap().interrupt.queue(interrupt);
            return;
        }
        let ip = self.registers.read(RegisterIndex::Ip);
        // let instruction = self.fetch().unwrap_or_else(|e| {
        //     // dbg!(self);
        //     panic!("Invalid Instruction at {ip}., {e}")
        // });
        let psr = self.registers.read(RegisterIndex::Cpsr);
        self.registers.cpsr_mut().set_mode(Mode::Irq);
        self.registers.write(RegisterIndex::Spsr, psr);
        self.registers.cpsr_mut().set_is_thumb(false);
        self.registers.write(RegisterIndex::Lr, ip);
        self.registers.write(RegisterIndex::Ip, 0x00000018);
    }
}
