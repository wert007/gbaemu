use std::{
    collections::{BTreeMap, HashMap},
    fmt::Display,
};

use gbaemu_core::{
    instructions::{Condition, Instruction},
    plugins::{DebugSymbols, Parameter},
    registers::RegisterIndex,
};

#[derive(Debug)]
struct Instr {
    ip: usize,
    instr: Instruction,
    function_map: HashMap<u32, String>,
}

impl Display for Instr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let txt = self
            .instr
            .to_string()
            .replace("[m", "")
            .replace("[38;5;8m", "");
        match self.instr.op {
            gbaemu_core::instructions::InstructionOp::Branch {
                store_return_address_in_link_register,
                target,
                ..
            } => match target {
                gbaemu_core::instructions::BranchTarget::Offset(it) => {
                    let target = (self.ip as u32 + 4).wrapping_add_signed(it);
                    match self.function_map.get(&target) {
                        Some(n) => {
                            write!(
                                f,
                                "{}B{} @{n}",
                                self.instr.condition,
                                if store_return_address_in_link_register {
                                    "L"
                                } else {
                                    ""
                                }
                            )
                        }
                        None => {
                            write!(
                                f,
                                "{}B{} {it:#06x}({it}d) >[{target:04x}]",
                                self.instr.condition,
                                if store_return_address_in_link_register {
                                    "L"
                                } else {
                                    ""
                                }
                            )
                        }
                    }
                }
                gbaemu_core::instructions::BranchTarget::RegisterWithOffset(register_index, it) => {
                    if register_index == RegisterIndex::Ip {
                        let target = (self.ip as u32 + 4).wrapping_add_signed(it);
                        match self.function_map.get(&target) {
                            Some(n) => {
                                write!(
                                    f,
                                    "{}B{} @{n}",
                                    self.instr.condition,
                                    if store_return_address_in_link_register {
                                        "L"
                                    } else {
                                        ""
                                    }
                                )
                            }
                            None => {
                                write!(
                                    f,
                                    "{}B{} {it:#06x}({it}d) >[{target:04x}]",
                                    self.instr.condition,
                                    if store_return_address_in_link_register {
                                        "L"
                                    } else {
                                        ""
                                    }
                                )
                            }
                        }
                    } else {
                        txt.fmt(f)
                    }
                }
            },
            _ => txt.fmt(f),
        }
    }
}

#[derive(Debug)]
enum Program {
    Instr(Instr),
    Data(u16),
}
impl Program {
    fn size(&self) -> usize {
        match self {
            Program::Instr(instruction) => instruction.instr.size,
            Program::Data(_) => 2,
        }
    }
}

struct Function<T> {
    start: u32,
    parameters: Vec<Parameter>,
    return_parameters: Vec<Parameter>,
    name: String,
    program: Vec<T>,
    is_mapped: bool,
    end: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BasicBlockSuccessor {
    None,
    One(u32),
    Two(u32, u32),
    OneUnknown,
    TwoUnknown(u32),
}

impl BasicBlockSuccessor {
    pub fn new(entry: &Program) -> Option<Self> {
        match entry {
            Program::Instr(instr) => {
                let next_instr_ip = instr.ip as u32 + instr.instr.size as u32;
                match instr.instr.op {
                    gbaemu_core::instructions::InstructionOp::Branch { target, .. } => match target
                    {
                        gbaemu_core::instructions::BranchTarget::Offset(it) => {
                            let target = (instr.ip as u32 + 4).wrapping_add_signed(it);
                            if instr.instr.condition == Condition::Always {
                                Some(Self::One(target))
                            } else {
                                Some(Self::Two(target, next_instr_ip))
                            }
                        }
                        gbaemu_core::instructions::BranchTarget::RegisterWithOffset(
                            register_index,
                            it,
                        ) => {
                            if register_index == RegisterIndex::Ip {
                                let target = (instr.ip as u32 + 4).wrapping_add_signed(it);
                                if instr.instr.condition == Condition::Always {
                                    Some(Self::One(target))
                                } else {
                                    Some(Self::Two(target, next_instr_ip))
                                }
                            } else {
                                if instr.instr.condition == Condition::Always {
                                    Some(Self::OneUnknown)
                                } else {
                                    Some(Self::TwoUnknown(next_instr_ip))
                                }
                            }
                        }
                    },
                    _ => None,
                }
            }
            Program::Data(_) => None,
        }
    }
}

impl Display for BasicBlockSuccessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BasicBlockSuccessor::None => write!(f, "?"),
            BasicBlockSuccessor::One(i) => write!(f, "_bb_{i:04x}"),
            BasicBlockSuccessor::Two(a, b) => write!(f, "_bb_{a:04x} or _bb_{b:04x}"),
            BasicBlockSuccessor::OneUnknown => write!(f, "_bb_xxxx"),
            BasicBlockSuccessor::TwoUnknown(i) => write!(f, "_bb_xxxx or _bb_{i:04x}"),
        }
    }
}

#[derive(Debug)]
struct BasicBlock {
    start: u32,
    end: u32,
    instructions: Vec<Program>,
    successor: BasicBlockSuccessor,
}
impl BasicBlock {
    fn contains(&self, target: u32) -> bool {
        self.start <= target && target < self.end
    }

    fn split(&mut self, target: u32) -> BasicBlock {
        assert!(self.contains(target));
        // dbg!(&self, target);
        self.end = target;
        self.successor = BasicBlockSuccessor::One(target);
        let mut split_index = 0;
        let mut ip = self.start;
        while ip < target {
            ip += self.instructions[split_index].size() as u32;
            split_index += 1;
        }
        BasicBlock {
            start: target,
            end: self.end,
            instructions: self.instructions.split_off(split_index),
            successor: self.successor,
        }
    }
}

fn main() {
    let program = load_program();

    let functions = load_functions(program);

    let functions = convert_to_basic_blocks(functions);
    for function in functions {
        print_function_header(&function);
        let mut ip = function.start as usize;
        for entry in function.program {
            println!("  _bb_{:04x} -> {}", entry.start, entry.successor);
            for entry in entry.instructions {
                let size = entry.size();
                match entry {
                    Program::Instr(instruction) => {
                        println!("    [{ip:04x}] {instruction}");
                    }
                    Program::Data(data) => {
                        println!("    [{ip:04x}] {data:#04x}");
                    }
                }
                ip += size;
            }
        }
    }
}

fn print_function_header(function: &Function<BasicBlock>) {
    print!("fn {}(", function.name);
    for p in &function.parameters {
        print!(
            "{} [{}]: {}, ",
            p.name,
            match p.location {
                gbaemu_core::plugins::Location::Register(register_index) =>
                    register_index.to_string(),
                gbaemu_core::plugins::Location::Memory(register_index, offset) =>
                    format!("[{register_index} + {offset}]"),
            },
            p.type_.to_string()
        )
    }
    print!(") -> ");
    for r in &function.return_parameters {
        print!(
            "{} [{}]: {}, ",
            r.name,
            match r.location {
                gbaemu_core::plugins::Location::Register(register_index) =>
                    register_index.to_string(),

                gbaemu_core::plugins::Location::Memory(register_index, offset) =>
                    format!("[{register_index} + {offset}]"),
            },
            r.type_.to_string()
        )
    }
    println!(":");
}

fn convert_to_basic_blocks(functions: Vec<Function<Program>>) -> Vec<Function<BasicBlock>> {
    functions
        .into_iter()
        .map(|f| convert_to_basic_block(f))
        .collect()
}

fn convert_to_basic_block(f: Function<Program>) -> Function<BasicBlock> {
    let mut basic_blocks = Vec::new();
    basic_blocks.push(BasicBlock {
        start: f.start,
        end: f.start,
        instructions: Vec::new(),
        successor: BasicBlockSuccessor::None,
    });
    let mut ip = f.start;
    for entry in f.program {
        let size = entry.size() as u32;
        let successor = BasicBlockSuccessor::new(&entry);
        basic_blocks.last_mut().unwrap().instructions.push(entry);
        ip += size;
        if let Some(successor) = successor {
            basic_blocks.last_mut().unwrap().successor = successor;
            basic_blocks.last_mut().unwrap().end = ip;
            basic_blocks.push(BasicBlock {
                start: ip,
                end: ip,
                instructions: Vec::new(),
                successor: BasicBlockSuccessor::None,
            });
        }
    }
    basic_blocks.pop_if(|b| b.instructions.is_empty());
    if basic_blocks.last().unwrap().successor == BasicBlockSuccessor::None {
        basic_blocks.last_mut().unwrap().successor = BasicBlockSuccessor::One(ip);
        basic_blocks.last_mut().unwrap().end = ip;
    }
    split_if_necessary(&mut basic_blocks);
    Function {
        parameters: f.parameters,
        return_parameters: f.return_parameters,
        start: f.start,
        name: f.name,
        program: basic_blocks,
        is_mapped: f.is_mapped,
        end: f.end,
    }
}

fn split_if_necessary(basic_blocks: &mut Vec<BasicBlock>) {
    let mut targets: Vec<u32> = basic_blocks
        .iter()
        .flat_map(|b| match b.successor {
            BasicBlockSuccessor::None => Vec::new(),
            BasicBlockSuccessor::One(a) => vec![a],
            BasicBlockSuccessor::Two(a, b) => vec![a, b],
            BasicBlockSuccessor::OneUnknown => Vec::new(),
            BasicBlockSuccessor::TwoUnknown(a) => vec![a],
        })
        .filter(|s| basic_blocks.iter().any(|b| b.contains(*s)))
        .filter(|s| !basic_blocks.iter().map(|b| b.start).any(|x| x == *s))
        .collect();
    while let Some(target) = targets.pop() {
        let Some(new) = basic_blocks.iter_mut().find(|b| b.contains(target)) else {
            continue;
        };
        let new = new.split(target);
        basic_blocks.push(new);
    }
    basic_blocks.sort_by_key(|b| b.start);
}

fn load_functions(program: BTreeMap<usize, Program>) -> Vec<Function<Program>> {
    let debug_symbols_bios: DebugSymbols =
        serde_json5::from_str(include_str!("../../assets/bios_symbols.json5")).unwrap();

    let mut functions = Vec::new();
    let mut current: Option<Function<Program>> = None;
    for (ip, entry) in program {
        match (
            debug_symbols_bios.find_function_for_address(ip as u32),
            current.take(),
        ) {
            (Some(f), Some(mut cur)) => {
                if f.name() != cur.name {
                    functions.push(cur);
                    current = Some(Function {
                        parameters: f.params.clone(),
                        return_parameters: f.return_params.clone(),
                        start: f.start,
                        name: f.name().into_owned(),
                        is_mapped: true,
                        program: vec![entry],
                        end: f.end,
                    });
                } else {
                    cur.program.push(entry);
                    current = Some(cur);
                }
            }
            (Some(f), None) => {
                current = Some(Function {
                    parameters: f.params.clone(),
                    return_parameters: f.return_params.clone(),
                    start: f.start,
                    name: f.name().into(),
                    is_mapped: true,
                    program: vec![entry],
                    end: f.start,
                });
            }
            (None, Some(mut cur)) => {
                if cur.is_mapped {
                    if ip < cur.end as usize {
                        cur.program.push(entry);
                        current = Some(cur);
                    } else {
                        functions.push(cur);
                        current = Some(Function {
                            parameters: Vec::new(),
                            return_parameters: Vec::new(),
                            start: ip as _,
                            name: format!("unknown_{ip:04x}"),
                            is_mapped: false,
                            end: (ip + entry.size()) as _,
                            program: vec![entry],
                        });
                    }
                } else {
                    cur.program.push(entry);
                    current = Some(cur);
                }
            }
            (None, None) => {
                current = Some(Function {
                    parameters: Vec::new(),
                    return_parameters: Vec::new(),
                    start: ip as _,
                    name: format!("unknown_{ip:04x}"),
                    is_mapped: false,
                    end: (ip + entry.size()) as _,
                    program: vec![entry],
                });
            }
        }
    }
    functions
}

fn load_program() -> BTreeMap<usize, Program> {
    let debug_symbols_bios: DebugSymbols =
        serde_json5::from_str(include_str!("../../assets/bios_symbols.json5")).unwrap();

    let bios = include_bytes!("../../assets/gba_bios.bin");
    let mut ip = 0;
    let mut program = BTreeMap::new();
    let mut is_prev_thumb = false;
    let function_map: HashMap<u32, String> = debug_symbols_bios
        .functions
        .iter()
        .map(|f| (f.start, f.name().to_string()))
        .collect();
    while ip < bios.len() {
        let half_word = read_half_word_at(bios, ip);
        let next_half_word = read_half_word_at(bios, ip + 2);
        let arm = Instruction::decode_arm(read_word_at(bios, ip));
        let thumb = Instruction::decode_thumb(half_word, next_half_word);
        match if arm.is_ok() && thumb.is_ok() {
            if let Some(f) = debug_symbols_bios
                .find_function_for_address(ip as _)
                .take_if(|f| f.start == ip as u32)
            {
                match f.mode {
                    gbaemu_core::plugins::ArmMode::Arm => arm,
                    gbaemu_core::plugins::ArmMode::Thumb
                    | gbaemu_core::plugins::ArmMode::ThumbToArm => thumb,
                }
            } else {
                if is_prev_thumb { thumb } else { arm }
            }
        } else {
            is_prev_thumb = thumb.is_ok();
            arm.or(thumb)
        } {
            Ok(instr) => {
                program.insert(
                    ip,
                    Program::Instr(Instr {
                        ip,
                        instr,
                        function_map: function_map.clone(),
                    }),
                );
                ip += instr.size;
            }
            Err(_) => {
                program.insert(ip, Program::Data(half_word));
                ip += 2;
            }
        }
    }
    program
}

fn read_half_word_at(bios: &[u8], ptr: usize) -> u16 {
    let lower = bios.get(ptr).copied().unwrap_or_default();
    let higher = bios.get(ptr + 1).copied().unwrap_or_default();
    u16::from_le_bytes([lower, higher])
}

fn read_word_at(bios: &[u8], ptr: usize) -> u32 {
    let byte1 = bios.get(ptr).copied().unwrap_or_default();
    let byte2 = bios.get(ptr + 1).copied().unwrap_or_default();
    let byte3 = bios.get(ptr + 2).copied().unwrap_or_default();
    let byte4 = bios.get(ptr + 3).copied().unwrap_or_default();
    u32::from_le_bytes([byte1, byte2, byte3, byte4])
}
