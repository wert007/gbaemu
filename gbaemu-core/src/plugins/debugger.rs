use std::path::PathBuf;

use crate::{
    GbaArgs,
    instructions::{Instruction, display::DisplayContext},
    memory::Memory,
    plugins::{Plugin, PluginWishes, debug_symbols::DebugSymbols},
    registers::{Mode, RegisterIndex, RegisterList, Registers},
};

pub struct Breakpoint {
    pub ip: u32,
    condition: Option<Box<dyn Fn(Registers) -> bool + Send>>,
    /// Breakpoint will cease to exist, once it been hit.
    fragile: bool,
}
impl Breakpoint {
    fn condition_met(&self, registers: Registers) -> bool {
        match &self.condition {
            Some(condition) => condition(registers),
            None => true,
        }
    }
}

pub struct MemoryAddressWatcher {
    pub address: u32,
    pub last_seen_value: u8,
}

pub struct Debugger {
    log_file: Option<PathBuf>,
    breakpoints: Vec<Breakpoint>,
    memory_address_watchers: Vec<MemoryAddressWatcher>,
    is_silent: bool,
    is_stepping: bool,
    watch_stack: bool,
    stackframe: Vec<u32>,
    debug_symbols: Vec<DebugSymbols>,
    registers: Registers,
}

impl Debugger {
    pub fn new(is_silent: bool) -> Self {
        let debug_symbols_bios =
            serde_json5::from_str(include_str!("../../../assets/bios_symbols.json5")).unwrap();
        Self {
            log_file: None,
            breakpoints: Vec::new(),
            memory_address_watchers: Vec::new(),
            is_stepping: false,
            is_silent,
            watch_stack: false,
            stackframe: Vec::new(),
            debug_symbols: vec![debug_symbols_bios],
            registers: Registers::new(),
        }
    }

    pub fn find_function_name_for_address(&self, address: u32) -> Option<&str> {
        self.debug_symbols
            .iter()
            .flat_map(|d| d.functions.iter())
            .filter(|f| f.start <= address && address < f.end)
            .find_map(|f| f.name.as_ref().map(|s| s.as_str()))
    }

    pub fn with_watch_stack(&mut self) -> &mut Self {
        self.watch_stack = true;
        self
    }

    pub fn with_breakpoint(&mut self, ip: u32) -> &mut Self {
        self.breakpoints.push(Breakpoint {
            ip,
            condition: None,
            fragile: false,
        });
        self
    }

    pub fn with_breakpoint_conditionally(
        &mut self,
        ip: u32,
        c: impl Fn(Registers) -> bool + 'static + Send,
    ) -> &mut Self {
        self.breakpoints.push(Breakpoint {
            ip,
            condition: Some(Box::new(c)),
            fragile: false,
        });
        self
    }

    pub fn with_watch_memory_address(&mut self, address: u32, length_in_bytes: u32) -> &mut Self {
        for i in 0..length_in_bytes {
            self.memory_address_watchers.push(MemoryAddressWatcher {
                address: address + i,
                last_seen_value: 0,
            });
        }
        self
    }

    fn print_stack_frame(&mut self, ip: u32) {
        self.println("Stackframe:");
        for ip in self.stackframe.iter().copied().chain(std::iter::once(ip)) {
            self.print(format!(" at {ip:#06x}"));
            if let Some(name) = self.find_function_name_for_address(ip) {
                self.print(": ");
                self.print(name);
            }
            self.println("");
        }
        // self.is_stepping = true;
    }

    fn watch_stack_changes_afterwards(
        &mut self,
        registers: &Registers,
        instruction: &Instruction,
        ip: u32,
    ) {
        if !self.watch_stack {
            return;
        }
        let cur_ip = registers.read_raw(RegisterIndex::Ip);
        self.stackframe.pop_if(|s| *s == ip);
        match &instruction.op {
            crate::instructions::InstructionOp::Branch {
                store_return_address_in_link_register,
                instruction_size,
                ..
            } => {
                if *store_return_address_in_link_register {
                    self.stackframe.push(ip + *instruction_size as u32);
                }
            }
            crate::instructions::InstructionOp::StoreOrLoadRegisters {
                is_load,
                register_base,
                register_base_write_back,
                register_list,
                addressing_mode,
            } => {
                if register_list.contains(RegisterIndex::Ip) {
                    if *is_load {
                    } else {
                        if *register_base == RegisterIndex::Sp {
                            println!("PUSH stm");
                            self.stackframe.push(cur_ip);
                            todo!();
                        } else {
                            dbg!(register_base, register_base_write_back, addressing_mode);
                            todo!()
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn println(&self, msg: impl AsRef<str>) {
        self.print(msg);
        self.print("\n");
    }

    fn print(&self, msg: impl AsRef<str>) {
        use std::io::Write;
        let msg = msg.as_ref();
        print!("{msg}");
        if let Some(path) = &self.log_file {
            let mut file = std::fs::File::options()
                .append(true)
                .create(true)
                .open(path)
                .expect("Log file could not be opened!");
            write!(file, "{msg}").expect("Coult not write to logfile!");
        }
    }
}

impl Plugin for Debugger {
    fn with_args(&mut self, args: GbaArgs) {
        if let Some(log_file) = args.log_file.clone() {
            self.log_file = Some(log_file);
        }
    }

    fn after_executing(
        &mut self,
        registers: &Registers,
        _mode: Mode,
        instruction: &Instruction,
        ip: u32,
        memory: &mut Memory,
    ) {
        self.watch_stack_changes_afterwards(registers, instruction, ip);

        let mut messages: Vec<String> = Vec::new();
        let should_print_instruction = !self.is_stepping;
        while let Some((addr, val)) = memory.memory_watcher.reads.pop_front() {
            match self
                .memory_address_watchers
                .iter()
                .find(|m| m.address == addr)
            {
                Some(it) => {
                    messages.push(format!("Read {val:#04x} at {addr:#x}"));
                    self.is_stepping = true;
                }
                None => {}
            }
        }
        let mut memory_changes = Vec::new();
        while let Some((addr, old, new)) = memory.memory_watcher.writes.pop_front() {
            memory_changes.push((addr, old, new));
            match self
                .memory_address_watchers
                .iter_mut()
                .find(|m| m.address == addr)
            {
                Some(it) => {
                    // messages.push(format!(
                    //     "Memory change at {addr:#x}: {old:#04x} -> {new:#04x}"
                    // ));
                    it.last_seen_value = new;
                    self.is_stepping = true;
                }
                None => {}
            }
        }
        for message in messages {
            self.println(message);
        }
        if self.is_stepping && should_print_instruction {
            let name = if let Some(name) = self.find_function_name_for_address(ip) {
                format!(" // {name}")
            } else {
                String::new()
            };
            self.println(format!(
                "{ip:04x}: {}{name}",
                instruction.display(DisplayContext {
                    highlighted_registers: RegisterList::default(),
                    is_tty: true,
                    register_values: *registers,
                    use_register_values: false,
                })
            ));
            self.println(format!(
                "{ip:04x}: {}{name}",
                instruction.display(DisplayContext {
                    highlighted_registers: RegisterList::default(),
                    is_tty: true,
                    register_values: *registers,
                    use_register_values: true,
                })
            ));
        }
        if !self.is_silent || self.is_stepping {
            for register in self.registers.diff(*registers) {
                if register == RegisterIndex::Cpsr {
                    self.println(format!(
                        "  {register}: {}{:?} ({:#012x}) -> {}{:?} ({:#012x})",
                        self.registers.flags(),
                        self.registers.cpsr().mode(),
                        self.registers.read(register),
                        registers.flags(),
                        self.registers.cpsr().mode(),
                        registers.read(register)
                    ));
                } else {
                    self.println(format!(
                        "  {register}: {:#012x} -> {:#012x}",
                        self.registers.read(register),
                        registers.read(register)
                    ));
                }
            }
            for (addr, old, new) in memory_changes {
                if old == new {
                    continue;
                }
                self.println(format!("  [{addr:#x}]: {old:#04x} -> {new:#04x}",));
            }
        }
    }

    fn should_execute(
        &mut self,
        registers: &Registers,
        _mode: Mode,
        instruction: &Instruction,
        ip: u32,
        memory: &Memory,
    ) -> PluginWishes {
        self.registers = *registers;
        let should_continue = !self.is_stepping
            && !self
                .breakpoints
                .iter()
                .any(|b| b.ip == ip && b.condition_met(*registers));
        self.is_stepping = !should_continue;
        if !self.is_silent || self.is_stepping {
            let name = if let Some(name) = self.find_function_name_for_address(ip) {
                format!(" // {name}")
            } else {
                String::new()
            };
            self.println(format!(
                "{ip:04x}: {}{name}",
                instruction.display(DisplayContext {
                    highlighted_registers: RegisterList::default(),
                    is_tty: true,
                    register_values: *registers,
                    use_register_values: false,
                })
            ));
            self.println(format!(
                "{ip:04x}: {}{name}",
                instruction.display(DisplayContext {
                    highlighted_registers: RegisterList::default(),
                    is_tty: true,
                    register_values: *registers,
                    use_register_values: true,
                })
            ));
        }
        if should_continue {
            return PluginWishes::default();
        }
        // self.stackframe.pop_if(|s| s == ip);
        self.breakpoints.retain(|b| b.ip != ip || !b.fragile);
        let mut line = String::new();
        println!("Press [c] to continue");
        std::io::stdin().read_line(&mut line).unwrap();
        let mut pause_execution = true;
        match line.to_lowercase().trim() {
            "r" => {
                registers.dump();
            }
            "j" => {
                if let Some(target) = self.stackframe.iter().copied().filter(|t| *t != ip).last() {
                    self.breakpoints.push(Breakpoint {
                        ip: target,
                        condition: None,
                        fragile: true,
                    });
                    self.is_stepping = false;
                    pause_execution = false;
                } else {
                    println!("Stackframe is empty. No function to jump to.")
                }
            }
            "cb" => {
                self.print_stack_frame(ip);
            }
            "c" => {
                self.is_stepping = false;
                pause_execution = false;
            }
            "s" => {
                self.is_silent = !self.is_silent;
            }
            cmd => {
                if let Some((address, size)) = try_parse_memory_address(cmd, registers) {
                    let value = match size {
                        1 => memory.read_byte_at_silent(address) as i8 as u32,
                        2 => memory.read_half_word_at_silent(address) as i16 as u32,
                        4 => memory.read_word_silent(address),
                        _ => unreachable!(),
                    };
                    self.println(format!(" = {value:#x} ({value}|{})", value as i32));
                } else if let Some(register) = try_parse_register(cmd) {
                    let value = registers.read(register);
                    self.println(format!(" = {value:#x} ({value}|{})", value as i32));
                } else {
                    pause_execution = false;
                }
            }
        }
        PluginWishes {
            pause_execution,
            stop_execution: false,
        }
    }
}

fn try_parse_memory_address(cmd: &str, registers: &Registers) -> Option<(u32, usize)> {
    let size = match cmd.chars().next()? {
        'b' => 1,
        'h' => 2,
        _ => 4,
    };
    let cmd = cmd.trim_start_matches(['b', 'h', 'w', ' ']);
    let addr_str = cmd.strip_prefix('[')?.strip_suffix(']')?;
    let addr = u32::from_str_radix(addr_str, 16).ok().or_else(|| {
        let r = try_parse_register(addr_str)?;
        Some(registers.read_raw(r))
    })?;
    Some((addr, size))
}

fn try_parse_register(register_ref: &str) -> Option<RegisterIndex> {
    let register_name = register_ref.strip_prefix("r:")?;
    Some(match register_name.trim().to_lowercase().as_str() {
        "r0" | "0" => RegisterIndex::R0,
        "r1" | "1" => RegisterIndex::R1,
        "r2" | "2" => RegisterIndex::R2,
        "r3" | "3" => RegisterIndex::R3,
        "r4" | "4" => RegisterIndex::R4,
        "r5" | "5" => RegisterIndex::R5,
        "r6" | "6" => RegisterIndex::R6,
        "r7" | "7" => RegisterIndex::R7,
        "r8" | "8" => RegisterIndex::R8,
        "r9" | "9" => RegisterIndex::R9,
        "r10" | "10" => RegisterIndex::R10,
        "r11" | "11" => RegisterIndex::R11,
        "r12" | "12" => RegisterIndex::R12,
        "sp" => RegisterIndex::Sp,
        "lr" => RegisterIndex::Lr,
        "ip" => RegisterIndex::Ip,
        _ => return None,
    })
}
