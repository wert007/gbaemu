pub use debug_symbols::*;

mod debug_symbols;

pub mod debugger;
pub mod function_watcher;

use crate::{
    GbaArgs,
    instructions::Instruction,
    interrupts::Interrupt,
    memory::Memory,
    registers::{Mode, RegisterIndex, Registers},
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PluginWishes {
    pause_execution: bool,
    stop_execution: bool,
    register_changes: [Option<u32>; 18],
    memory_changes: Vec<(u32, u8)>,
}
impl PluginWishes {
    pub fn apply_register_changes(&self, registers: &mut Registers) {
        for (i, value) in self
            .register_changes
            .iter()
            .enumerate()
            .filter_map(|(a, r)| r.map(|r| (a, r)))
        {
            let register = RegisterIndex::try_from(i as u32).unwrap();
            registers.write(register, value);
        }
    }

    pub fn apply_memory_changes(&self, memory: &mut Memory) {
        for (a, b) in &self.memory_changes {
            memory.write_byte_to(*a, *b);
        }
    }

    pub fn combined_with(self, other: Self) -> Self {
        let mut memory_changes: Vec<(u32, u8)> = self
            .memory_changes
            .into_iter()
            .chain(other.memory_changes)
            .collect();
        memory_changes.sort_by_key(|c| c.0);
        memory_changes.dedup_by_key(|c| c.0);
        Self {
            pause_execution: self.pause_execution || other.pause_execution,
            stop_execution: self.stop_execution || other.stop_execution,
            register_changes: std::array::from_fn(|i| {
                self.register_changes[i].or(other.register_changes[i])
            }),
            memory_changes,
        }
    }

    pub(crate) fn pause_execution(&self) -> bool {
        self.pause_execution
    }
}

pub trait Plugin: Send {
    fn with_args(&mut self, args: GbaArgs);
    fn interrupt_occured(&mut self, _interrupt: Interrupt) -> Option<PluginWishes> {
        None
    }
    fn should_execute(
        &mut self,
        _registers: &Registers,
        _mode: Mode,
        _instruction: &Instruction,
        _ip: u32,
        _memory: &mut Memory,
    ) -> PluginWishes {
        PluginWishes::default()
    }

    fn after_executing(
        &mut self,
        _registers: &Registers,
        _mode: Mode,
        _instruction: &Instruction,
        _ip: u32,
        _memory: &mut Memory,
    ) {
    }
}
