use crate::registers::{RegisterIndex, RegisterList, Registers};
use colorable::*;
use std::fmt::Display;

use super::{
    shifter_operand::ShifterOperand, BranchTarget, Condition, Instruction, InstructionOp,
    StoreLoadMemoryAddress,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct DisplayContext {
    pub highlighted_registers: RegisterList,
    pub is_tty: bool,
    pub register_values: Registers,
    pub use_register_values: bool,
}

impl DisplayContext {
    pub fn disable_register_values(mut self) -> Self {
        self.use_register_values = false;
        self
    }
}

pub struct DisplayedInstruction {
    pub instruction: Instruction,
    pub ctx: DisplayContext,
}

impl Display for DisplayedInstruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self
            .instruction
            .condition
            .can_execute(self.ctx.register_values.flags())
        {
            if self.ctx.is_tty {
                write!(
                    f,
                    "{}",
                    self.instruction
                        .op
                        .display(self.instruction.condition, self.ctx,)
                        .dark_grey()
                )
            } else {
                write!(
                    f,
                    "({})",
                    self.instruction
                        .op
                        .display(self.instruction.condition, self.ctx,)
                )
            }
        } else {
            write!(
                f,
                "{}",
                self.instruction
                    .op
                    .display(self.instruction.condition, self.ctx,)
            )
        }
    }
}

pub struct DisplayedInstructionOp {
    pub condition: Condition,
    pub op: InstructionOp,
    pub ctx: DisplayContext,
}

impl Display for DisplayedInstructionOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.op {
            InstructionOp::Branch {
                store_return_address_in_link_register,
                return_address_is_thumb: _,
                does_switch_mode,
                target,
                instruction_size: _,
            } => write!(
                f,
                "B{}{}{} {}",
                if store_return_address_in_link_register {
                    "L"
                } else {
                    ""
                },
                if does_switch_mode { "X" } else { "" },
                self.condition,
                target.display(self.ctx),
            ),
            InstructionOp::StoreOrLoadRegister {
                is_load,
                address,
                register_destination,
                read_size,
            } => write!(
                f,
                "{}{}{read_size} {} {}",
                if is_load { "LDR" } else { "STR" },
                self.condition,
                register_destination.display(if is_load {
                    self.ctx.disable_register_values()
                } else {
                    self.ctx
                }),
                address.display(self.ctx),
            ),
            InstructionOp::StoreOrLoadRegisters {
                is_load,
                register_base,
                register_base_write_back,
                register_list,
                addressing_mode,
            } => write!(
                f,
                "{}{}{addressing_mode} {}{} {}",
                if is_load { "LDM" } else { "STM" },
                self.condition,
                register_base.display(self.ctx),
                if register_base_write_back { "!" } else { "" },
                register_list.display(self.ctx),
            ),
            InstructionOp::Mrs {
                use_spsr,
                register_base,
            } => write!(
                f,
                "MRS{} {}, {}",
                self.condition,
                register_base.display(self.ctx),
                if use_spsr { "SPSR" } else { "CPSR" }
            ),
            InstructionOp::Msr {
                use_spsr,
                field_mask,
                operand,
            } => {
                write!(
                    f,
                    "MSR{} {}_{field_mask}, {}",
                    self.condition,
                    if use_spsr { "SPSR" } else { "CPSR" },
                    operand.display(self.ctx),
                )
            }
            InstructionOp::ShifterOperandInstruction {
                op,
                update_flags,
                base,
                destination,
                value,
            } => {
                write!(
                    f,
                    "{op}{}{} ",
                    self.condition,
                    if update_flags { "S" } else { "" }
                )?;
                if op.uses_destination() {
                    write!(
                        f,
                        "{}, ",
                        destination.display(self.ctx.disable_register_values())
                    )?;
                }
                if op.uses_base() {
                    write!(f, "{}, ", base.display(self.ctx))?;
                }
                write!(f, "{}", value.display(self.ctx))?;
                Ok(())
            }
        }
    }
}

pub struct DisplayedBranchTarget {
    pub target: BranchTarget,
    pub ctx: DisplayContext,
}

impl Display for DisplayedBranchTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.target {
            BranchTarget::Offset(offset) => {
                write!(f, "#{offset:x}")
            }
            BranchTarget::RegisterWithOffset(register, offset) => {
                if offset == 0 {
                    write!(f, "{}", register.display(self.ctx),)
                } else {
                    write!(
                        f,
                        "{} {} #{offset:x}",
                        register.display(self.ctx),
                        if offset.is_negative() { '-' } else { '+' }
                    )
                }
            }
        }
    }
}

pub struct DisplayedRegisterIndex {
    pub register: RegisterIndex,
    pub ctx: DisplayContext,
}

impl Display for DisplayedRegisterIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = if self.ctx.use_register_values {
            format!("#{:x}", self.ctx.register_values.read(self.register))
        } else {
            self.register.to_string()
        };
        if self.ctx.highlighted_registers.contains(self.register) {
            if self.ctx.is_tty {
                write!(f, "{}", value.cyan())
            } else {
                write!(f, ">{}", value)
            }
        } else {
            write!(f, "{}", value)
        }
    }
}

pub struct DisplayedStoreLoadMemoryAddress {
    pub address: StoreLoadMemoryAddress,
    pub ctx: DisplayContext,
}

impl Display for DisplayedStoreLoadMemoryAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.ctx.use_register_values {
            let value = self.ctx.register_values.read(self.address.register_base);
            let rhs = self.address.offset.resolve(&self.ctx.register_values);
            if self.address.is_post_indexing {
                write!(
                    f,
                    "[#{value:x}]{} {}{}",
                    if self.address.write_back { "!" } else { "" },
                    if self.address.negate_offset { '-' } else { '+' },
                    self.address.offset.display(self.ctx),
                )
            } else {
                let value = if self.address.negate_offset {
                    value.wrapping_sub(rhs)
                } else {
                    value.wrapping_add(rhs)
                };
                write!(
                    f,
                    "[#{value:x}]{}",
                    if self.address.write_back { "!" } else { "" }
                )
            }
        } else {
            if self.address.is_post_indexing {
                write!(
                    f,
                    "[{}]{} {}{}",
                    self.address.register_base.display(self.ctx),
                    if self.address.write_back { "!" } else { "" },
                    if self.address.negate_offset { '-' } else { '+' },
                    self.address.offset.display(self.ctx),
                )
            } else {
                write!(
                    f,
                    "[{}, {}{}]{}",
                    self.address.register_base.display(self.ctx),
                    if self.address.negate_offset { '-' } else { '+' },
                    self.address.offset.display(self.ctx),
                    if self.address.write_back { "!" } else { "" }
                )
            }
        }
    }
}

pub struct DisplayedRegisterList {
    pub register_list: RegisterList,
    pub ctx: DisplayContext,
}

impl Display for DisplayedRegisterList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{")?;
        for register in self.register_list {
            write!(f, "{}, ", register.display(self.ctx))?;
        }
        write!(f, "}}")?;
        Ok(())
    }
}

pub struct DisplayedShifterOperand {
    pub operand: ShifterOperand,
    pub ctx: DisplayContext,
}

impl Display for DisplayedShifterOperand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.operand {
            ShifterOperand::Immediate(immediate, _) => write!(f, "#{immediate:x}"),
            ShifterOperand::Register(register) => write!(f, "{}", register.display(self.ctx)),
            ShifterOperand::RegisterImmediate(register, shift_op, immediate) => write!(
                f,
                "{}, {shift_op} #{immediate:x}",
                register.display(self.ctx)
            ),
            ShifterOperand::RegisterRegister(register, shift_op, register2) => write!(
                f,
                "{}, {shift_op} {}",
                register.display(self.ctx),
                register2.display(self.ctx),
            ),
        }
    }
}
