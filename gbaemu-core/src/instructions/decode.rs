use super::*;

impl Instruction {
    pub fn decode_arm(word: u32) -> Result<Self, InstructionDecodeError> {
        let condition = Condition::try_from((word >> 28) as u8)
            .map_err(|()| InstructionDecodeError::InvalidCondition)?;
        let op_code_1 = (word & 0x0E00_0000) >> 25;
        match op_code_1 {
            0b101 => {
                // Branch
                let store_return_address_in_link_register = (word & 1 << 24) > 0;
                let mut offset = word & 0x00FF_FFFF;
                // Sign extend the offset.
                if (offset & 1 << 23) > 0 {
                    offset |= 0xFF00_0000
                }
                offset <<= 2;
                let offset = offset as i32;
                Ok(Instruction {
                    condition,
                    op: InstructionOp::Branch {
                        target: BranchTarget::Offset(offset),
                        store_return_address_in_link_register,
                        does_switch_mode: false,
                        return_address_is_thumb: false,
                        instruction_size: 4,
                    },
                })
            }
            0b010 | 0b011 => {
                // LDR | STR
                let read_byte_instead_of_word = (word & 1 << 22) > 0;
                let is_load = (word & 1 << 20) > 0;
                let address = StoreLoadMemoryAddress::try_from(word)?;
                let rd = (word & 0xF000) >> 12;
                let rd = RegisterIndex::try_from(rd)
                    .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rd))?;
                // if rd == RegisterIndex::Ip && is_load {
                //     // let offset = (offset | 0xffff_f000) as i32;
                //     Ok(Instruction {
                //         condition,
                //         op: InstructionOp::Branch {
                //             store_return_address_in_link_register: false,
                //             return_address_is_thumb: false,
                //             does_switch_mode: true,
                //             target: todo!(),
                //             instruction_size: 4,
                //         },
                //     })
                // } else {
                Ok(Instruction {
                    condition,
                    op: InstructionOp::StoreOrLoadRegister {
                        is_load,
                        address,
                        register_destination: rd,
                        read_size: if read_byte_instead_of_word {
                            StoreLoadMemorySize::Byte
                        } else {
                            StoreLoadMemorySize::Word
                        },
                    },
                })
                // }
                // } else {
                //     dbg!(
                //         is_immediate,
                //         is_post_indexing,
                //         negate_offset,
                //         read_byte_instead_of_word,
                //         w_bit,
                //         is_load
                //     );
                //     Err(InstructionDecodeError::UnknownArm(word))
                // }
            }
            0b001 | 0b000 => {
                let rn = (word & 0xF_0000) >> 16;
                let rn = RegisterIndex::try_from(rn)
                    .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rn))?;
                let rd = (word & 0xF000) >> 12;
                let rd = RegisterIndex::try_from(rd)
                    .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rd))?;
                let update_flags = (word & 1 << 20) > 0;
                let is_immediate = (word & 1 << 25) > 0;

                if word & 0x2000090 == 0x90 {
                    let op_code_3 = (word & 0xf0) >> 4;
                    let address = StoreLoadMemoryAddress::from_addressing_mode_3(word)?;
                    let is_load = (word & 1 << 20) > 0;
                    return match op_code_3 {
                        0b1011 => Ok(Instruction {
                            condition,
                            op: InstructionOp::StoreOrLoadRegister {
                                is_load,
                                address,
                                register_destination: rd,
                                read_size: StoreLoadMemorySize::Halfword,
                            },
                        }),
                        0b1111 => Ok(Instruction {
                            condition,
                            op: InstructionOp::StoreOrLoadRegister {
                                is_load,
                                address,
                                register_destination: rd,
                                read_size: StoreLoadMemorySize::SignedHalfword,
                            },
                        }),
                        0b1001 => {
                            let rm = (word & 0xF00) >> 8;
                            let rm = RegisterIndex::try_from(rm)
                                .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rm))?;

                            let rn = word & 0xF;
                            let rn = RegisterIndex::try_from(rn)
                                .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rn))?;
                            let rd = (word & 0xF0000) >> 16;
                            let rd = RegisterIndex::try_from(rd)
                                .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rd))?;

                            Ok(Instruction {
                                condition,
                                op: InstructionOp::ShifterOperandInstruction {
                                    op: ShifterOperandInstructionOp::Multiplicate,
                                    update_flags,
                                    base: rn,
                                    destination: rd,
                                    value: ShifterOperand::Register(rm),
                                },
                            })
                        }
                        _ => Err(InstructionDecodeError::UnknownArm(word)),
                    };
                    // return Ok(Instruction {
                    //     condition,
                    //     op: InstructionOp::StoreOrLoadRegister {
                    //         is_load,
                    //         address,
                    //         register_destination: rd,
                    //         read_size: StoreLoadMemorySize::Halfword,
                    //     },
                    // });
                } else if word & 0x0FB00000 == 0x1000000 {
                    let use_spsr = (word & 1 << 22) > 0;
                    let sbo = (word & 0xF_0000) >> 16;
                    assert_eq!(sbo, 0xF, "Should be ones!");
                    let sbz = word & 0xFFF;
                    assert_eq!(sbz, 0x0, "Should be zeroes!");
                    return Ok(Instruction {
                        condition,
                        op: InstructionOp::Mrs {
                            use_spsr,
                            register_base: rd,
                        },
                    });
                } else if word & 0x0FF0_00F0 == 0x1200010 {
                    // BX
                    let sbo = (word & 0xfff00) >> 8;
                    assert_eq!(sbo, 0xfff, "Should be ones!");
                    let rm = word & 0xF;
                    let rm = RegisterIndex::try_from(rm)
                        .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rm))?;
                    return Ok(Instruction {
                        condition,
                        op: InstructionOp::Branch {
                            store_return_address_in_link_register: false,
                            return_address_is_thumb: false,
                            does_switch_mode: true,
                            target: BranchTarget::RegisterWithOffset(rm, 0),
                            instruction_size: 4,
                        },
                    });
                } else if word & 0x0DB00000 == 0x0120_0000 {
                    let use_spsr = (word & 1 << 22) > 0;
                    let sbo = (word & 0xf000) >> 12;
                    assert_eq!(sbo, 0xf, "Should be ones!");
                    let operand = if is_immediate {
                        ShifterOperand::try_from(word)?
                    } else {
                        let sbz = (word & 0xff0) >> 4;
                        assert_eq!(sbz, 0, "Should be zeros!");
                        let rm = word & 0xF;
                        let rm = RegisterIndex::try_from(rm)
                            .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rm))?;
                        ShifterOperand::Register(rm)
                    };
                    let field_mask = (word & 0xF_0000) >> 16;
                    let field_mask = MsrFieldMask::try_from(field_mask as u8).expect("TODO");
                    return Ok(Instruction {
                        condition,
                        op: InstructionOp::Msr {
                            use_spsr,
                            field_mask,
                            operand,
                        },
                    });
                }

                let op_code_2 = (word & 0xDE00000) >> 21;
                if word & 0x2000090 == 0x90 {
                    return Err(InstructionDecodeError::UnknownArm(word));
                }

                let op = match op_code_2 {
                    0x00 => ShifterOperandInstructionOp::And,
                    0x01 => ShifterOperandInstructionOp::ExclusiveOr,
                    0x02 => ShifterOperandInstructionOp::Subtract,
                    0x03 => ShifterOperandInstructionOp::ReverseSubtract,
                    0x04 => ShifterOperandInstructionOp::Add,
                    0x05 => ShifterOperandInstructionOp::AddWithCarry,
                    0x08 => ShifterOperandInstructionOp::Test,
                    0x09 => {
                        assert!(update_flags, "value was {word:x}");
                        ShifterOperandInstructionOp::TestEquals
                    }
                    0x0A => {
                        assert!(update_flags);
                        ShifterOperandInstructionOp::Compare
                    }
                    0x0C => ShifterOperandInstructionOp::InclusiveOr,
                    0x0D => {
                        assert_eq!(rn, RegisterIndex::R0);

                        ShifterOperandInstructionOp::Move
                    }
                    0x0E => ShifterOperandInstructionOp::BitClear,
                    unknown_op_code_2 => {
                        println!("op_code_1: {op_code_1:03b}");
                        println!("op_code_2: {unknown_op_code_2:08b}");
                        return Err(InstructionDecodeError::UnknownArm(word));
                    }
                };
                Ok(Instruction {
                    condition,
                    op: InstructionOp::ShifterOperandInstruction {
                        op,
                        update_flags,
                        base: rn,
                        destination: rd,
                        value: ShifterOperand::try_from(word)?,
                    },
                })
            }
            0b100 => {
                let is_post_indexing = (word & 1 << 24) == 0;
                let negate_offset = (word & 1 << 23) == 0;
                let s_bit = (word & 1 << 22) > 0;
                assert!(!s_bit);
                let register_base_write_back = (word & 1 << 21) > 0;
                let is_load = (word & 1 << 20) > 0;
                let rn = (word & 0xF_0000) >> 16;
                let rn = RegisterIndex::try_from(rn)
                    .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rn))?;
                let register_list = RegisterList::try_from(word & 0xFFFF).unwrap();
                Ok(Instruction {
                    condition,
                    op: InstructionOp::StoreOrLoadRegisters {
                        is_load,
                        register_list,
                        register_base: rn,
                        register_base_write_back,
                        addressing_mode: StoreLoadManyAddressingMode::from_flags(
                            is_post_indexing,
                            negate_offset,
                        ),
                    },
                })
            }
            _ => Err(InstructionDecodeError::UnknownOpCode1(op_code_1, word)),
        }
    }

    pub(crate) fn decode_thumb(
        half_word: u16,
        next_half_word: u16,
    ) -> Result<Instruction, InstructionDecodeError> {
        let op_code_1 = (half_word & 0xE000) >> 13;
        match op_code_1 {
            0b000 => thumb_op_code_000(half_word),
            0b001 => thumb_op_code_001(half_word),
            0b010 => thumb_op_code_010(half_word),
            0b011 => thumb_op_code_011(half_word),
            0b100 => thumb_op_code_100(half_word),
            0b101 => thumb_op_code_101(half_word),
            0b110 => thumb_op_code_110(half_word),
            0b111 => thumb_op_code_111(half_word, next_half_word),
            unknown_op_code_1 => {
                println!("op_code_1: {unknown_op_code_1:03b}");
                Err(InstructionDecodeError::UnknownThumb(half_word))
            }
        }
    }
}

fn thumb_op_code_111(
    half_word: u16,
    next_half_word: u16,
) -> Result<Instruction, InstructionDecodeError> {
    if half_word & 0x1800 == 0x1000 {
        let mut first_offset = (half_word & 0x07FF) as u32;
        if (first_offset & 0x0400) > 0 {
            first_offset |= 0xFFFF_F800;
        }
        let first_offset = first_offset << 12;

        let does_switch_mode = (next_half_word & 0x1000) == 0;
        let h = (next_half_word & 0x0800) > 0;
        assert!(h);
        let second_offset = (next_half_word & 0x07FF) as u32;
        let second_offset = second_offset << 1;
        let offset = (first_offset
            | if does_switch_mode {
                second_offset & !2
            } else {
                second_offset
            }) as i32;

        Ok(Instruction {
            condition: Condition::Always,
            op: InstructionOp::Branch {
                store_return_address_in_link_register: true,
                return_address_is_thumb: true,
                does_switch_mode,
                target: BranchTarget::Offset(offset),
                instruction_size: 4,
            },
        })
    } else if half_word & 0x1800 == 0x00000 {
        let mut offset = (half_word & 0x07FF) << 1;
        if (offset & 0x0800) > 0 {
            offset |= 0xF000;
        }

        let offset = offset as i16 as i32;
        Ok(Instruction {
            condition: Condition::Always,
            op: InstructionOp::Branch {
                store_return_address_in_link_register: false,
                return_address_is_thumb: true,
                does_switch_mode: false,
                target: BranchTarget::Offset(offset),
                instruction_size: 2,
            },
        })
    } else if half_word & 0x1800 == 0x1800 {
        // This is valid on GBA
        let does_switch_mode = (half_word & 0x1000) == 0;
        let h = (half_word & 0x0800) > 0;
        assert!(h);
        let offset = (half_word & 0x07FF) as u32;
        let offset = offset << 1;
        let offset = offset as i32;
        Ok(Instruction {
            condition: Condition::Always,
            op: InstructionOp::Branch {
                store_return_address_in_link_register: true,
                return_address_is_thumb: true,
                does_switch_mode,
                target: BranchTarget::Offset(offset),
                instruction_size: 2,
            },
        })
    } else {
        Err(InstructionDecodeError::UnknownThumb(half_word))
    }
}

fn thumb_op_code_110(half_word: u16) -> Result<Instruction, InstructionDecodeError> {
    if (half_word & 1 << 12) > 0 {
        let condition = Condition::try_from(((half_word & 0x0f00) >> 8) as u8)
            .map_err(|()| InstructionDecodeError::InvalidCondition)?;
        let offset = (half_word & 0xff) as i8;
        let offset = (offset as i32) << 1;
        Ok(Instruction {
            condition,
            op: InstructionOp::Branch {
                store_return_address_in_link_register: false,
                return_address_is_thumb: true,
                does_switch_mode: false,
                target: BranchTarget::Offset(offset),
                instruction_size: 2,
            },
        })
    } else if half_word & 0x1000 == 0x0000 {
        let is_load = (half_word & 0x0800) > 0;
        let rn = ((half_word & 0x0700) >> 8) as u32;
        let rn = RegisterIndex::try_from(rn)
            .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rn))?;
        let register_list = RegisterList::try_from((half_word & 0x00ff) as u32)
            .map_err(|()| InstructionDecodeError::InvalidRegisterList)?;
        Ok(Instruction {
            condition: Condition::Always,
            op: InstructionOp::StoreOrLoadRegisters {
                is_load,
                register_base: rn,
                register_list,
                register_base_write_back: true,
                addressing_mode: StoreLoadManyAddressingMode::IncrementAfter,
            },
        })
    } else {
        Err(InstructionDecodeError::UnknownThumb(half_word))
    }
}

fn thumb_op_code_101(half_word: u16) -> Result<Instruction, InstructionDecodeError> {
    let op_code = (half_word & 0x1f00) >> 8;
    match op_code {
        0b01000..=0b01111 | 0b00000..=0b00111 => {
            let is_sp = (half_word & 0x0800) > 0;
            let base = if is_sp {
                RegisterIndex::Sp
            } else {
                RegisterIndex::Ip
            };
            let immediate = (half_word & 0x00ff) as u32;
            let rd = ((half_word & 0x0700) >> 8) as u32;
            let rd = RegisterIndex::try_from(rd)
                .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rd))?;
            Ok(Instruction {
                condition: Condition::Always,
                op: InstructionOp::ShifterOperandInstruction {
                    op: ShifterOperandInstructionOp::Add,
                    update_flags: false,
                    base,
                    destination: rd,
                    value: ShifterOperand::Immediate(immediate << 2, None),
                },
            })
        }
        0b10000 => {
            let is_sub = (half_word & 1 << 7) > 0;
            let op = if is_sub {
                ShifterOperandInstructionOp::Subtract
            } else {
                ShifterOperandInstructionOp::Add
            };
            let immediate = ((half_word & 0x007f) << 2) as _;
            Ok(Instruction {
                condition: Condition::Always,
                op: InstructionOp::ShifterOperandInstruction {
                    op,
                    update_flags: false,
                    base: RegisterIndex::Sp,
                    destination: RegisterIndex::Sp,
                    value: ShifterOperand::Immediate(immediate, None),
                },
            })
        }
        0b10100 | 0b10101 => {
            let has_lr = (half_word & 0x0100) > 0;
            let value = (has_lr as u32) << 14 | (half_word as u32 & 0x00ff);
            let register_list = RegisterList::try_from(value)
                .map_err(|()| InstructionDecodeError::InvalidRegisterList)?;
            Ok(Instruction {
                condition: Condition::Always,
                op: InstructionOp::StoreOrLoadRegisters {
                    is_load: false,
                    register_base: RegisterIndex::Sp,
                    register_list,
                    register_base_write_back: true,
                    addressing_mode: StoreLoadManyAddressingMode::DecrementBefore,
                },
            })
        }
        0b11100 | 0b11101 => {
            let includes_ip = (half_word & 1 << 8) > 0;
            let register_list = half_word & 0xff | if includes_ip { 0x8000 } else { 0 };
            let register_list = RegisterList::try_from(register_list as u32)
                .map_err(|()| InstructionDecodeError::InvalidRegisterList)?;
            Ok(Instruction {
                condition: Condition::Always,
                op: InstructionOp::StoreOrLoadRegisters {
                    is_load: true,
                    register_base: RegisterIndex::Sp,
                    register_base_write_back: true,
                    register_list,
                    addressing_mode: StoreLoadManyAddressingMode::IncrementAfter,
                },
            })
        }
        err => {
            println!("op_code_1 was 0b101 and op_code_2 was {err:05b}");
            Err(InstructionDecodeError::UnknownThumb(half_word))
        }
    }
}

fn thumb_op_code_100(half_word: u16) -> Result<Instruction, InstructionDecodeError> {
    let is_load = (half_word & 0x0800) > 0;
    let use_second_register = (half_word & 0x1000) == 0;
    let rd = if use_second_register {
        (half_word & 0x0007) as u32
    } else {
        ((half_word & 0x0700) >> 8) as u32
    };
    let rd = RegisterIndex::try_from(rd)
        .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rd))?;

    let rn = if use_second_register {
        let rn = ((half_word & 0x0038) >> 3) as u32;
        RegisterIndex::try_from(rn)
            .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rn))?
    } else {
        RegisterIndex::Sp
    };
    let read_size = if use_second_register {
        StoreLoadMemorySize::Halfword
    } else {
        StoreLoadMemorySize::Word
    };
    let immediate = if use_second_register {
        ((half_word & 0x07C0) >> 5) as u32
    } else {
        ((half_word & 0xff) << 2) as u32
    };
    Ok(Instruction {
        condition: Condition::Always,
        op: InstructionOp::StoreOrLoadRegister {
            is_load,
            address: StoreLoadMemoryAddress {
                register_base: rn,
                offset: ShifterOperand::Immediate(immediate, None),
                ignore_bit_1_of_pc: false,
                write_back: false,
                negate_offset: false,
                is_post_indexing: false,
            },
            register_destination: rd,
            read_size,
        },
    })
}

fn thumb_op_code_011(half_word: u16) -> Result<Instruction, InstructionDecodeError> {
    let rd = (half_word & 0x0007) as u32;
    let rd = RegisterIndex::try_from(rd)
        .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rd))?;
    let rm = ((half_word & 0x0038) >> 3) as u32;
    let rm = RegisterIndex::try_from(rm)
        .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rm))?;
    let immediate = ((half_word & 0x07C0) >> 6) as u32;
    let is_byte = (half_word & 0x1000) > 0;
    let immediate = if is_byte { immediate } else { immediate << 2 };
    let is_load = (half_word & 0x0800) > 0;
    Ok(Instruction {
        condition: Condition::Always,
        op: InstructionOp::StoreOrLoadRegister {
            is_load,
            address: StoreLoadMemoryAddress {
                register_base: rm,
                offset: ShifterOperand::Immediate(immediate, None),
                ignore_bit_1_of_pc: false,
                write_back: false,
                negate_offset: false,
                is_post_indexing: false,
            },
            register_destination: rd,
            read_size: if is_byte {
                StoreLoadMemorySize::Byte
            } else {
                StoreLoadMemorySize::Word
            },
        },
    })
}

fn thumb_op_code_010(half_word: u16) -> Result<Instruction, InstructionDecodeError> {
    let op_code_2 = (half_word & 0x1C00) >> 10;
    match op_code_2 {
        0b000 => {
            let alu_op_code = (half_word & 0x03C0) >> 6;
            let rm = (half_word & 0x0038) >> 3;
            let rm = RegisterIndex::try_from(rm as u32)
                .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rm as _))?;
            let rd = half_word & 0x0007;
            let rd = RegisterIndex::try_from(rd as u32)
                .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rd as _))?;
            let (op, value, base) = match alu_op_code {
                0b0000 => (
                    ShifterOperandInstructionOp::And,
                    ShifterOperand::Register(rm),
                    rd,
                ),
                0b0001 => (
                    ShifterOperandInstructionOp::ExclusiveOr,
                    ShifterOperand::Register(rm),
                    rd,
                ),
                0b0111 => (
                    ShifterOperandInstructionOp::Move,
                    ShifterOperand::RegisterRegister(rd, ShiftOperator::RotateRight, rm),
                    rd,
                ),
                0b1000 => (
                    ShifterOperandInstructionOp::Test,
                    ShifterOperand::Register(rm),
                    rd,
                ),
                0b1001 => (
                    ShifterOperandInstructionOp::ReverseSubtract,
                    ShifterOperand::Immediate(0, None),
                    rm,
                ),
                0b1010 => (
                    ShifterOperandInstructionOp::Compare,
                    ShifterOperand::Register(rm),
                    rd,
                ),
                0b1011 => (
                    ShifterOperandInstructionOp::CompareNegative,
                    ShifterOperand::Register(rm),
                    rd,
                ),
                0b1100 => (
                    ShifterOperandInstructionOp::InclusiveOr,
                    ShifterOperand::Register(rm),
                    rd,
                ),
                0b1110 => (
                    ShifterOperandInstructionOp::BitClear,
                    ShifterOperand::Register(rm),
                    rd,
                ),
                0b1101 => (
                    ShifterOperandInstructionOp::Multiplicate,
                    ShifterOperand::Register(rm),
                    rd,
                ),
                0b1111 => (
                    ShifterOperandInstructionOp::MoveNegate,
                    ShifterOperand::Register(rm),
                    rd,
                ),
                unknown_alu_op_code => {
                    println!("alu_op_code: {unknown_alu_op_code:04b}");
                    return Err(InstructionDecodeError::UnknownThumb(half_word));
                }
            };
            Ok(Instruction {
                condition: Condition::Always,
                op: InstructionOp::ShifterOperandInstruction {
                    op,
                    update_flags: true,
                    base,
                    destination: rd,
                    value,
                },
            })
        }
        0b001 => {
            let op_code_3 = (half_word & 0x0380) >> 7;
            match op_code_3 {
                0b010 | 0b011 => {
                    let h1 = (half_word & 1 << 7) > 0;
                    let h2 = (half_word & 1 << 6) > 0;
                    let rd = (half_word & 0x0007) | ((h1 as u16) << 3);
                    let rm = ((half_word & 0x0038) >> 3) | ((h2 as u16) << 3);
                    let rd = rd as u32;
                    let rm = rm as u32;
                    let rd = RegisterIndex::try_from(rd)
                        .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rd))?;
                    let rm = RegisterIndex::try_from(rm)
                        .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rm))?;
                    Ok(Instruction {
                        condition: Condition::Always,
                        op: InstructionOp::ShifterOperandInstruction {
                            op: ShifterOperandInstructionOp::Compare,
                            update_flags: true,
                            base: rd,
                            destination: RegisterIndex::R0,
                            value: ShifterOperand::Register(rm),
                        },
                    })
                }
                0b110 => {
                    let rm = (half_word & 0x0078) >> 3;
                    let rm = RegisterIndex::try_from(rm as u32)
                        .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rm as _))?;
                    let sbz = half_word & 0x7;
                    assert_eq!(sbz, 0, "Should be zero!");
                    Ok(Instruction {
                        condition: Condition::Always,
                        op: InstructionOp::Branch {
                            store_return_address_in_link_register: false,
                            return_address_is_thumb: true,
                            does_switch_mode: true,
                            target: BranchTarget::RegisterWithOffset(rm, 0),
                            instruction_size: 2,
                        },
                    })
                }
                0b100 | 0b101 => {
                    let h1 = (half_word & 1 << 7) > 0;
                    let h2 = (half_word & 1 << 6) > 0;
                    let rd = (half_word & 0x0007) | ((h1 as u16) << 3);
                    let rm = ((half_word & 0x0038) >> 3) | ((h2 as u16) << 3);
                    let rd = rd as u32;
                    let rm = rm as u32;
                    let rd = RegisterIndex::try_from(rd)
                        .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rd))?;
                    let rm = RegisterIndex::try_from(rm)
                        .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rm))?;
                    Ok(Instruction {
                        condition: Condition::Always,
                        op: InstructionOp::ShifterOperandInstruction {
                            op: ShifterOperandInstructionOp::Move,
                            update_flags: false,
                            base: RegisterIndex::R0,
                            destination: rd,
                            value: ShifterOperand::Register(rm),
                        },
                    })
                }
                0b111 => {
                    let rm = ((half_word & 0x0078) >> 3) as u32;
                    let rm = RegisterIndex::try_from(rm)
                        .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rm))?;
                    let sbz = half_word & 0x0007;
                    assert_eq!(sbz, 0, "Should be zero");
                    Ok(Instruction {
                        condition: Condition::Always,
                        op: InstructionOp::Branch {
                            store_return_address_in_link_register: true,
                            return_address_is_thumb: true,
                            does_switch_mode: false,
                            target: BranchTarget::RegisterWithOffset(rm, 0),
                            instruction_size: 2,
                        },
                    })
                }
                unknown => {
                    println!("New High Register or BX instruction! (op_code_3 = {unknown:03b})");
                    Err(InstructionDecodeError::UnknownThumb(half_word))
                }
            }
        }
        0b010 | 0b011 => {
            let immediate = ((half_word & 0x00ff) << 2) as u32;
            let rd = ((half_word & 0x0700) >> 8) as u32;
            let rd = RegisterIndex::try_from(rd)
                .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rd))?;
            Ok(Instruction {
                condition: Condition::Always,
                op: InstructionOp::StoreOrLoadRegister {
                    is_load: true,
                    address: StoreLoadMemoryAddress {
                        register_base: RegisterIndex::Ip,
                        offset: ShifterOperand::Immediate(immediate, None),
                        negate_offset: false,
                        write_back: false,
                        ignore_bit_1_of_pc: true,
                        is_post_indexing: false,
                    },
                    register_destination: rd,
                    read_size: StoreLoadMemorySize::Word,
                },
            })
        }
        0b100 if (half_word & 1 << 9) == 0 => {
            let rm = (half_word & 0x01C0) >> 6;
            let rm = RegisterIndex::try_from(rm as u32)
                .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rm as _))?;
            let rn = (half_word & 0x0038) >> 3;
            let rn = RegisterIndex::try_from(rn as u32)
                .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rn as _))?;
            let rd = half_word & 0x0007;
            let rd = RegisterIndex::try_from(rd as u32)
                .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rd as _))?;

            Ok(Instruction {
                condition: Condition::Always,
                op: InstructionOp::StoreOrLoadRegister {
                    is_load: false,
                    address: StoreLoadMemoryAddress {
                        register_base: rn,
                        offset: ShifterOperand::Register(rm),
                        negate_offset: false,
                        write_back: false,
                        ignore_bit_1_of_pc: false,
                        is_post_indexing: false,
                    },
                    register_destination: rd,
                    read_size: StoreLoadMemorySize::Word,
                },
            })
        }
        0b100..=0b111 => {
            let is_halfword_instead_of_byte = (half_word & 1 << 9) > 0;
            let is_load = (half_word & 1 << 11) > 0;
            let is_word = (half_word & 1 << 10) > 0;
            let read_size = match (is_word, is_halfword_instead_of_byte) {
                (false, true) => StoreLoadMemorySize::Halfword,
                (false, false) => StoreLoadMemorySize::Word,
                (true, false) => StoreLoadMemorySize::Word,
                (true, true) => StoreLoadMemorySize::Byte,
                // _ => unreachable!(),
            };

            let rm = (half_word & 0x01C0) >> 6;
            let rm = RegisterIndex::try_from(rm as u32)
                .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rm as _))?;
            let rn = (half_word & 0x0038) >> 3;
            let rn = RegisterIndex::try_from(rn as u32)
                .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rn as _))?;
            let rd = half_word & 0x0007;
            let rd = RegisterIndex::try_from(rd as u32)
                .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rd as _))?;

            Ok(Instruction {
                condition: Condition::Always,
                op: InstructionOp::StoreOrLoadRegister {
                    is_load,
                    address: StoreLoadMemoryAddress {
                        ignore_bit_1_of_pc: false,
                        register_base: rn,
                        write_back: false,
                        negate_offset: false,
                        offset: ShifterOperand::Register(rm),
                        is_post_indexing: false,
                    },
                    register_destination: rd,
                    read_size,
                },
            })

            // println!("New load and store instruction!, op = {op:02b}");
            // Err(InstructionDecodeError::UnknownThumb(half_word))
        }
        unknown_op_code_2 => {
            println!("op_code_2: {unknown_op_code_2:03b}");
            unreachable!("op_code_2 should be < 0b111");
        }
    }
}

fn thumb_op_code_001(half_word: u16) -> Result<Instruction, InstructionDecodeError> {
    let op = match (half_word & 0x1800) >> 11 {
        0b00 => ShifterOperandInstructionOp::Move,
        0b01 => ShifterOperandInstructionOp::Compare,
        0b10 => ShifterOperandInstructionOp::Add,
        0b11 => ShifterOperandInstructionOp::Subtract,
        unknown_op_code_2 => {
            unreachable!("op_code_1: 0b001 and unknown_op_code_2: {unknown_op_code_2:02b}");
        }
    };
    let rd = (half_word & 0x0700) >> 8;
    let rd = RegisterIndex::try_from(rd as u32)
        .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rd as _))?;
    let immediate = half_word & 0x00ff;
    Ok(Instruction {
        condition: Condition::Always,
        op: InstructionOp::ShifterOperandInstruction {
            op,
            update_flags: true,
            base: rd,
            destination: rd,
            value: ShifterOperand::Immediate(immediate as _, None),
        },
    })
}

fn thumb_op_code_000(half_word: u16) -> Result<Instruction, InstructionDecodeError> {
    let immediate = (half_word & 0x07C0) >> 6;
    let rn = (half_word & 0x0038) >> 3;
    let rn = RegisterIndex::try_from(rn as u32)
        .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rn as _))?;
    let rd = half_word & 0x0007;
    let rd = RegisterIndex::try_from(rd as u32)
        .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rd as _))?;
    if (half_word & 0x1000) == 0 {
        // LSL or LSR
        let is_right_shift = (half_word & 0x0800) > 0;
        let shift_op = if is_right_shift {
            ShiftOperator::RightShift
        } else {
            ShiftOperator::LeftShift
        };
        Ok(Instruction {
            condition: Condition::Always,
            op: InstructionOp::ShifterOperandInstruction {
                op: ShifterOperandInstructionOp::Move,
                update_flags: true,
                base: RegisterIndex::R0,
                destination: rd,
                value: ShifterOperand::RegisterImmediate(rn, shift_op, immediate as u8),
            },
        })
    } else if (half_word & 0x1C00) == 0x1C00 {
        // Add or sub
        let immediate = (half_word & 0x01C0) >> 6;
        let is_add = (half_word & 0x0200) == 0;
        let op = if is_add {
            ShifterOperandInstructionOp::Add
        } else {
            ShifterOperandInstructionOp::Subtract
        };
        Ok(Instruction {
            condition: Condition::Always,
            op: InstructionOp::ShifterOperandInstruction {
                op,
                update_flags: true,
                base: rn,
                destination: rd,
                value: ShifterOperand::Immediate(immediate as _, None),
            },
        })
    } else if (half_word & 0x1C00) == 0x1800 {
        let rm = ((half_word & 0x01C0) >> 6) as u32;
        let rm = RegisterIndex::try_from(rm)
            .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rm))?;
        let is_add = (half_word & 0x0200) == 0;
        let op = if is_add {
            ShifterOperandInstructionOp::Add
        } else {
            ShifterOperandInstructionOp::Subtract
        };
        // Add or sub
        Ok(Instruction {
            condition: Condition::Always,
            op: InstructionOp::ShifterOperandInstruction {
                op,
                update_flags: true,
                base: rn,
                destination: rd,
                value: ShifterOperand::Register(rm),
            },
        })
    } else if (half_word & 0x1800) == 0x1000 {
        let rm = ((half_word & 0x01C0) >> 6) as u32;
        let rm = RegisterIndex::try_from(rm)
            .map_err(|()| InstructionDecodeError::InvalidRegisterIndex(rm))?;
        // Add or sub
        Ok(Instruction {
            condition: Condition::Always,
            op: InstructionOp::ShifterOperandInstruction {
                op: ShifterOperandInstructionOp::Move,
                update_flags: true,
                base: rn,
                destination: rd,
                value: ShifterOperand::RegisterImmediate(
                    rm,
                    ShiftOperator::ArithmeticRightShift,
                    immediate as _,
                ),
            },
        })
    } else {
        Err(InstructionDecodeError::UnknownThumb(half_word))
    }
}
