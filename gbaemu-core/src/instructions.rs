use display::{
    DisplayContext, DisplayedBranchTarget, DisplayedInstruction, DisplayedInstructionOp,
    DisplayedStoreLoadMemoryAddress,
};
use shifter_operand::{ShiftOperator, ShifterOperand, ShifterOperandInstructionOp};
use std::{
    fmt::{Debug, Display},
    ops::Range,
};
use thiserror::Error;

use crate::{
    Gba,
    registers::{RegisterIndex, RegisterList, Registers},
};

mod decode;
pub mod display;
mod shifter_operand;
#[cfg(test)]
mod tests;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Condition {
    Equal,
    NotEqual,
    CarrySet,
    CarryClear,
    Negative,
    Positive,
    OverflowSet,
    OverflowClear,
    UnsignedHigher,
    UnsignedLowerOrSame,
    GreaterThanEquals,
    LessThan,
    GreaterThan,
    LessThanEquals,
    Always,
}

impl Display for Condition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Condition::Equal => write!(f, "EQ"),
            Condition::NotEqual => write!(f, "NE"),
            Condition::CarrySet => write!(f, "CS"),
            Condition::CarryClear => write!(f, "CC"),
            Condition::Negative => write!(f, "MI"),
            Condition::Positive => write!(f, "PL"),
            Condition::OverflowSet => write!(f, "VS"),
            Condition::OverflowClear => write!(f, "VC"),
            Condition::UnsignedHigher => write!(f, "HI"),
            Condition::UnsignedLowerOrSame => write!(f, "LS"),
            Condition::GreaterThanEquals => write!(f, "GE"),
            Condition::LessThan => write!(f, "LT"),
            Condition::GreaterThan => write!(f, "GT"),
            Condition::LessThanEquals => write!(f, "LE"),
            Condition::Always => write!(f, ""),
        }
    }
}

impl Condition {
    pub fn can_execute(&self, flags: InstructionFlags) -> bool {
        match self {
            Condition::Equal => flags.zero_flag,
            Condition::NotEqual => !flags.zero_flag,
            Condition::CarrySet => flags.carry_flag,
            Condition::CarryClear => !flags.carry_flag,
            Condition::Negative => flags.negative_flag,
            Condition::Positive => !flags.negative_flag,
            Condition::OverflowSet => flags.overflow_flag,
            Condition::OverflowClear => !flags.overflow_flag,
            Condition::UnsignedHigher => flags.carry_flag && !flags.zero_flag,
            Condition::UnsignedLowerOrSame => !flags.carry_flag || flags.zero_flag,
            Condition::GreaterThanEquals => flags.negative_flag == flags.overflow_flag,
            Condition::LessThan => flags.negative_flag != flags.overflow_flag,
            Condition::GreaterThan => {
                !flags.zero_flag && flags.negative_flag == flags.overflow_flag
            }
            Condition::LessThanEquals => {
                flags.zero_flag || flags.negative_flag != flags.overflow_flag
            }
            Condition::Always => true,
        }
    }
}

#[derive(Error, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConditionError {
    #[error("Must be below 15, was {0}")]
    OutOfRange(u8),
    #[error("Meaning of 15 is undefined I think.")]
    InvalidValue,
}

impl TryFrom<u8> for Condition {
    type Error = ConditionError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::Equal,
            1 => Self::NotEqual,
            2 => Self::CarrySet,
            3 => Self::CarryClear,
            4 => Self::Negative,
            5 => Self::Positive,
            6 => Self::OverflowSet,
            7 => Self::OverflowClear,
            8 => Self::UnsignedHigher,
            9 => Self::UnsignedLowerOrSame,
            10 => Self::GreaterThanEquals,
            11 => Self::LessThan,
            12 => Self::GreaterThan,
            13 => Self::LessThanEquals,
            14 => Self::Always,
            15 => return Err(ConditionError::InvalidValue),
            _ => return Err(ConditionError::OutOfRange(value)),
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Instruction {
    pub condition: Condition,
    pub op: InstructionOp,
    pub size: usize,
}

impl Display for Instruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.display(DisplayContext {
                highlighted_registers: RegisterList::default(),
                is_tty: true,
                register_values: Registers::new(),
                use_register_values: false,
                display_memory_offsets: true,
            })
        )
    }
}

impl Instruction {
    pub fn execute(self, state: &mut Gba) {
        if self.condition.can_execute(state.registers.flags()) {
            self.op.execute(state)
        }
    }

    pub(crate) fn increments_instruction_pointer(&self, state: &Gba) -> bool {
        self.condition.can_execute(state.registers.flags())
            && match self.op {
                InstructionOp::StoreOrLoadRegister {
                    is_load: true,
                    register_destination: RegisterIndex::Ip,
                    read_size: StoreLoadMemorySize::Word,
                    ..
                }
                | InstructionOp::ShifterOperandInstruction {
                    op:
                        // TODO: And multiple others
                        ShifterOperandInstructionOp::Move
                        | ShifterOperandInstructionOp::Subtract
                        | ShifterOperandInstructionOp::Add,
                    destination: RegisterIndex::Ip,
                    ..
                }
                | InstructionOp::Branch { .. } => true,
                InstructionOp::StoreOrLoadRegisters {
                    is_load: true,
                    register_list,
                    ..
                } if register_list.contains(RegisterIndex::Ip) => true,
                _ => false,
            }
    }

    pub(crate) fn display(&self, ctx: DisplayContext) -> DisplayedInstruction {
        DisplayedInstruction {
            instruction: *self,
            ctx,
        }
    }

    // This is very handwavey for now. compare with
    // https://problemkaputt.de/gbatek.htm#armcpuinstructioncycletimes and
    // https://mgba.io/2015/06/27/cycle-counting-prefetch/ for more details.
    pub(crate) fn tick_duration(&self, flags: InstructionFlags) -> usize {
        self.op.tick_duration()
            + if self.condition.can_execute(flags) {
                0
            } else {
                1
            }
    }

    pub(crate) fn accessed_memory_addresses(&self, registers: Registers) -> Option<Range<u32>> {
        if !self.condition.can_execute(registers.flags()) {
            return None;
        }
        match self.op {
            InstructionOp::Branch { .. } => None,
            InstructionOp::StoreOrLoadRegister {
                is_load,
                address,
                read_size,
                ..
            } => {
                if is_load {
                    return None;
                }
                let size = match read_size {
                    StoreLoadMemorySize::Byte => 1,
                    StoreLoadMemorySize::SignedByte => 1,
                    StoreLoadMemorySize::Halfword => 2,
                    StoreLoadMemorySize::SignedHalfword => 2,
                    StoreLoadMemorySize::Word => 4,
                };
                // assert!(!address.is_post_indexing);
                let address = address.resolve_from_registers(registers);
                Some(address..(address + size))
            }
            InstructionOp::StoreOrLoadRegisters {
                is_load,
                register_base,
                register_list,
                addressing_mode,
                ..
            } => {
                if is_load {
                    return None;
                }
                let size = register_list.len() * 4;
                let mut address = registers.read(register_base);
                if addressing_mode.does_change_before() {
                    address = if addressing_mode.is_decreasing() {
                        address - 4
                    } else {
                        address + 4
                    };
                }
                Some(if addressing_mode.is_decreasing() {
                    (address - size)..address
                } else {
                    address..(address + size)
                })
            }
            InstructionOp::Mrs { .. } => None,
            InstructionOp::Msr { .. } => None,
            InstructionOp::ShifterOperandInstruction { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub struct InstructionFlags {
    pub(crate) zero_flag: bool,
    pub(crate) carry_flag: bool,
    pub(crate) negative_flag: bool,
    pub(crate) overflow_flag: bool,
}

impl Display for InstructionFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}{}{}",
            if self.zero_flag { "Z" } else { "-" },
            if self.carry_flag { "C" } else { "-" },
            if self.negative_flag { "N" } else { "-" },
            if self.overflow_flag { "V" } else { "-" }
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchTarget {
    Offset(i32),
    RegisterWithOffset(RegisterIndex, i32),
}

impl BranchTarget {
    // pub fn resolve_to_offset(&self, state: &Gba) -> i32 {
    //     match *self {
    //         BranchTarget::Offset(offset) => offset,
    //         BranchTarget::XOffset(offset) => offset,
    //         BranchTarget::XTarget(register) => state.registers.read(register) as i32,
    //         BranchTarget::RegisterWithOffset(register, offset) => {
    //             state.registers.read(register).wrapping_add_signed(offset) as i32
    //         }
    //     }
    // }

    pub fn register(&self) -> Option<RegisterIndex> {
        match self {
            BranchTarget::Offset(_) => None,
            BranchTarget::RegisterWithOffset(register_index, _) => Some(*register_index),
        }
    }

    pub fn display(&self, ctx: DisplayContext) -> DisplayedBranchTarget {
        DisplayedBranchTarget { target: *self, ctx }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreLoadMemorySize {
    Byte,
    #[allow(dead_code)]
    SignedByte,
    Halfword,
    SignedHalfword,
    Word,
}

impl Display for StoreLoadMemorySize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreLoadMemorySize::Byte => write!(f, "B"),
            StoreLoadMemorySize::Halfword => write!(f, "H"),
            StoreLoadMemorySize::Word => write!(f, ""),
            StoreLoadMemorySize::SignedByte => write!(f, "SB"),
            StoreLoadMemorySize::SignedHalfword => write!(f, "SH"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreLoadMemoryAddress {
    ignore_bit_1_of_pc: bool,
    register_base: RegisterIndex,
    write_back: bool,
    negate_offset: bool,
    offset: ShifterOperand,
    is_post_indexing: bool,
}

impl TryFrom<u32> for StoreLoadMemoryAddress {
    type Error = InstructionDecodeError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        let is_post_indexing = (value & 1 << 24) == 0;
        let negate_offset = (value & 1 << 23) == 0;
        let is_immediate = (value & 1 << 25) == 0;
        let write_back = (value & 1 << 21) > 0;
        let rn = (value & 0xF_0000) >> 16;
        let rn = RegisterIndex::try_from(rn)
            .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rn))?;

        match is_immediate {
            true => {
                let offset = value & 0x0000_0fff;
                Ok(Self {
                    register_base: rn,
                    offset: ShifterOperand::Immediate(offset, None),
                    negate_offset,
                    write_back,
                    ignore_bit_1_of_pc: false,
                    is_post_indexing,
                })
            }
            false => {
                let rm = value & 0x0000_000f;
                let rm = RegisterIndex::try_from(rm)
                    .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rm))?;
                let shift = (value & 0x0000_0060) >> 5;
                let shift_imm = (value & 0x0000_0f80) >> 7;
                let shift_op = ShiftOperator::decode(shift);
                let op = ShifterOperand::RegisterImmediate(rm, shift_op, shift_imm as _);
                Ok(Self {
                    register_base: rn,
                    offset: op,
                    negate_offset,
                    write_back,
                    ignore_bit_1_of_pc: false,
                    is_post_indexing,
                })
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::VariantArray)]
pub enum StoreLoadManyAddressingMode {
    IncrementBefore,
    IncrementAfter,
    DecrementBefore,
    DecrementAfter,
}

impl StoreLoadManyAddressingMode {
    fn from_flags(p_flag: bool, u_flag: bool) -> Self {
        match (p_flag, u_flag) {
            (true, true) => Self::DecrementAfter,
            (true, false) => Self::IncrementAfter,
            (false, true) => Self::DecrementBefore,
            (false, false) => Self::IncrementBefore,
        }
    }

    // FIXE: The wrapping_add(4) is in weird places!
    fn start_address(&self, base: u32, register_list: RegisterList) -> u32 {
        match self {
            StoreLoadManyAddressingMode::IncrementBefore => base.wrapping_add(4),
            StoreLoadManyAddressingMode::IncrementAfter => base,
            StoreLoadManyAddressingMode::DecrementBefore => {
                base.wrapping_sub(register_list.byte_len())
            }
            StoreLoadManyAddressingMode::DecrementAfter => {
                base.wrapping_sub(register_list.byte_len()).wrapping_add(4)
            }
        }
    }

    fn end_address(&self, base: u32, register_list: RegisterList) -> u32 {
        match self {
            StoreLoadManyAddressingMode::IncrementBefore => {
                base.wrapping_add(register_list.byte_len())
            }
            StoreLoadManyAddressingMode::IncrementAfter => {
                base.wrapping_add(register_list.byte_len()).wrapping_sub(4)
            }
            StoreLoadManyAddressingMode::DecrementBefore => base.wrapping_sub(4),
            StoreLoadManyAddressingMode::DecrementAfter => base,
        }
    }

    fn write_back(&self, base: u32, register_list: RegisterList) -> u32 {
        match self {
            StoreLoadManyAddressingMode::IncrementAfter
            | StoreLoadManyAddressingMode::IncrementBefore => {
                base.wrapping_add(register_list.byte_len())
            }
            StoreLoadManyAddressingMode::DecrementBefore
            | StoreLoadManyAddressingMode::DecrementAfter => {
                base.wrapping_sub(register_list.byte_len())
            }
        }
    }

    fn offset_before(&self) -> i32 {
        match self {
            StoreLoadManyAddressingMode::IncrementBefore => 4,
            StoreLoadManyAddressingMode::IncrementAfter => 0,
            StoreLoadManyAddressingMode::DecrementBefore => -4,
            StoreLoadManyAddressingMode::DecrementAfter => 0,
        }
    }

    fn offset_after(&self) -> i32 {
        match self {
            StoreLoadManyAddressingMode::IncrementBefore => 0,
            StoreLoadManyAddressingMode::IncrementAfter => 4,
            StoreLoadManyAddressingMode::DecrementBefore => 0,
            StoreLoadManyAddressingMode::DecrementAfter => -4,
        }
    }

    fn is_decreasing(&self) -> bool {
        matches!(self, Self::DecrementAfter | Self::DecrementBefore)
    }

    fn does_change_before(&self) -> bool {
        matches!(self, Self::IncrementBefore | Self::DecrementBefore)
    }
}

impl Display for StoreLoadManyAddressingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreLoadManyAddressingMode::IncrementBefore => write!(f, "IB"),
            StoreLoadManyAddressingMode::IncrementAfter => write!(f, "IA"),
            StoreLoadManyAddressingMode::DecrementBefore => write!(f, "DB"),
            StoreLoadManyAddressingMode::DecrementAfter => write!(f, "DA"),
        }
    }
}

impl StoreLoadMemoryAddress {
    fn resolve_mut(&self, state: &mut Gba, update_flags: bool) -> u32 {
        let base = state.registers.read(self.register_base);
        let base = if self.register_base == RegisterIndex::Ip && self.ignore_bit_1_of_pc {
            base & !2
        } else {
            base
        };
        let address = if !self.is_post_indexing {
            let offset = self.offset.resolve_mut(state, update_flags);
            if self.negate_offset {
                base.wrapping_sub(offset)
            } else {
                base.wrapping_add(offset)
            }
        } else {
            base
        };
        if self.write_back {
            state.registers.write(self.register_base, address);
        }
        if self.is_post_indexing {
            let offset = self.offset.resolve_mut(state, update_flags);
            let address = if self.negate_offset {
                address.wrapping_sub(offset)
            } else {
                address.wrapping_add(offset)
            };
            state.registers.write(self.register_base, address);
        }
        address
    }

    fn resolve_from_registers(&self, registers: Registers) -> u32 {
        let base = registers.read(self.register_base);
        let base = if self.register_base == RegisterIndex::Ip && self.ignore_bit_1_of_pc {
            base & !2
        } else {
            base
        };
        let address = if !self.is_post_indexing {
            let offset = self.offset.resolve(&registers);
            if self.negate_offset {
                base.wrapping_sub(offset)
            } else {
                base.wrapping_add(offset)
            }
        } else {
            base
        };
        // if self.is_post_indexing {
        //     let offset = self.offset.resolve(&registers);
        //     let address = if self.negate_offset {
        //         address.wrapping_sub(offset)
        //     } else {
        //         address.wrapping_add(offset)
        //     };
        // }
        address
    }

    fn display(&self, ctx: DisplayContext) -> DisplayedStoreLoadMemoryAddress {
        DisplayedStoreLoadMemoryAddress {
            address: *self,
            ctx,
        }
    }

    fn from_addressing_mode_3(word: u32) -> Result<Self, InstructionDecodeError> {
        let is_immediate = (word & 1 << 22) > 0;
        let is_post_indexing = (word & 1 << 24) == 0;
        let write_back = (word & 1 << 21) > 0;
        let negate_offset = (word & 1 << 23) == 0;
        let rn = (word & 0x000f_0000) >> 16;
        let rn = RegisterIndex::try_from(rn)
            .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rn))?;
        let offset = if is_immediate {
            let immediate = ((word & 0xf00) >> 4) | (word & 0xf);
            ShifterOperand::Immediate(immediate, None)
        } else {
            let rm = word & 0xf;
            let rm = RegisterIndex::try_from(rm)
                .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rm))?;
            ShifterOperand::Register(rm)
        };
        Ok(Self {
            ignore_bit_1_of_pc: false,
            register_base: rn,
            write_back,
            negate_offset,
            offset,
            is_post_indexing,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstructionOp {
    Branch {
        store_return_address_in_link_register: bool,
        return_address_is_thumb: bool,
        does_switch_mode: bool,
        target: BranchTarget,
        instruction_size: usize,
    },
    StoreOrLoadRegister {
        is_load: bool,
        address: StoreLoadMemoryAddress,
        register_destination: RegisterIndex,
        read_size: StoreLoadMemorySize,
    },
    StoreOrLoadRegisters {
        is_load: bool,
        register_base: RegisterIndex,
        register_base_write_back: bool,
        register_list: RegisterList,
        addressing_mode: StoreLoadManyAddressingMode,
    },
    Mrs {
        use_spsr: bool,
        register_base: RegisterIndex,
    },
    Msr {
        use_spsr: bool,
        field_mask: MsrFieldMask,
        operand: ShifterOperand,
    },
    ShifterOperandInstruction {
        op: ShifterOperandInstructionOp,
        update_flags: bool,
        base: RegisterIndex,
        destination: RegisterIndex,
        value: ShifterOperand,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MsrFieldMask {
    control: bool,
    extension: bool,
    status: bool,
    flags: bool,
}
impl MsrFieldMask {
    fn to_mask(self) -> u32 {
        let mut result = 0;
        if self.control {
            result |= 0x0000_00FF;
        }
        if self.extension {
            result |= 0x0000_FF00;
        }
        if self.status {
            result |= 0x00FF_0000;
        }
        if self.flags {
            result |= 0xFF00_0000;
        }
        result
    }
}

impl Display for MsrFieldMask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.control {
            write!(f, "c")?;
        }
        if self.extension {
            write!(f, "x")?;
        }
        if self.status {
            write!(f, "s")?;
        }
        if self.flags {
            write!(f, "f")?;
        }
        Ok(())
    }
}

impl TryFrom<u8> for MsrFieldMask {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value > 0xf {
            return Err(());
        }
        Ok(Self {
            control: (value & 0x1) > 0,
            extension: (value & 0x2) > 0,
            status: (value & 0x4) > 0,
            flags: (value & 0x8) > 0,
        })
    }
}

impl InstructionOp {
    fn execute(self, state: &mut Gba) {
        match self {
            InstructionOp::Branch {
                store_return_address_in_link_register,
                return_address_is_thumb,
                does_switch_mode,
                target,
                instruction_size,
            } => {
                let target = match target {
                    BranchTarget::Offset(offset) => state
                        .registers
                        .read(RegisterIndex::Ip)
                        .wrapping_add_signed(offset),
                    BranchTarget::RegisterWithOffset(register, offset) => {
                        state.registers.read(register).wrapping_add_signed(offset)
                    }
                };
                let target = if does_switch_mode {
                    let is_thumb = (target & 0x1) > 0;
                    state.registers.cpsr_mut().set_is_thumb(is_thumb);
                    target & 0xffff_fffe
                } else {
                    target
                };
                if store_return_address_in_link_register {
                    state.registers.write(
                        RegisterIndex::Lr,
                        (state.registers.read_raw(RegisterIndex::Ip) + instruction_size as u32)
                            | if return_address_is_thumb { 1 } else { 0 },
                    );
                }
                state.registers.write(RegisterIndex::Ip, target);
            }
            InstructionOp::StoreOrLoadRegister {
                is_load,
                register_destination,
                address,
                read_size,
            } => {
                let address_raw = address.resolve_mut(state, false);
                if is_load {
                    let value = match read_size {
                        StoreLoadMemorySize::Byte => state.memory.read_byte_at(address_raw) as u32,
                        StoreLoadMemorySize::Halfword => {
                            state.memory.read_half_word_at(address_raw) as u32
                        }
                        StoreLoadMemorySize::Word => state.memory.read_word(address_raw),
                        StoreLoadMemorySize::SignedByte => {
                            state.memory.read_byte_at(address_raw) as i8 as i32 as u32
                        }
                        StoreLoadMemorySize::SignedHalfword => {
                            state.memory.read_half_word_at(address_raw) as i16 as i32 as u32
                        }
                    };
                    let value = if register_destination == RegisterIndex::Ip {
                        value
                            & if state.registers.cpsr().is_thumb() {
                                0xffff_fffe
                            } else {
                                0xffff_fffc
                            }
                    } else {
                        value
                    };
                    state.registers.write(register_destination, value);
                } else {
                    let value = state.registers.read(register_destination);
                    match read_size {
                        StoreLoadMemorySize::Byte => {
                            state.memory.write_byte_to(address_raw, value as u8)
                        }
                        StoreLoadMemorySize::Halfword => {
                            // eprintln!("{address_raw:#x}: {value:#x} as u16 = {:#x}", value as u16);
                            state.memory.write_half_word_to(address_raw, value as u16)
                        }
                        StoreLoadMemorySize::Word => state.memory.write_word_to(address_raw, value),
                        StoreLoadMemorySize::SignedByte | StoreLoadMemorySize::SignedHalfword => {
                            unreachable!("Signedness makes only sense when loading!")
                        }
                    }
                }
            }
            InstructionOp::StoreOrLoadRegisters {
                is_load,
                register_list,
                register_base,
                register_base_write_back,
                addressing_mode,
            } => {
                let base = state.registers.read(register_base);
                let mut address = addressing_mode.start_address(base, register_list);
                if register_base == RegisterIndex::Sp && state.args.watch_stack {
                    if is_load {
                        println!(
                            "Stack (={base:x}|{addressing_mode}={address:x}) pop: ({:?})",
                            state.registers.cpsr().mode()
                        );
                    } else {
                        println!(
                            "Stack (={base:x}|{addressing_mode}={address:x}) push: ({:?})",
                            state.registers.cpsr().mode()
                        );
                    }
                }
                for register in register_list {
                    if is_load {
                        let value = state.memory.read_word(address);
                        if register_base == RegisterIndex::Sp && state.args.watch_stack {
                            println!("{register}(=0x{address:08x}): 0x{value:08x}");
                        }
                        let value = if register == RegisterIndex::Ip {
                            value
                                & if state.registers.cpsr().is_thumb() {
                                    0xffff_fffe
                                } else {
                                    0xffff_fffc
                                }
                        } else {
                            value
                        };
                        state.registers.write(register, value);
                    } else {
                        let value = state.registers.read(register);
                        if register_base == RegisterIndex::Sp && state.args.watch_stack {
                            println!("{register}(=0x{address:08x}): 0x{value:08x}");
                        }
                        state.memory.write_word_to(address, value);
                    }
                    address += 4;
                }
                let end_address = addressing_mode.end_address(base, register_list);
                assert_eq!(
                    address - 4,
                    end_address,
                    "actual: {address:x}, expected: {end_address:x}"
                );
                if register_base_write_back {
                    state.registers.write(
                        register_base,
                        addressing_mode.write_back(base, register_list),
                    );
                }
            }
            InstructionOp::Mrs {
                use_spsr,
                register_base,
            } => {
                let source = if use_spsr {
                    RegisterIndex::Spsr
                } else {
                    RegisterIndex::Cpsr
                };
                state
                    .registers
                    .write(register_base, state.registers.read(source));
            }
            InstructionOp::ShifterOperandInstruction {
                op,
                update_flags,
                base,
                destination,
                value,
            } => op.execute(state, update_flags, base, destination, value),
            InstructionOp::Msr {
                use_spsr,
                field_mask,
                operand,
            } => {
                let value = operand.resolve_mut(state, false);
                if value & Gba::UNALLOC_MASK > 0 {
                    todo!("UNPREDICTABLE")
                }
                let mask;
                if use_spsr {
                    let old_value = state.registers.read(RegisterIndex::Spsr);

                    if state.registers.cpsr().mode().unwrap().has_spsr() {
                        mask = field_mask.to_mask()
                            & (Gba::USER_MASK | Gba::PRIV_MASK | Gba::STATE_MASK);
                        state
                            .registers
                            .write(RegisterIndex::Spsr, (old_value & !mask) | (value & mask));
                    } else {
                        _ = dbg!(state.registers.cpsr().mode());
                        todo!("UNPREDICTABLE")
                    }
                } else {
                    let old_value = state.registers.read(RegisterIndex::Cpsr);

                    if state.registers.cpsr().mode().unwrap().is_privileged() {
                        if (value & Gba::STATE_MASK) > 0 {
                            todo!("UNPREDICTABLE")
                        } else {
                            mask = field_mask.to_mask() & (Gba::USER_MASK | Gba::PRIV_MASK);
                        }
                    } else {
                        mask = field_mask.to_mask() & Gba::USER_MASK;
                    }
                    state
                        .registers
                        .write(RegisterIndex::Cpsr, (old_value & !mask) | (value & mask));
                }
            }
        }
    }

    fn display(&self, condition: Condition, ctx: DisplayContext) -> DisplayedInstructionOp {
        DisplayedInstructionOp {
            op: *self,
            condition,
            ctx,
        }
    }

    /// ALU              1S          +1S+1N if R15 loaded, +1I if SHIFT(Rs)
    /// SWP              1S+2N+1I
    /// SWI,trap         2S+1N
    /// MUL              1S+ml

    /// MSR,MRS          1S
    /// LDR              1S+1N+1I    +1S+1N if R15 loaded
    /// STR              2N
    /// LDM              nS+1N+1I    +1S+1N if R15 loaded
    /// STM              (n-1)S+2N
    /// BL (THUMB)       3S+1N
    /// B,BL             2S+1N
    /// {cond} false     1S
    fn tick_duration(&self) -> usize {
        match self {
            InstructionOp::Branch {
                store_return_address_in_link_register,
                instruction_size,
                ..
            } => {
                if *store_return_address_in_link_register && *instruction_size == 2 {
                    4
                } else {
                    3
                }
            }
            InstructionOp::StoreOrLoadRegister {
                is_load, address, ..
            } => {
                if *is_load {
                    3 + if address.register_base == RegisterIndex::Ip {
                        2
                    } else {
                        0
                    }
                } else {
                    2
                }
            }
            InstructionOp::StoreOrLoadRegisters {
                is_load,
                register_list,
                ..
            } => {
                if *is_load {
                    register_list.len() as usize * 1
                        + 2
                        + if register_list.contains(RegisterIndex::Ip) {
                            2
                        } else {
                            0
                        }
                } else {
                    (register_list.len() as usize - 1) * 1 + 2
                }
            }
            InstructionOp::Mrs { .. } => 1,
            InstructionOp::Msr { .. } => 1,
            InstructionOp::ShifterOperandInstruction { op, .. } => match op {
                ShifterOperandInstructionOp::Move => 1,
                ShifterOperandInstructionOp::Compare => 1,
                ShifterOperandInstructionOp::CompareNegative => 1,
                ShifterOperandInstructionOp::And => 1,
                ShifterOperandInstructionOp::InclusiveOr => 1,
                ShifterOperandInstructionOp::ExclusiveOr => 1,
                ShifterOperandInstructionOp::Add => 1,
                ShifterOperandInstructionOp::AddWithCarry => 1,
                ShifterOperandInstructionOp::Subtract => 1,
                ShifterOperandInstructionOp::Multiplicate => {
                    1 + // TODO: read actual values to determine duration!
                    0
                }
                ShifterOperandInstructionOp::ReverseSubtract => 1,
                ShifterOperandInstructionOp::TestEquals => 1,
                ShifterOperandInstructionOp::Test => 1,
                ShifterOperandInstructionOp::MoveNegate => 1,
                ShifterOperandInstructionOp::BitClear => 1,
            },
        }
    }
}

// impl Debug for Instruction {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         self.op.write(f, self.condition)
//     }
// }

#[derive(Error, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum InstructionDecodeError {
    #[error("Unknown ARM Instruction 0x{0:08X} ({0:#b})")]
    UnknownArm(u32),
    #[error("Unknown Thumb Instruction 0x{0:04X} ({0:#b})")]
    UnknownThumb(u16),
    #[error("Unknown Thumb Instruction 0x{0:04X}{1:04X} ({0:#b}{1:b})")]
    UnknownThumbWide(u16, u16),
    #[error("Unknown Op Code 1 {0:03b} (Instruction was {1:#08X})")]
    UnknownOpCode1(u32, u32),
    #[error("Register Index out of range (<18) {0}")]
    InvalidRegisterIndex(u32),
    #[error("Invalid value of condition found.")]
    InvalidCondition(#[from] ConditionError),
    #[error("Invalid value for register list found.")]
    InvalidRegisterList,
    #[error("Invalid value for store load memory address found {0:x}.")]
    InvalidStoreLoadMemoryAddress(u32),
}

impl Debug for InstructionDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}
