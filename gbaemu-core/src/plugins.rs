mod debug_symbols;

pub mod debugger;

use crate::{
    GbaArgs,
    instructions::Instruction,
    memory::Memory,
    registers::{Mode, Registers},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PluginWishes {
    pause_execution: bool,
    stop_execution: bool,
}
impl PluginWishes {
    pub fn combined_with(self, other: Self) -> Self {
        Self {
            pause_execution: self.pause_execution || other.pause_execution,
            stop_execution: self.stop_execution || other.stop_execution,
        }
    }

    pub(crate) fn pause_execution(&self) -> bool {
        self.pause_execution
    }
}

pub trait Plugin: Send {
    fn with_args(&mut self, args: GbaArgs);
    fn should_execute(
        &mut self,
        _registers: &Registers,
        _mode: Mode,
        _instruction: &Instruction,
        _ip: u32,
        _memory: &Memory,
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
