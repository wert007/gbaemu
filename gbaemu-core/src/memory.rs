use std::{
    collections::VecDeque,
    ptr,
    sync::{Arc, Mutex},
};

pub struct SimpleMemory {
    data: Vec<u8>,
    start: usize,
}
impl SimpleMemory {
    pub(crate) fn new(start: usize, size: usize) -> Self {
        Self {
            data: vec![0; size],
            start,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}

impl MemoryPlugin for SimpleMemory {
    fn claims_address(&self, address: usize) -> bool {
        (self.start..self.start + self.data.len()).contains(&address)
    }

    fn reset(&mut self) {}

    fn read_byte(&self, address: usize) -> u8 {
        self.data[address - self.start]
    }

    fn write_byte(&mut self, address: usize, byte: u8) {
        self.data[address - self.start] = byte;
    }
}

pub trait MemoryPlugin {
    fn claims_address(&self, address: usize) -> bool;
    fn reset(&mut self);
    fn read_slice(&self, address: usize, length: usize) -> Vec<u8> {
        (address..(address + length))
            .map(|a| self.read_byte(a))
            .collect()
    }
    fn read_word(&self, address: usize) -> u32 {
        u32::from_le_bytes([
            self.read_byte(address),
            self.read_byte(address + 1),
            self.read_byte(address + 2),
            self.read_byte(address + 3),
        ])
    }
    fn read_half_word(&self, address: usize) -> u16 {
        u16::from_le_bytes([self.read_byte(address), self.read_byte(address + 1)])
    }
    fn read_byte(&self, address: usize) -> u8;
    fn write_word(&mut self, address: usize, word: u32) {
        for (offset, byte) in word.to_le_bytes().into_iter().enumerate() {
            self.write_byte(address + offset, byte);
        }
    }
    fn write_half_word(&mut self, address: usize, half_word: u16) {
        for (offset, byte) in half_word.to_le_bytes().into_iter().enumerate() {
            self.write_byte(address + offset, byte);
        }
    }
    fn write_byte(&mut self, address: usize, byte: u8);
}

impl<T: MemoryPlugin> MemoryPlugin for Arc<Mutex<T>> {
    fn claims_address(&self, address: usize) -> bool {
        self.lock().unwrap().claims_address(address)
    }

    fn read_byte(&self, address: usize) -> u8 {
        self.lock().unwrap().read_byte(address)
    }

    fn write_byte(&mut self, address: usize, byte: u8) {
        self.lock().unwrap().write_byte(address, byte);
    }

    fn reset(&mut self) {
        self.lock().unwrap().reset();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemoryProtection {
    BiosOnly,
}

#[derive(Debug, Default)]
pub struct MemoryWatcher {
    pub reads: VecDeque<(u32, u8)>,
    pub writes: VecDeque<(u32, u8, u8)>,
}

pub struct Memory {
    raw: Vec<u8>,
    pub plugins: Vec<Box<dyn MemoryPlugin + Send>>,
    pub memory_watcher: MemoryWatcher,
    // Used for memory protection!
    ip: u32,
}

impl Memory {
    // const BIOS_SIZE: usize = include_bytes!("../../assets/gba_bios.bin").len();
    pub fn new() -> Self {
        let mut raw = vec![0; 0x0FFFFFFF];
        let bios = include_bytes!("../../assets/gba_bios.bin");
        unsafe {
            assert!(bios.len() <= raw.len());
            ptr::copy_nonoverlapping(bios.as_ptr(), raw.as_mut_ptr(), bios.len());
        }
        Self {
            raw,
            ip: 0,
            plugins: Vec::new(),
            memory_watcher: Default::default(),
        }
    }

    pub fn reset(&mut self) {
        self.raw = vec![0; 0x0FFFFFFF];
        self.ip = 0;
        self.plugins.iter_mut().for_each(|p| p.reset());
    }

    pub fn add_plugin<Plugin: MemoryPlugin + Send + 'static>(&mut self, plugin: Plugin) {
        self.plugins.push(Box::new(plugin));
    }

    pub fn with_plugin<Plugin: MemoryPlugin + Send + 'static>(mut self, plugin: Plugin) -> Self {
        self.add_plugin(plugin);
        self
    }

    pub fn stack(&self, sp: u32) -> &[u8] {
        // &self.raw[0x0300700..sp as usize]
        &self.raw[sp as usize..0x03007FFF]
    }

    pub fn read_word(&mut self, address: u32) -> u32 {
        assert_eq!(address % 4, 0);
        let address = address as usize;
        let address = self
            .handle_mirrors(address)
            .expect("Implement memory protection");
        let value = self
            .get_plugin_for(address)
            .map(|p| p.read_word(address))
            .unwrap_or_else(|| {
                u32::from_le_bytes([
                    self.raw[address],
                    self.raw[address + 1],
                    self.raw[address + 2],
                    self.raw[address + 3],
                ])
            });
        for (i, b) in value.to_le_bytes().into_iter().enumerate() {
            self.log_all::<true>((address + i) as u32, b, b);
        }

        value
    }
    pub fn read_word_silent(&self, address: u32) -> u32 {
        assert_eq!(address % 4, 0);
        let address = address as usize;
        let address = self
            .handle_mirrors(address)
            .expect("Implement memory protection");
        let value = self
            .get_plugin_for(address)
            .map(|p| p.read_word(address))
            .unwrap_or_else(|| {
                u32::from_le_bytes([
                    self.raw[address],
                    self.raw[address + 1],
                    self.raw[address + 2],
                    self.raw[address + 3],
                ])
            });

        value
    }

    pub fn read_half_word_at(&mut self, address: u32) -> u16 {
        assert_eq!(address % 2, 0);
        let address = address as usize;
        let address = self
            .handle_mirrors(address)
            .expect("Implement memory protection");
        let value = self
            .get_plugin_for(address)
            .map(|p| p.read_half_word(address))
            .unwrap_or_else(|| u16::from_le_bytes([self.raw[address], self.raw[address + 1]]));
        for (i, b) in value.to_le_bytes().into_iter().enumerate() {
            self.log_all::<true>((address + i) as u32, b, b);
        }
        value
    }

    pub fn read_half_word_at_silent(&self, address: u32) -> u16 {
        assert_eq!(address % 2, 0);
        let address = address as usize;
        let address = self
            .handle_mirrors(address)
            .expect("Implement memory protection");
        let value = self
            .get_plugin_for(address)
            .map(|p| p.read_half_word(address))
            .unwrap_or_else(|| u16::from_le_bytes([self.raw[address], self.raw[address + 1]]));
        value
    }

    pub(crate) fn read_byte_at(&mut self, address: u32) -> u8 {
        let address = address as usize;
        let address = self
            .handle_mirrors(address)
            .expect("Implement memory protection");
        let value = self
            .get_plugin_for(address)
            .map(|p| p.read_byte(address))
            .unwrap_or_else(|| self.raw[address]);
        self.log_all::<true>(address as _, value, value);

        value
    }

    pub(crate) fn read_byte_at_silent(&self, address: u32) -> u8 {
        let address = address as usize;
        let address = self
            .handle_mirrors(address)
            .expect("Implement memory protection");
        let value = self
            .get_plugin_for(address)
            .map(|p| p.read_byte(address))
            .unwrap_or_else(|| self.raw[address]);

        value
    }

    pub fn write_word_to(&mut self, address: u32, word: u32) {
        if self.is_read_only(address, 4) {
            return;
        }
        assert_eq!(address % 4, 0);
        let address = address as usize;
        let address = self
            .handle_mirrors(address)
            .expect("Implement memory protection");
        let old_value = self
            .get_plugin_for(address)
            .map(|p| p.read_word(address))
            .unwrap_or(u32::from_le_bytes([
                self.raw[address],
                self.raw[address + 1],
                self.raw[address + 2],
                self.raw[address + 3],
            ]));

        for (i, (c, n)) in old_value
            .to_le_bytes()
            .into_iter()
            .zip(word.to_le_bytes())
            .enumerate()
        {
            self.log_all::<false>((address + i) as u32, n, c);
        }
        self.get_plugin_for_mut(address)
            .map(|p| p.write_word(address, word))
            .unwrap_or_else(|| {
                let bytes = word.to_le_bytes();
                for (index, byte) in bytes.into_iter().enumerate() {
                    self.raw[address + index] = byte;
                }
            })
    }

    pub(crate) fn write_half_word_to(&mut self, address: u32, half_word: u16) {
        if self.is_read_only(address, 2) {
            return;
        }
        assert_eq!(address % 2, 0);
        let address = address as usize;
        let address = self
            .handle_mirrors(address)
            .expect("Implement memory protection");
        let old_value = self
            .get_plugin_for(address)
            .map(|p| p.read_half_word(address))
            .unwrap_or(u16::from_le_bytes([
                self.raw[address],
                self.raw[address + 1],
            ]));

        for (i, (c, n)) in old_value
            .to_le_bytes()
            .into_iter()
            .zip(half_word.to_le_bytes())
            .enumerate()
        {
            self.log_all::<false>((address + i) as u32, n, c);
        }
        self.get_plugin_for_mut(address)
            .map(|p| p.write_half_word(address, half_word))
            .unwrap_or_else(|| {
                let bytes = half_word.to_le_bytes();
                for (index, byte) in bytes.into_iter().enumerate() {
                    self.raw[address + index] = byte;
                }
            })
    }

    pub fn write_byte_to(&mut self, address: u32, byte: u8) {
        if self.is_read_only(address, 1) {
            return;
        }
        let address = address as usize;
        let address = self
            .handle_mirrors(address)
            .expect("Implement memory protection");
        let old_value = self
            .get_plugin_for(address)
            .map(|p| p.read_byte(address))
            .unwrap_or(self.raw[address]);
        self.log_all::<false>(address as u32, byte, old_value);
        self.get_plugin_for_mut(address)
            .map(|p| p.write_byte(address, byte))
            .unwrap_or_else(|| {
                self.raw[address] = byte;
            })
    }

    fn get_plugin_for(&self, address: usize) -> Option<&(dyn MemoryPlugin + Send)> {
        let address = self
            .handle_mirrors(address)
            .expect("Implement memory protection");
        self.plugins
            .iter()
            .map(|p| p.as_ref())
            .find(|p| p.claims_address(address))
    }

    fn get_plugin_for_mut(
        &mut self,
        address: usize,
    ) -> Option<&mut (dyn MemoryPlugin + Send + 'static)> {
        let address = self
            .handle_mirrors(address)
            .expect("Implement memory protection");
        self.plugins
            .iter_mut()
            .map(move |p| &mut **p)
            .find(move |p| p.claims_address(address))
    }

    #[allow(dead_code)]
    pub(crate) fn dump(&mut self) {
        let not_used_ranges = [
            0x00004000..=0x01FFFFFF,
            0x02040000..=0x02FFFFFF,
            0x03008000..=0x03FFFFFF,
            0x04000400..=0x04FFFFFF,
            0x05000400..=0x05FFFFFF,
            0x06018000..=0x06FFFFFF,
            0x07000400..=0x07FFFFFF,
            0x0E010000..=0x0FFFFFFF,
            0x10000000..=0xFFFFFFFF,
        ];
        const BIOS_SIZE: usize = include_bytes!("../../assets/gba_bios.bin").len();

        for a in (0usize..0x4000000).step_by(4) {
            if not_used_ranges.iter().any(|r| r.contains(&a)) || a < BIOS_SIZE {
                continue;
            }
            let value = self.read_word(a as _);
            if value == 0 {
                continue;
            }
            println!("{a:08x}: {value:08x} ({value}/{})", value as i32);
        }
    }

    fn log_all<const IS_READING: bool>(&mut self, address: u32, value: u8, old_value: u8) {
        if IS_READING {
            self.memory_watcher.reads.push_back((address, value));
        } else {
            self.memory_watcher
                .writes
                .push_back((address, old_value, value));
        }
    }

    pub fn is_read_only(&self, address: u32, size: u32) -> bool {
        (0x00000000..=0x00003FFF).contains(&address)
            || (0x00000000..=0x00003FFF).contains(&(address + size))
    }

    fn handle_mirrors(&self, address: usize) -> Result<usize, MemoryProtection> {
        let address = match address {
            // Accessing unused memory at 00004000h-01FFFFFFh, and
            // 10000000h-FFFFFFFFh (and 02000000h-03FFFFFFh when RAM is disabled
            // via Port 4000800h) returns the recently pre-fetched opcode. For
            // ARM code this is simply:
            //   WORD = [$+8]
            //   For THUMB code the result consists of two 16bit fragments and
            //   depends on the address area and alignment where the opcode was
            //   stored.
            //   For THUMB code in Main RAM, Palette Memory, VRAM, and Cartridge
            //   ROM this is:
            //     LSW = [$+4], MSW = [$+4]
            //   For THUMB code in BIOS or OAM (and in 32K-WRAM on Original-NDS
            //   (in GBA mode)):
            //     LSW = [$+4], MSW = [$+6]   ;for opcodes at 4-byte aligned locations
            //     LSW = [$+2], MSW = [$+4]   ;for opcodes at non-4-byte aligned locations
            //   For THUMB code in 32K-WRAM on GBA, GBA SP, GBA Micro, NDS-Lite
            //   (but not NDS):
            //     LSW = [$+4], MSW = OldHI   ;for opcodes at 4-byte aligned locations
            //     LSW = OldLO, MSW = [$+4]   ;for opcodes at non-4-byte aligned locations
            //   Whereas OldLO/OldHI are usually:
            //     OldLO=[$+2], OldHI=[$+2]
            //   Unless the previous opcode's prefetch was overwritten; that can
            //   happen if the previous opcode was itself an LDR opcode, ie. if
            //   it was itself reading data:
            //     OldLO=LSW(data), OldHI=MSW(data)
            //     Theoretically, this might also change if a DMA transfer
            //   occurs.
            // Note: Additionally, as usually, the 32bit data value will be
            //   rotated if the data address wasn't 4-byte aligned, and the
            //   upper bits of the 32bit value will be masked in case of
            //   LDRB/LDRH reads.
            //Note: The opcode prefetch is caused by the prefetch pipeline in
            //the CPU itself, not by the external gamepak prefetch, ie. it works
            //   for code in ROM and RAM as well.
            0x00004000..=0x01FFFFFF => unreachable!("TODO: Unpredictable,"),
            0x02000000..=0x02ffffff => address & 0x0203FFFF,
            0x03000000..=0x03ffffff => address & 0x03007FFF,
            0x05000000..=0x05ffffff => address & 0x050003FF,
            0x06017FFF..=0x0603ffff => address & 0x06017FFF,
            0x06040000..=0x06ffffff => self.handle_mirrors(address & 0x0603FFFF)?,
            0x07000000..=0x07ffffff => address & 0x070003FF,
            0x0e000000..=0x0FFFFFFF => address & 0x0E00FFFF,
            0x10000000..=0xffffffff => self.handle_mirrors(address & 0x0FFFFFFF)?,
            address => address,
        };
        if self.ip >= 0x00004000 {
            match address {
                0x00000000..=0x00003fff => return Err(MemoryProtection::BiosOnly),
                _ => {}
            }
        }
        Ok(address)
    }

    pub(crate) fn read_vec(&self, mut at: u32, mut length: u32) -> Vec<u8> {
        let mut result = Vec::with_capacity(length as _);
        while length > 0 {
            result.push(self.read_byte_at_silent(at));
            at += 1;
            length -= 1;
        }
        result
    }
}
