use std::path::is_separator;

use crate::{
    instructions::{display::DisplayContext, Instruction},
    registers::{Mode, RegisterIndex, RegisterList, Registers},
};

pub trait Plugin: Send {
    fn should_execute(
        &mut self,
        _registers: &Registers,
        _mode: Mode,
        _instruction: &Instruction,
        _ip: u32,
    ) -> bool {
        true
    }

    fn after_executing(
        &mut self,
        _registers: &Registers,
        _mode: Mode,
        _instruction: &Instruction,
        _ip: u32,
    ) {
    }
}

pub struct Breakpoint {
    pub ip: u32,
}

pub struct Debugger {
    breakpoints: Vec<Breakpoint>,
    is_stepping: bool,
    watch_stack: bool,
    stackframe: Vec<u32>,
}

impl Debugger {
    pub fn new() -> Self {
        Self {
            breakpoints: Vec::new(),
            is_stepping: false,
            watch_stack: false,
            stackframe: Vec::new(),
        }
    }

    pub fn with_watch_stack(&mut self) -> &mut Self {
        self.watch_stack = true;
        self
    }

    pub fn with_breakpoint(&mut self, ip: u32) -> &mut Self {
        self.breakpoints.push(Breakpoint { ip });
        self
    }
}

impl Plugin for Debugger {
    fn after_executing(
        &mut self,
        registers: &Registers,
        _mode: Mode,
        instruction: &Instruction,
        _ip: u32,
    ) {
        if !self.watch_stack {
            return;
        }
        match &instruction.op {
            crate::instructions::InstructionOp::Branch {
                store_return_address_in_link_register,
                return_address_is_thumb,
                does_switch_mode,
                target,
                instruction_size,
            } => {
                if *store_return_address_in_link_register {
                    self.stackframe.push(registers.read_raw(RegisterIndex::Ip));
                }
            }
            crate::instructions::InstructionOp::StoreOrLoadRegister {
                is_load,
                address,
                register_destination,
                read_size,
            } => {
                if *is_load && *register_destination == RegisterIndex::Ip {
                    todo!()
                }
            }
            crate::instructions::InstructionOp::StoreOrLoadRegisters {
                is_load,
                register_base,
                register_base_write_back,
                register_list,
                addressing_mode,
            } => {
                if *is_load && register_list.contains(RegisterIndex::Ip) {
                    if *register_base == RegisterIndex::Sp && *register_base_write_back {
                        self.stackframe.pop();
                    } else {
                        dbg!(register_base, register_base_write_back, addressing_mode);
                        todo!()
                    }
                }
            }
            crate::instructions::InstructionOp::ShifterOperandInstruction {
                op,
                update_flags,
                base,
                destination,
                value,
            } => {
                if *destination == RegisterIndex::Ip {
                    todo!()
                }
            }
            _ => {}
        }
    }

    fn should_execute(
        &mut self,
        registers: &Registers,
        _mode: Mode,
        instruction: &Instruction,
        ip: u32,
    ) -> bool {
        if self.watch_stack {
            let function_call = match &instruction.op {
                crate::instructions::InstructionOp::Branch {
                    store_return_address_in_link_register,
                    return_address_is_thumb,
                    does_switch_mode,
                    target,
                    instruction_size,
                } => {
                    // target
                    *store_return_address_in_link_register
                }
                crate::instructions::InstructionOp::StoreOrLoadRegister {
                    is_load,
                    address,
                    register_destination,
                    read_size,
                } => false,
                crate::instructions::InstructionOp::StoreOrLoadRegisters {
                    is_load,
                    register_base,
                    register_base_write_back,
                    register_list,
                    addressing_mode,
                } => {
                    if *is_load && register_list.contains(RegisterIndex::Ip) {
                        if *register_base == RegisterIndex::Sp && *register_base_write_back {
                            true
                        } else {
                            dbg!(register_base, register_base_write_back, addressing_mode);
                            todo!()
                        }
                    } else {
                        false
                    }
                }
                crate::instructions::InstructionOp::Mrs {
                    use_spsr,
                    register_base,
                } => false,
                crate::instructions::InstructionOp::Msr {
                    use_spsr,
                    field_mask,
                    operand,
                } => false,
                crate::instructions::InstructionOp::ShifterOperandInstruction {
                    op,
                    update_flags,
                    base,
                    destination,
                    value,
                } => {
                    if *destination == RegisterIndex::Ip {
                        todo!()
                    } else {
                        false
                    }
                }
            };
            if function_call {
                println!("Stackframe:");
                for ip in &self.stackframe {
                    println!(" at {ip:#04x}");
                }
            }
        }
        if !self.is_stepping && !self.breakpoints.iter().any(|b| b.ip == ip) {
            return true;
        }
        self.is_stepping = true;
        instruction.display(DisplayContext {
            highlighted_registers: RegisterList::default(),
            is_tty: true,
            register_values: *registers,
            use_register_values: true,
        });
        loop {
            let mut line = String::new();
            println!("Press [c] to continue");
            std::io::stdin().read_line(&mut line).unwrap();
            match line.to_lowercase().trim() {
                "r" => {
                    registers.dump();
                }
                "c" => {
                    self.is_stepping = false;
                    break;
                }
                _ => {
                    break;
                }
            }
        }
        true
    }
}
