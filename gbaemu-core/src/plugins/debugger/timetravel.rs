use std::collections::VecDeque;

use crate::{instructions::Instruction, registers::Registers};

pub struct InstructionReversionBuilder {
    ip: u32,
    registers_before: Registers,
    instruction: Instruction,
}

pub struct InstructionReversion {
    ip: u32,
    registers: [u32; 18],
    address_range: Option<std::ops::Range<u32>>,
    memory: Vec<u8>,
}

pub struct History {
    capacity: usize,
    entries: VecDeque<InstructionReversion>,
    next_entry: Option<InstructionReversionBuilder>,
}
impl History {
    pub(crate) fn new(history_capacity: usize) -> Self {
        Self {
            capacity: history_capacity,
            entries: VecDeque::new(),
            next_entry: None,
        }
    }

    pub(crate) fn start_entry(
        &mut self,
        ip: u32,
        registers: crate::registers::Registers,
        instruction: crate::instructions::Instruction,
        memory: &crate::memory::Memory,
    ) {
        self.next_entry = Some(InstructionReversionBuilder {
            ip,
            registers_before: registers,
            instruction,
        });
    }

    pub(crate) fn finish_entry(
        &mut self,
        registers: Registers,
        memory: &mut crate::memory::Memory,
    ) {
        if !self.make_space(1) {
            self.next_entry = None;
            return;
        }
        let Some(entry) = self.next_entry.take() else {
            unreachable!("start_entry needs to be called before finish_entry!")
        };
        let values = entry.registers_before.read_values_to_array();

        let address_range = entry
            .instruction
            .accessed_memory_addresses(entry.registers_before);
        // dbg!(
        //     &address_range,
        //     entry
        //         .instruction
        //         .display(DisplayContext {
        //             is_tty: true,
        //             register_values: entry.registers_before,
        //             use_register_values: true,
        //             ..Default::default()
        //         })
        //         .to_string()
        // );
        let memory = match address_range.clone() {
            Some(range) => range.map(|a| memory.read_byte_at_silent(a)).collect(),
            None => Vec::new(),
        };
        self.entries.push_back(InstructionReversion {
            ip: entry.ip,
            registers: values,
            address_range,
            memory,
        });
    }

    fn make_space(&mut self, space: usize) -> bool {
        if self.entries.len() + space < self.capacity {
            return true;
        }
        while self.entries.pop_front().is_some() {
            if self.entries.len() + space < self.capacity {
                return true;
            }
        }
        false
    }

    pub(crate) fn go_back(
        &mut self,
        register_changes: &mut [Option<u32>; 18],
        memory_changes: &mut Vec<(u32, u8)>,
    ) {
        let Some(entry) = self.entries.pop_back() else {
            return;
        };
        *register_changes = entry.registers.map(|r| Some(r));
        if let Some(range) = entry.address_range {
            for (i, a) in range.into_iter().enumerate() {
                memory_changes.push((a, entry.memory[i]));
            }
        }
    }
}
