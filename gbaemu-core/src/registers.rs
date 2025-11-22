use std::fmt::{Debug, Display};

use strum::IntoEnumIterator;

use crate::instructions::{
    display::{DisplayContext, DisplayedRegisterIndex, DisplayedRegisterList},
    InstructionFlags,
};

#[derive(Clone, Copy, Default)]
pub struct Registers([[u32; 18]; 6]);

impl Registers {
    pub fn new() -> Self {
        let mut result = Self([[0; 18]; 6]);
        result.write(RegisterIndex::Cpsr, 0xd3);
        result
    }

    pub fn from_extern(cpu: &armv4t_emu::Cpu) -> Self {
        let mut result = Self([[0; 18]; 6]);
        let mode = cpu.mode();
        for register in RegisterIndex::iter() {
            let bank_index = if register == RegisterIndex::Cpsr || register == RegisterIndex::Ip {
                0
            } else {
                Mode::from_extern(mode).index(register)
            };
            result.0[bank_index][register as u32 as usize] =
                cpu.reg_get(mode, register as u32 as u8);
        }
        result
    }

    pub(crate) fn read_raw(&self, register: RegisterIndex) -> u32 {
        let index = self.register_bank(register);
        self.0[index][register as u32 as usize]
    }

    pub fn read(&self, register: RegisterIndex) -> u32 {
        self.read_raw(register)
            + if register == RegisterIndex::Ip {
                if self.cpsr().is_thumb() {
                    4
                } else {
                    8
                }
            } else {
                0
            }
    }

    fn register_bank(&self, register: RegisterIndex) -> usize {
        if register == RegisterIndex::Cpsr || register == RegisterIndex::Ip {
            0
        } else {
            self.cpsr().mode().index(register)
        }
    }

    pub fn write(&mut self, register: RegisterIndex, value: u32) {
        let index = self.register_bank(register);
        self.0[index][register as u32 as usize] = value;
    }

    pub fn get_mut(&mut self, register: RegisterIndex) -> &mut u32 {
        let index = self.register_bank(register);
        &mut self.0[index][register as u32 as usize]
    }

    pub fn flags(&self) -> InstructionFlags {
        let cpsr = self.read(RegisterIndex::Cpsr);
        InstructionFlags {
            negative_flag: (cpsr & 1 << 31) > 0,
            zero_flag: (cpsr & 1 << 30) > 0,
            carry_flag: (cpsr & 1 << 29) > 0,
            overflow_flag: (cpsr & 1 << 28) > 0,
        }
    }

    pub fn update_nz_flags(&mut self, value: u32) {
        self.update_negative_flag((value & 1 << 31) > 0);
        self.update_zero_flag(value == 0);
    }

    pub fn update_nzv_flags(&mut self, (value, overflow): (i32, bool)) {
        self.update_negative_flag(value < 0);
        self.update_zero_flag(value == 0);
        self.update_overflow_flag(overflow);
    }

    pub fn update_negative_flag(&mut self, is_set: bool) {
        if is_set {
            *self.get_mut(RegisterIndex::Cpsr) |= 1 << 31;
        } else {
            *self.get_mut(RegisterIndex::Cpsr) &= !(1 << 31);
        }
    }

    pub fn update_zero_flag(&mut self, is_set: bool) {
        if is_set {
            *self.get_mut(RegisterIndex::Cpsr) |= 1 << 30;
        } else {
            *self.get_mut(RegisterIndex::Cpsr) &= !(1 << 30);
        }
    }

    pub fn update_carry_flag(&mut self, is_set: bool) {
        if is_set {
            *self.get_mut(RegisterIndex::Cpsr) |= 1 << 29;
        } else {
            *self.get_mut(RegisterIndex::Cpsr) &= !(1 << 29);
        }
    }

    pub fn update_overflow_flag(&mut self, is_set: bool) {
        if is_set {
            *self.get_mut(RegisterIndex::Cpsr) |= 1 << 28;
        } else {
            *self.get_mut(RegisterIndex::Cpsr) &= !(1 << 28);
        }
    }

    pub fn diff(&self, registers: Registers) -> impl Iterator<Item = RegisterIndex> + '_ {
        RegisterIndex::iter().filter(move |&r| self.read(r) != registers.read(r))
    }

    pub fn cpsr(&self) -> PsrRegister {
        PsrRegister(self.read(RegisterIndex::Cpsr))
    }

    pub(crate) fn cpsr_mut(&mut self) -> PsrRegisterMut<'_> {
        PsrRegisterMut(self.get_mut(RegisterIndex::Cpsr))
    }

    #[allow(dead_code)]
    pub(crate) fn dump(&self) {
        eprintln!("=== Registers: ===");
        for (index, register) in RegisterIndex::iter().enumerate() {
            eprint!("{register}: {:x}, ", self.read_raw(register));
            if index % 8 == 7 {
                eprintln!();
            }
        }
        eprintln!();
    }
}

impl Debug for Registers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mode = self.cpsr().mode();
        for (index, register) in RegisterIndex::iter().enumerate() {
            write!(
                f,
                "{register}: {:08x} ",
                self.0[mode.index(register)][register as u32 as usize]
            )?;
            if index % 8 == 7 {
                writeln!(f)?;
            }
        }
        Ok(())
    }
}

pub struct PsrRegisterMut<'a>(&'a mut u32);
impl<'a> PsrRegisterMut<'a> {
    pub(crate) fn set_is_thumb(&mut self, is_thumb: bool) {
        if is_thumb {
            *self.0 |= 0x20;
        } else {
            *self.0 &= !0x20;
        }
    }

    pub fn set_mode(&mut self, mode: Mode) {
        let raw_mode = match mode {
            Mode::User => 0b10000,
            Mode::Fiq => 0b10001,
            Mode::Irq => 0b10010,
            Mode::Supervisor => 0b10011,
            Mode::Abort => 0b10111,
            Mode::Undefined => 0b11011,
            Mode::System => 0b11111,
        };
        *self.0 = (raw_mode & 0x1f) | (*self.0 & !0x1f);
    }
}

pub struct PsrRegister(u32);
impl PsrRegister {
    pub(crate) fn is_thumb(&self) -> bool {
        self.0 & 0x20 > 0
    }

    pub fn mode(&self) -> Mode {
        let raw_mode = self.0 & 0x1f;
        match raw_mode {
            0b10000 => Mode::User,
            0b10001 => Mode::Fiq,
            0b10010 => Mode::Irq,
            0b10011 => Mode::Supervisor,
            0b10111 => Mode::Abort,
            0b11011 => Mode::Undefined,
            0b11111 => Mode::System,
            _ => unreachable!("Invalid mode found: {raw_mode:05b}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    User,
    Fiq,
    Irq,
    Supervisor,
    Abort,
    Undefined,
    System,
}
impl Mode {
    pub(crate) fn is_privileged(&self) -> bool {
        *self != Mode::User
    }

    pub(crate) fn has_spsr(&self) -> bool {
        *self != Mode::User && *self != Mode::System
    }

    fn index(&self, register: RegisterIndex) -> usize {
        const BANKED_REGISTERS: [RegisterIndex; 3] =
            [RegisterIndex::Sp, RegisterIndex::Lr, RegisterIndex::Spsr];
        if register < RegisterIndex::R8 {
            return 0;
        }
        match self {
            Self::User | Self::System => 0,
            Self::Supervisor => {
                if BANKED_REGISTERS.contains(&register) {
                    1
                } else {
                    0
                }
            }

            Self::Abort => {
                if BANKED_REGISTERS.contains(&register) {
                    2
                } else {
                    0
                }
            }

            Self::Undefined => {
                if BANKED_REGISTERS.contains(&register) {
                    3
                } else {
                    0
                }
            }

            Self::Irq => {
                if BANKED_REGISTERS.contains(&register) {
                    4
                } else {
                    0
                }
            }

            Self::Fiq => 5,
        }
    }

    fn from_extern(mode: armv4t_emu::Mode) -> Self {
        match mode {
            armv4t_emu::Mode::User => Self::User,
            armv4t_emu::Mode::Fiq => Self::Fiq,
            armv4t_emu::Mode::Irq => Self::Irq,
            armv4t_emu::Mode::Supervisor => Self::Supervisor,
            armv4t_emu::Mode::Abort => Self::Abort,
            armv4t_emu::Mode::Undefined => Self::Undefined,
            armv4t_emu::Mode::System => Self::System,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RegisterList(u32);
impl RegisterList {
    pub(crate) fn len(&self) -> u32 {
        self.0.count_ones()
    }

    pub fn with(mut self, register: RegisterIndex) -> Self {
        self.0 |= 1 << register as usize;
        self
    }

    #[allow(dead_code)]
    pub fn from_registers(registers: impl IntoIterator<Item = RegisterIndex>) -> RegisterList {
        let mut raw = 0;
        for r in registers {
            raw |= 1 << (r) as usize;
        }
        Self(raw)
    }

    pub(crate) fn byte_len(&self) -> u32 {
        self.len() * 4
    }

    pub fn display(&self, ctx: DisplayContext) -> DisplayedRegisterList {
        DisplayedRegisterList {
            register_list: *self,
            ctx,
        }
    }

    pub fn contains(&self, register: RegisterIndex) -> bool {
        self.into_iter().any(|r| r == register)
    }
}

impl TryFrom<u32> for RegisterList {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if value > u16::MAX as u32 {
            Err(())
        } else {
            Ok(RegisterList(value))
        }
    }
}

impl IntoIterator for RegisterList {
    type Item = RegisterIndex;

    type IntoIter = RegisterListIterator;

    fn into_iter(self) -> Self::IntoIter {
        RegisterListIterator(self, 0)
    }
}
pub struct RegisterListIterator(RegisterList, u32);
impl Iterator for RegisterListIterator {
    type Item = RegisterIndex;

    fn next(&mut self) -> Option<Self::Item> {
        let result = loop {
            if self.1 > 17 {
                return None;
            }
            if self.0 .0 & 1 << self.1 > 0 {
                break Some(RegisterIndex::try_from(self.1).unwrap());
            }
            self.1 += 1;
        };
        self.1 += 1;
        result
    }
}

#[repr(u32)]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    strum::EnumIter,
    PartialOrd,
    Ord,
    Hash,
    strum::FromRepr,
    strum::AsRefStr,
    strum::EnumString,
)]
#[strum(serialize_all = "UPPERCASE")]
pub enum RegisterIndex {
    R0,
    R1,
    R2,
    R3,
    R4,
    R5,
    R6,
    R7,
    R8,
    R9,
    R10,
    R11,
    R12,
    Sp,
    Lr,
    Ip,
    Cpsr,
    Spsr,
}

impl RegisterIndex {
    pub fn display(&self, ctx: DisplayContext) -> DisplayedRegisterIndex {
        DisplayedRegisterIndex {
            register: *self,
            ctx,
        }
    }
}

impl Display for RegisterIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegisterIndex::R0 => write!(f, "R0"),
            RegisterIndex::R1 => write!(f, "R1"),
            RegisterIndex::R2 => write!(f, "R2"),
            RegisterIndex::R3 => write!(f, "R3"),
            RegisterIndex::R4 => write!(f, "R4"),
            RegisterIndex::R5 => write!(f, "R5"),
            RegisterIndex::R6 => write!(f, "R6"),
            RegisterIndex::R7 => write!(f, "R7"),
            RegisterIndex::R8 => write!(f, "R8"),
            RegisterIndex::R9 => write!(f, "R9"),
            RegisterIndex::R10 => write!(f, "R10"),
            RegisterIndex::R11 => write!(f, "R11"),
            RegisterIndex::R12 => write!(f, "R12"),
            RegisterIndex::Sp => write!(f, "SP"),
            RegisterIndex::Lr => write!(f, "LR"),
            RegisterIndex::Ip => write!(f, "IP"),
            RegisterIndex::Cpsr => write!(f, "CPSR"),
            RegisterIndex::Spsr => write!(f, "SPSR"),
        }
    }
}

impl TryFrom<u32> for RegisterIndex {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::R0,
            1 => Self::R1,
            2 => Self::R2,
            3 => Self::R3,
            4 => Self::R4,
            5 => Self::R5,
            6 => Self::R6,
            7 => Self::R7,
            8 => Self::R8,
            9 => Self::R9,
            10 => Self::R10,
            11 => Self::R11,
            12 => Self::R12,
            13 => Self::Sp,
            14 => Self::Lr,
            15 => Self::Ip,
            16 => Self::Cpsr,
            17 => Self::Spsr,
            _ => return Err(()),
        })
    }
}
