use strum::{EnumMessage, IntoEnumIterator};

use crate::{
    memory::Memory,
    plugins::{PluginWishes, debugger::Breakpoint},
    registers::{RegisterIndex, Registers},
};

mod parser;

#[derive(Debug, Clone, PartialEq, Eq, strum::EnumDiscriminants)]
#[strum_discriminants(derive(strum::EnumIter, strum::EnumMessage))]
pub enum Command {
    #[strum_discriminants(strum(message = "b/back - Go back one instruction in the debugger"))]
    GoBack,
    #[strum_discriminants(strum(
        message = "r/registers - Show the current values of all registers"
    ))]
    ShowRegisters,
    #[strum_discriminants(strum(
        message = "sf/stackframe - Show the stackframe of all functions being called right now"
    ))]
    ShowStackFrame,
    #[strum_discriminants(strum(
        message = "jr/jump-return - Continue exectution until the end of the current function. Needs a stackframe to work."
    ))]
    JumpOut,
    #[strum_discriminants(strum(message = "c/continue - Continue exectution again."))]
    Continue,
    #[strum_discriminants(strum(message = "n/next/s/step - Execute one instruction."))]
    Step,
    #[strum_discriminants(strum(message = "show-asm - Output executed assembly onto stdout."))]
    StartEmittingAssembly,
    #[strum_discriminants(strum(
        message = "hide-asm - Stop Outputting executed assembly onto stdout."
    ))]
    StopEmittingAssembly,
    #[strum_discriminants(strum(message = "[ADDR] - Read Memory value at address."))]
    ReadMemory(Address),
    #[strum_discriminants(strum(message = "[ADDR]=VALUE - Write value to memory address."))]
    WriteMemory(Address),
    #[strum_discriminants(strum(message = "?/h/help - Emits this help."))]
    Help,
    Echo(String),
}

impl Command {
    pub fn parse(cmd: &str) -> Result<Command, ()> {
        parser::parse_command(cmd.to_string())
    }

    pub(crate) fn execute(
        &self,
        debugger: &mut super::Debugger,
        registers: Registers,
        ip: u32,
        memory: &Memory,
    ) -> PluginWishes {
        let mut pause_execution = true;
        let mut register_changes = [None; 18];
        let mut memory_changes = Vec::new();

        match self {
            Command::GoBack => debugger
                .history
                .go_back(&mut register_changes, &mut memory_changes),
            Command::ShowRegisters => {
                registers.dump();
            }
            Command::ShowStackFrame => {
                debugger.print_stack_frame(ip);
            }
            Command::JumpOut => {
                if let Some(target) = debugger
                    .stackframe
                    .iter()
                    .copied()
                    .filter(|t| *t != ip)
                    .last()
                {
                    debugger.breakpoints.push(Breakpoint {
                        ip: target,
                        condition: None,
                        action: None,
                        fragile: true,
                    });
                    debugger.is_stepping = false;
                    pause_execution = false;
                } else {
                    println!("Stackframe is empty. No function to jump to.")
                }
            }
            Command::Continue => {
                debugger.is_stepping = false;
                pause_execution = false;
            }
            Command::Step => {
                pause_execution = false;
            }
            Command::StartEmittingAssembly => {
                debugger.is_silent = false;
            }
            Command::StopEmittingAssembly => {
                debugger.is_silent = true;
            }
            Command::ReadMemory(address) => {
                let address = address.resolve(registers, memory);
                eprintln!("[{}] = {:#x}", address, memory.read_word_silent(address));
            }
            Command::WriteMemory(address) => todo!(),
            Command::Help => {
                eprintln!(
                    "ADDR or VALUE can be written as hex (0xaf123) or decimal (1921). You can also use registers (R6 or r6 or sp) or memory locations"
                );
                for c in CommandDiscriminants::iter().filter_map(|c| c.get_message()) {
                    eprintln!("  {c}");
                }
            }
            Command::Echo(v) => {
                println!("{v}");
            }
        }
        PluginWishes {
            pause_execution,
            stop_execution: false,
            register_changes,
            memory_changes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Address {
    Literal(u32),
    Indirect(Box<Address>),
    Register(RegisterIndex),
}
impl Address {
    fn resolve(&self, registers: Registers, memory: &Memory) -> u32 {
        match self {
            Address::Literal(it) => *it,
            Address::Indirect(address) => {
                let address = address.resolve(registers, memory);
                memory.read_word_silent(address)
            }
            Address::Register(register) => registers.read(*register),
        }
    }
}
