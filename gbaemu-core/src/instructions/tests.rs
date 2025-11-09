use crate::Cartridge;

use super::*;

#[test]
fn decode_set_of_instructions_correct() {
    let input = [
        (
            0xe8bd5000u32,
            Instruction {
                condition: Condition::Always,
                op: InstructionOp::StoreOrLoadRegisters {
                    is_load: true,
                    register_base: RegisterIndex::Sp,
                    register_base_write_back: true,
                    register_list: RegisterList::from_registers([
                        RegisterIndex::Lr,
                        RegisterIndex::R12,
                    ]),
                    addressing_mode: StoreLoadManyAddressingMode::IncrementAfter,
                },
            },
        ),
        (
            0xE008099Bu32,
            Instruction {
                condition: Condition::Always,
                op: InstructionOp::ShifterOperandInstruction {
                    op: ShifterOperandInstructionOp::Multiplicate,
                    update_flags: false,
                    base: RegisterIndex::R11,
                    destination: RegisterIndex::R8,
                    value: ShifterOperand::Register(RegisterIndex::R9),
                },
            },
        ),
    ];
    for (encoded, decoded) in input {
        assert_eq!(Instruction::decode_arm(encoded), Ok(decoded))
    }
}

#[test]
fn test_branch_instructions_arm() {
    const DUMMY_CARTRIGDE: Cartridge = Cartridge { raw: Vec::new() };
    let input: &[(u32, Instruction, &[(RegisterIndex, u32)])] = &[
        (
            0xea000018u32,
            Instruction {
                condition: Condition::Always,
                op: InstructionOp::Branch {
                    store_return_address_in_link_register: false,
                    return_address_is_thumb: false,
                    does_switch_mode: false,
                    target: BranchTarget::Offset(0x60),
                    instruction_size: 4,
                },
            },
            &[(RegisterIndex::Ip, 0x70)],
        ),
        (
            0xeb00000f,
            Instruction {
                condition: Condition::Always,
                op: InstructionOp::Branch {
                    store_return_address_in_link_register: true,
                    return_address_is_thumb: false,
                    does_switch_mode: false,
                    target: BranchTarget::Offset(0x3c),
                    instruction_size: 4,
                },
            },
            &[(RegisterIndex::Ip, 0x4c), (RegisterIndex::Lr, 0x4)],
        ),
        (
            0xe12fff10,
            Instruction {
                condition: Condition::Always,
                op: InstructionOp::Branch {
                    store_return_address_in_link_register: false,
                    return_address_is_thumb: false,
                    does_switch_mode: true,
                    target: BranchTarget::RegisterWithOffset(RegisterIndex::R0, 0),
                    instruction_size: 4,
                },
            },
            &[(RegisterIndex::Ip, 8)],
        ),
    ];
    for &(encoded, decoded, registers) in input.into_iter() {
        assert_eq!(Instruction::decode_arm(encoded), Ok(decoded));
        let mut state = Gba::new(DUMMY_CARTRIGDE);
        decoded.execute(&mut state);
        for &(register, value) in registers {
            let actual = state.registers.read(register);
            assert_eq!(
                actual, value,
                "Expected {register} = 0x{value:x}, Actual {register} = 0x{actual:x}"
            );
        }
    }
}

#[test]
fn test_branch_instructions_thumb() {
    const DUMMY_CARTRIGDE: Cartridge = Cartridge { raw: Vec::new() };
    let input: &[((u16, u16), Instruction, &[(RegisterIndex, u32)])] = &[(
        (0xe083, 0x2d71),
        Instruction {
            condition: Condition::Always,
            op: InstructionOp::Branch {
                store_return_address_in_link_register: false,
                return_address_is_thumb: true,
                does_switch_mode: false,
                target: BranchTarget::Offset(0x106),
                instruction_size: 4,
            },
        },
        &[(RegisterIndex::Ip, 0x116)],
    )];
    for &(encoded, decoded, registers) in input.into_iter() {
        assert_eq!(Instruction::decode_thumb(encoded.0, encoded.1), Ok(decoded));
        let mut state = Gba::new(DUMMY_CARTRIGDE);
        decoded.execute(&mut state);
        for &(register, value) in registers {
            let actual = state.registers.read(register);
            assert_eq!(
                actual, value,
                "Expected {register} = 0x{value:x}, Actual {register} = 0x{actual:x}"
            );
        }
    }
}

#[test]
fn test_instruction_pointer_register() {
    // ADD R0, IP, #1
    const DUMMY_CARTRIGDE: Cartridge = Cartridge { raw: Vec::new() };
    let input = [(
        0xe28f0001,
        Instruction {
            condition: Condition::Always,
            op: InstructionOp::ShifterOperandInstruction {
                op: ShifterOperandInstructionOp::Add,
                update_flags: false,
                base: RegisterIndex::Ip,
                destination: RegisterIndex::R0,
                value: ShifterOperand::Immediate(1, None),
            },
        },
        &[(RegisterIndex::R0, 0x11c | 1)],
    )];
    for (encoded, decoded, registers) in input {
        assert_eq!(Instruction::decode_arm(encoded), Ok(decoded));
        let mut state = Gba::new(DUMMY_CARTRIGDE);
        state.registers.write(RegisterIndex::Ip, 0x114);
        decoded.execute(&mut state);
        for (register, value) in *registers {
            let actual = state.registers.read(register);
            assert_eq!(
                actual, value,
                "Expected {register} = 0x{value:x}, Actual {register} = 0x{actual:x}"
            );
        }
    }
}

#[test]
fn test_store_load_many_addressing_modes() {
    // let full_register_list: RegisterList = RegisterList::try_from(0xffff).unwrap();
    // assert_eq!(
    //     StoreLoadManyAddressingMode::DecrementAfter.start_address_offset(full_register_list),
    //     0
    // );
    // assert_eq!(
    //     StoreLoadManyAddressingMode::DecrementAfter.end_address_offset(full_register_list),
    //     16 * 4
    // )
}

#[test]
fn test_instruction_decoding() -> Result<(), InstructionDecodeError> {
    let thumbs = [
        (
            0xc61u16,
            Instruction {
                condition: Condition::Always,
                op: InstructionOp::ShifterOperandInstruction {
                    op: ShifterOperandInstructionOp::Move,
                    update_flags: true,
                    base: RegisterIndex::R0,
                    destination: RegisterIndex::R1,
                    value: ShifterOperand::RegisterImmediate(
                        RegisterIndex::R4,
                        ShiftOperator::RightShift,
                        0x11,
                    ),
                },
            },
        ),
        (
            0x1909,
            Instruction {
                condition: Condition::Always,
                op: InstructionOp::ShifterOperandInstruction {
                    op: ShifterOperandInstructionOp::Add,
                    update_flags: true,
                    base: RegisterIndex::R1,
                    destination: RegisterIndex::R1,
                    value: ShifterOperand::Register(RegisterIndex::R4),
                },
            },
        ),
        (
            0x43d0,
            Instruction {
                condition: Condition::Always,
                op: InstructionOp::ShifterOperandInstruction {
                    op: ShifterOperandInstructionOp::MoveNegate,
                    update_flags: true,
                    base: RegisterIndex::R0,
                    destination: RegisterIndex::R0,
                    value: ShifterOperand::Register(RegisterIndex::R2),
                },
            },
        ),
        (
            0x1E52,
            Instruction {
                condition: Condition::Always,
                op: InstructionOp::ShifterOperandInstruction {
                    op: ShifterOperandInstructionOp::Subtract,
                    update_flags: true,
                    base: RegisterIndex::R2,
                    destination: RegisterIndex::R2,
                    value: ShifterOperand::Immediate(0x1, None),
                },
            },
        ),
        (
            0x4561,
            Instruction {
                condition: Condition::Always,
                op: InstructionOp::ShifterOperandInstruction {
                    op: ShifterOperandInstructionOp::Compare,
                    update_flags: true,
                    base: RegisterIndex::R1,
                    destination: RegisterIndex::R0,
                    value: ShifterOperand::Register(RegisterIndex::R12),
                },
            },
        ),
        (
            0x4778,
            Instruction {
                condition: Condition::Always,
                op: InstructionOp::Branch {
                    store_return_address_in_link_register: false,
                    return_address_is_thumb: true,
                    does_switch_mode: true,
                    target: BranchTarget::RegisterWithOffset(RegisterIndex::Ip, 0),
                    instruction_size: 2,
                },
            },
        ),
        (
            0x585a,
            Instruction {
                condition: Condition::Always,
                op: InstructionOp::StoreOrLoadRegister {
                    is_load: true,
                    address: StoreLoadMemoryAddress {
                        ignore_bit_1_of_pc: false,
                        register_base: RegisterIndex::R3,
                        write_back: false,
                        negate_offset: false,
                        offset: ShifterOperand::Register(RegisterIndex::R1),
                        is_post_indexing: false,
                    },
                    register_destination: RegisterIndex::R2,
                    read_size: StoreLoadMemorySize::Word,
                },
            },
        ),
    ];
    for (encoded, decoded) in thumbs {
        let result = Instruction::decode_thumb(encoded, 0)?;
        assert_eq!(result, decoded);
    }
    let arms = [
        (
            0xb8a103fcu32,
            Instruction {
                condition: Condition::LessThan,
                op: InstructionOp::StoreOrLoadRegisters {
                    is_load: false,
                    register_base: RegisterIndex::R1,
                    register_base_write_back: true,
                    register_list: RegisterList::from_registers([
                        RegisterIndex::R2,
                        RegisterIndex::R3,
                        RegisterIndex::R4,
                        RegisterIndex::R5,
                        RegisterIndex::R6,
                        RegisterIndex::R7,
                        RegisterIndex::R8,
                        RegisterIndex::R9,
                    ]),
                    addressing_mode: StoreLoadManyAddressingMode::IncrementAfter,
                },
            },
        ),
        (
            0xe751500c,
            Instruction {
                condition: Condition::Always,
                op: InstructionOp::StoreOrLoadRegister {
                    is_load: true,
                    address: StoreLoadMemoryAddress {
                        ignore_bit_1_of_pc: false,
                        register_base: RegisterIndex::R1,
                        write_back: false,
                        negate_offset: true,
                        offset: ShifterOperand::RegisterImmediate(
                            RegisterIndex::R12,
                            ShiftOperator::LeftShift,
                            0,
                        ),
                        is_post_indexing: false,
                    },
                    register_destination: RegisterIndex::R5,
                    read_size: StoreLoadMemorySize::Byte,
                },
            },
        ),
        (
            0xe28f0f96,
            Instruction {
                condition: Condition::Always,
                op: InstructionOp::ShifterOperandInstruction {
                    op: ShifterOperandInstructionOp::Add,
                    update_flags: false,
                    base: RegisterIndex::Ip,
                    destination: RegisterIndex::R0,
                    value: ShifterOperand::Immediate(0x258, Some(false)),
                },
            },
        ),
        (
            0xe59f01cc,
            Instruction {
                condition: Condition::Always,
                op: InstructionOp::StoreOrLoadRegister {
                    is_load: true,
                    address: StoreLoadMemoryAddress {
                        ignore_bit_1_of_pc: false,
                        register_base: RegisterIndex::Ip,
                        write_back: false,
                        negate_offset: false,
                        offset: ShifterOperand::Immediate(0x1cc, None),
                        is_post_indexing: false,
                    },
                    register_destination: RegisterIndex::R0,
                    read_size: StoreLoadMemorySize::Word,
                },
            },
        ),
        (
            0xe1d270b0,
            Instruction {
                condition: Condition::Always,
                op: InstructionOp::StoreOrLoadRegister {
                    is_load: true,
                    address: StoreLoadMemoryAddress {
                        ignore_bit_1_of_pc: false,
                        register_base: RegisterIndex::R2,
                        write_back: false,
                        negate_offset: false,
                        offset: ShifterOperand::Immediate(0x0, None),
                        is_post_indexing: false,
                    },
                    register_destination: RegisterIndex::R7,
                    read_size: StoreLoadMemorySize::Halfword,
                },
            },
        ),
        (
            0xe19c10f1,
            Instruction {
                condition: Condition::Always,
                op: InstructionOp::StoreOrLoadRegister {
                    is_load: true,
                    address: StoreLoadMemoryAddress {
                        ignore_bit_1_of_pc: false,
                        register_base: RegisterIndex::R12,
                        write_back: false,
                        negate_offset: false,
                        offset: ShifterOperand::Register(RegisterIndex::R1),
                        is_post_indexing: false,
                    },
                    register_destination: RegisterIndex::R1,
                    read_size: StoreLoadMemorySize::SignedHalfword,
                },
            },
        ),
    ];

    for (encoded, decoded) in arms {
        let result = Instruction::decode_arm(encoded)?;
        assert_eq!(result, decoded);
    }
    Ok(())
}

#[test]
fn test_flags() {
    const DUMMY_CARTRIGDE: Cartridge = Cartridge { raw: Vec::new() };
    let instructions = [
        (
            0xe35e0000u32,
            InstructionFlags {
                zero_flag: true,
                carry_flag: true,
                ..Default::default()
            },
        ),
        (0xe33c0001, InstructionFlags::default()),
    ];

    for (instruction, expected) in instructions {
        let mut state = Gba::new(DUMMY_CARTRIGDE);
        let instruction = Instruction::decode_arm(instruction).unwrap();
        instruction.execute(&mut state);
        assert_eq!(state.registers.flags(), expected,);
    }
}

#[test]
fn test_msr() {
    const DUMMY_CARTRIGDE: Cartridge = Cartridge { raw: Vec::new() };
    let instructions = [(0xe129f000u32, 0x5fu32)];
    for (instruction, cpsr) in instructions {
        let mut state = Gba::new(DUMMY_CARTRIGDE);
        let instruction = Instruction::decode_arm(instruction).unwrap();
        state.registers.write(RegisterIndex::R0, 0x5f);
        instruction.execute(&mut state);
        let actual = state.registers.read(RegisterIndex::Cpsr);
        assert_eq!(actual, cpsr, "actual: {actual:x}, expected: {cpsr:x}");
    }
}
