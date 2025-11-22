use std::fmt::Display;

use crate::{
    Gba,
    registers::{RegisterIndex, Registers},
};

use super::{
    InstructionDecodeError,
    display::{DisplayContext, DisplayedShifterOperand},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShifterOperandInstructionOp {
    Move,
    Compare,
    CompareNegative,
    And,
    InclusiveOr,
    ExclusiveOr,
    Add,
    AddWithCarry,
    Subtract,
    Multiplicate,
    ReverseSubtract,
    TestEquals,
    Test,
    MoveNegate,
    BitClear,
}

impl Display for ShifterOperandInstructionOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShifterOperandInstructionOp::Move => write!(f, "MOV"),
            ShifterOperandInstructionOp::MoveNegate => write!(f, "MVN"),
            ShifterOperandInstructionOp::Compare => write!(f, "CMP"),
            ShifterOperandInstructionOp::CompareNegative => write!(f, "CMN"),
            ShifterOperandInstructionOp::And => write!(f, "AND"),
            ShifterOperandInstructionOp::InclusiveOr => write!(f, "ORR"),
            ShifterOperandInstructionOp::ExclusiveOr => write!(f, "EOR"),
            ShifterOperandInstructionOp::Add => write!(f, "ADD"),
            ShifterOperandInstructionOp::AddWithCarry => write!(f, "ADC"),
            ShifterOperandInstructionOp::Subtract => write!(f, "SUB"),
            ShifterOperandInstructionOp::Multiplicate => write!(f, "MUL"),
            ShifterOperandInstructionOp::ReverseSubtract => write!(f, "RSB"),
            ShifterOperandInstructionOp::TestEquals => write!(f, "TEQ"),
            ShifterOperandInstructionOp::Test => write!(f, "TST"),
            ShifterOperandInstructionOp::BitClear => write!(f, "BIC"),
        }
    }
}

impl ShifterOperandInstructionOp {
    pub fn execute(
        &self,
        state: &mut Gba,
        update_flags: bool,
        base: RegisterIndex,
        destination: RegisterIndex,
        value: ShifterOperand,
    ) {
        let value = value.resolve_mut(state, update_flags);
        match self {
            ShifterOperandInstructionOp::Move => {
                state.registers.write(destination, value);
                if update_flags {
                    if destination == RegisterIndex::Ip {
                        state.registers.write(
                            RegisterIndex::Cpsr,
                            state.registers.read(RegisterIndex::Spsr),
                        );
                    } else {
                        state.registers.update_nz_flags(value);
                    }
                }
            }
            ShifterOperandInstructionOp::MoveNegate => {
                let value = !value;
                state.registers.write(destination, value);

                if destination == RegisterIndex::Ip {
                    state.registers.write(
                        RegisterIndex::Cpsr,
                        state.registers.read(RegisterIndex::Spsr),
                    );
                } else if update_flags {
                    state.registers.update_nz_flags(value);
                }
            }
            ShifterOperandInstructionOp::Compare => {
                let lhs = state.registers.read(base) as i32;
                state
                    .registers
                    .update_nzv_flags(lhs.overflowing_sub(value as i32));
                state.registers.update_carry_flag(lhs >= value as i32);
                assert!(update_flags);
            }
            ShifterOperandInstructionOp::CompareNegative => {
                let lhs = state.registers.read(base) as i32;
                state
                    .registers
                    .update_nzv_flags(lhs.overflowing_add(value as i32));
                state.registers.update_carry_flag(lhs < value as i32);
                assert!(update_flags);
            }
            ShifterOperandInstructionOp::Add => {
                let lhs = state.registers.read(base) as i32;
                let lhs = if base == RegisterIndex::Ip {
                    lhs & !3
                } else {
                    lhs
                };
                let result_with_overflow_flag = lhs.overflowing_add(value as i32);
                state
                    .registers
                    .write(destination, result_with_overflow_flag.0 as u32);
                if destination == RegisterIndex::Ip {
                    state.registers.write(
                        RegisterIndex::Cpsr,
                        state.registers.read(RegisterIndex::Spsr),
                    );
                } else {
                    if update_flags {
                        state.registers.update_nzv_flags(result_with_overflow_flag);
                        let carry = (lhs as u32).overflowing_add(value).1;
                        state.registers.update_carry_flag(carry);
                    }
                }
            }
            ShifterOperandInstructionOp::AddWithCarry => {
                let lhs = state.registers.read(base) as i32;
                let added_carry_with_overflow_flag =
                    lhs.overflowing_add(state.registers.flags().carry_flag as i32);
                let result_with_overflow_flag = added_carry_with_overflow_flag
                    .0
                    .overflowing_add(value as i32);

                let result_with_overflow_flag = (
                    result_with_overflow_flag.0,
                    result_with_overflow_flag.1 || added_carry_with_overflow_flag.1,
                );
                state
                    .registers
                    .write(destination, result_with_overflow_flag.0 as u32);
                if destination == RegisterIndex::Ip {
                    state.registers.write(
                        RegisterIndex::Cpsr,
                        state.registers.read(RegisterIndex::Spsr),
                    );
                } else {
                    if update_flags {
                        state.registers.update_nzv_flags(result_with_overflow_flag);
                        // state.registers.update_carry_flag(lhs >= value);
                    }
                }
            }
            ShifterOperandInstructionOp::Subtract => {
                let lhs = state.registers.read(base) as i32;
                let result_with_overflow_flag = lhs.overflowing_sub(value as i32);
                state
                    .registers
                    .write(destination, result_with_overflow_flag.0 as u32);
                if destination == RegisterIndex::Ip {
                    let spsr = state.registers.read(RegisterIndex::Spsr);
                    state.registers.write(RegisterIndex::Cpsr, spsr);
                } else {
                    if update_flags {
                        state.registers.update_nzv_flags(result_with_overflow_flag);
                        state.registers.update_carry_flag(lhs >= value as i32);
                    }
                }
            }
            ShifterOperandInstructionOp::ReverseSubtract => {
                let rhs = state.registers.read(base) as i32;
                let result_with_overflow_flag = (value as i32).overflowing_sub(rhs);
                state
                    .registers
                    .write(destination, result_with_overflow_flag.0 as u32);
                if destination == RegisterIndex::Ip {
                    state.registers.write(
                        RegisterIndex::Cpsr,
                        state.registers.read(RegisterIndex::Spsr),
                    );
                } else {
                    if update_flags {
                        state.registers.update_nzv_flags(result_with_overflow_flag);
                        state.registers.update_carry_flag(value as i32 >= rhs);
                    }
                }
            }
            ShifterOperandInstructionOp::Multiplicate => {
                let lhs = state.registers.read(base) as i32;
                let result_with_overflow_flag = lhs.overflowing_mul(value as i32);
                state
                    .registers
                    .write(destination, result_with_overflow_flag.0 as u32);
                if destination == RegisterIndex::Ip {
                    state.registers.write(
                        RegisterIndex::Cpsr,
                        state.registers.read(RegisterIndex::Spsr),
                    );
                } else {
                    if update_flags {
                        state.registers.update_nzv_flags(result_with_overflow_flag);
                        // state.registers.update_carry_flag(lhs >= value);
                    }
                }
            }
            ShifterOperandInstructionOp::And => {
                let result = state.registers.read(base) & value;
                state.registers.write(destination, result);
                if update_flags {
                    state.registers.update_nz_flags(result);
                }
            }
            ShifterOperandInstructionOp::InclusiveOr => {
                let result = state.registers.read(base) | value;
                state.registers.write(destination, result);
                if update_flags {
                    state.registers.update_nz_flags(result);
                }
            }
            ShifterOperandInstructionOp::ExclusiveOr => {
                let result = state.registers.read(base) ^ value;
                state.registers.write(destination, result);
                if update_flags {
                    state.registers.update_nz_flags(result);
                }
            }
            ShifterOperandInstructionOp::TestEquals => {
                let result = state.registers.read(base) ^ value;
                assert!(update_flags);
                state.registers.update_nz_flags(result);
            }
            ShifterOperandInstructionOp::Test => {
                let result = state.registers.read(base) & value;
                assert!(update_flags);
                state.registers.update_nz_flags(result);
            }
            ShifterOperandInstructionOp::BitClear => {
                let result = state.registers.read(base) & !value;
                state.registers.write(destination, result);
                if update_flags {
                    state.registers.update_nz_flags(result);
                }
            }
        }
    }

    pub(crate) fn uses_destination(&self) -> bool {
        !matches!(self, Self::Compare | Self::Test | Self::TestEquals)
    }

    pub(crate) fn uses_base(&self) -> bool {
        !matches!(self, Self::Move | Self::MoveNegate)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_shift() {
        assert_eq!(ShiftOperator::RightShift.execute(1, 1, false), (0, true));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShiftOperator {
    LeftShift,
    RightShift,
    RotateRight,
    ArithmeticRightShift,
}

impl ShiftOperator {
    fn execute(&self, lhs: u32, rhs: u32, carry_flag: bool) -> (u32, bool) {
        let value = match self {
            ShiftOperator::LeftShift => lhs.wrapping_shl(rhs),
            ShiftOperator::RightShift => {
                if rhs == 0 {
                    32
                } else {
                    lhs.wrapping_shr(rhs)
                }
            }
            ShiftOperator::RotateRight => {
                if rhs == 0 {
                    ((carry_flag as u32) << 31) | (lhs >> 1)
                } else {
                    lhs.rotate_right(rhs)
                }
            }
            Self::ArithmeticRightShift => {
                if rhs == 0 {
                    if (lhs as i32) < 0 { u32::MAX } else { 0 }
                } else {
                    (lhs as i32).wrapping_shr(rhs) as u32
                }
            }
        };
        assert!(rhs < 32);
        let carry_flag = match self {
            ShiftOperator::LeftShift => {
                if rhs == 0 {
                    carry_flag
                } else {
                    (lhs & 1 << (32 - rhs)) > 0
                }
            }
            ShiftOperator::RightShift => {
                if rhs == 0 {
                    (lhs as i32) < 0
                } else {
                    (lhs & 1 << (rhs - 1)) > 0
                }
            }
            ShiftOperator::ArithmeticRightShift => {
                if rhs == 0 {
                    (lhs as i32) < 0
                } else {
                    (lhs & 1 << (rhs - 1)) > 0
                }
            }
            ShiftOperator::RotateRight => {
                if rhs == 0 {
                    (lhs & 1) > 0
                } else {
                    (lhs & 1 << (rhs - 1)) > 0
                }
            }
        };
        (value, carry_flag)
    }

    pub(crate) fn decode(shift: u32) -> ShiftOperator {
        match shift {
            0b00 => Self::LeftShift,
            0b01 => Self::RightShift,
            0b10 => Self::ArithmeticRightShift,
            0b11 => Self::RotateRight,
            _ => todo!("Unimplemented shift {shift}"),
        }
    }
}

impl Display for ShiftOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShiftOperator::LeftShift => write!(f, "LSL"),
            ShiftOperator::RightShift => write!(f, "LSR"),
            ShiftOperator::RotateRight => write!(f, "ROR"),
            ShiftOperator::ArithmeticRightShift => write!(f, "ASR"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShifterOperand {
    Immediate(u32, Option<bool>),
    Register(RegisterIndex),
    RegisterImmediate(RegisterIndex, ShiftOperator, u8),
    RegisterRegister(RegisterIndex, ShiftOperator, RegisterIndex),
    // LeftShiftRegister(RegisterIndex, u8),
    // RightShiftRegister(RegisterIndex, u8),
    // RotateRightRegister(RegisterIndex, RegisterIndex),
}

impl ShifterOperand {
    pub(crate) fn resolve_mut(&self, state: &mut Gba, update_flags: bool) -> u32 {
        match *self {
            ShifterOperand::Immediate(value, carry_flag) => {
                if update_flags {
                    state.registers.update_carry_flag(
                        carry_flag.unwrap_or(state.registers.flags().carry_flag),
                    );
                }
                value
            }
            ShifterOperand::Register(register) => state.registers.read(register),
            Self::RegisterImmediate(register, shift_op, immediate) => {
                let value = state.registers.read(register);
                let (value, carry_flag) =
                    shift_op.execute(value, immediate as _, state.registers.flags().carry_flag);
                if update_flags {
                    state.registers.update_carry_flag(carry_flag);
                }
                value
            }
            Self::RegisterRegister(register, shift_op, register2) => {
                let value = state.registers.read(register);
                let value2 = state.registers.read(register2);
                let (value, carry_flag) =
                    shift_op.execute(value, value2, state.registers.flags().carry_flag);
                if update_flags {
                    state.registers.update_carry_flag(carry_flag);
                }
                value
            }
        }
    }
    pub(crate) fn resolve(&self, registers: &Registers) -> u32 {
        match *self {
            ShifterOperand::Immediate(value, _) => value,
            ShifterOperand::Register(register) => registers.read(register),
            Self::RegisterImmediate(register, shift_op, immediate) => {
                let value = registers.read(register);
                let (value, _carry_flag) =
                    shift_op.execute(value, immediate as _, registers.flags().carry_flag);
                value
            }
            Self::RegisterRegister(register, shift_op, register2) => {
                let value = registers.read(register);
                let value2 = registers.read(register2);
                let (value, _carry_flag) =
                    shift_op.execute(value, value2, registers.flags().carry_flag);
                value
            }
        }
    }

    pub fn display(&self, ctx: DisplayContext) -> DisplayedShifterOperand {
        DisplayedShifterOperand {
            operand: *self,
            ctx,
        }
    }
}

impl TryFrom<u32> for ShifterOperand {
    type Error = InstructionDecodeError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        let is_immediate = (value & 1 << 25) > 0;
        let is_immediate_shifts = (value & 1 << 25) == 0 && (value & 0x10) == 0;
        let is_register_shift = (value & 1 << 25) == 0 && (value & 0x10) > 0 && (value & 0x80) == 0;

        if is_immediate {
            let immediate = value & 0x0ff;
            let rotate_immediate = (value & 0xf00) >> 8;
            let immediate = immediate.rotate_right(rotate_immediate * 2);
            Ok(Self::Immediate(
                immediate,
                (rotate_immediate != 0).then_some((immediate & 1 << 31) > 0),
            ))
        } else if is_immediate_shifts {
            let rm = value & 0x0000_000F;
            let rm = RegisterIndex::try_from(rm)
                .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rm))?;

            let immediate = ((value & 0x0000_0F80) >> 7) as u8;
            let shift = (value & 0x0000_0060) >> 5;

            let shift_op = ShiftOperator::decode(shift);
            Ok(Self::RegisterImmediate(rm, shift_op, immediate))
        } else if is_register_shift {
            let rm = value & 0x0000_000F;
            let rm = RegisterIndex::try_from(rm)
                .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rm))?;
            let shift = (value & 0x0000_0060) >> 5;
            let rs = (value & 0x0000_0F00) >> 8;
            let rs = RegisterIndex::try_from(rs)
                .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rs))?;
            let shift_op = ShiftOperator::decode(shift);
            Ok(Self::RegisterRegister(rm, shift_op, rs))
        } else {
            todo!("This is not a shifter operand!")
        }
    }
}
