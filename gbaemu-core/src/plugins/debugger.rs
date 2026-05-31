mod commands;
mod timetravel;

use std::path::PathBuf;

use crate::{
    GbaArgs,
    instructions::{Instruction, display::DisplayContext},
    interrupts::Interrupt,
    memory::Memory,
    plugins::{
        Plugin, PluginWishes,
        debug_symbols::DebugSymbols,
        debugger::{commands::Command, timetravel::History},
    },
    registers::{Mode, RegisterIndex, RegisterList, Registers},
};

pub struct Breakpoint {
    pub ip: u32,
    condition: Option<Box<dyn Fn(Registers) -> bool + Send>>,
    action: Option<Box<dyn Fn(Registers) + Send>>,
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

    fn action(&self, registers: Registers) -> bool {
        if let Some(a) = &self.action {
            a(registers);
            false
        } else {
            true
        }
    }
}

pub struct MemoryAddressWatcher {
    pub address: u32,
    pub last_seen_value: u8,
    pub print_on_read: bool,
    pub step_on_read: bool,
    pub step_on_write: bool,
}

pub struct InterruptHandler {
    interrupt: Option<Interrupt>,
    action: Box<dyn Fn(Interrupt) -> PluginWishes + Send + 'static>,
}

pub struct Debugger {
    log_file: Option<PathBuf>,
    breakpoints: Vec<Breakpoint>,
    interrupt_handler: Vec<InterruptHandler>,
    memory_address_watchers: Vec<MemoryAddressWatcher>,
    skipped_sections: Vec<(u32, u32)>,
    is_silent: bool,
    is_stepping: bool,
    watch_stack: bool,
    stackframe: Vec<u32>,
    debug_symbols: Vec<DebugSymbols>,
    registers: Registers,
    history: History,
}

impl Debugger {
    pub fn new(is_silent: bool, history_capacity: usize) -> Self {
        let debug_symbols_bios =
            serde_json5::from_str(include_str!("../../../assets/bios_symbols.json5")).unwrap();
        Self {
            log_file: None,
            history: History::new(history_capacity),
            breakpoints: Vec::new(),
            memory_address_watchers: Vec::new(),
            is_stepping: false,
            is_silent,
            watch_stack: false,
            stackframe: Vec::new(),
            debug_symbols: vec![debug_symbols_bios],
            registers: Registers::new(),
            skipped_sections: Vec::new(),
            interrupt_handler: Vec::new(),
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
            action: None,
            fragile: false,
        });
        self
    }

    pub fn panic_on(&mut self, ip: u32) -> &mut Self {
        self.with_breakpoint_conditionally(ip, |_| panic!())
    }

    pub fn with_breakpoint_conditionally(
        &mut self,
        ip: u32,
        c: impl Fn(Registers) -> bool + 'static + Send,
    ) -> &mut Self {
        self.breakpoints.push(Breakpoint {
            ip,
            condition: Some(Box::new(c)),
            action: None,
            fragile: false,
        });
        self
    }

    pub fn with_watch_memory_address(
        &mut self,
        address: u32,
        length_in_bytes: u32,
        step_on_write: bool,
        step_on_read: bool,
        print_on_read: bool,
    ) -> &mut Self {
        for i in 0..length_in_bytes {
            self.memory_address_watchers.push(MemoryAddressWatcher {
                address: address + i,
                last_seen_value: 0,
                step_on_read,
                step_on_write,
                print_on_read,
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

    pub fn emit_register_on(&mut self, register: RegisterIndex, address: u32) -> &mut Self {
        self.breakpoints.push(Breakpoint {
            ip: address,
            condition: None,
            action: Some(Box::new(move |r| {
                println!("[{address:04x}] {register} = {}", r.read(register));
            })),
            fragile: false,
        });
        self
    }

    pub fn skip_function(&mut self, name: &str) -> &mut Self {
        if let Some(f) = self.debug_symbols.iter().find_map(|s| {
            s.functions
                .iter()
                .find(|f| f.name.as_ref().map(String::as_str) == Some(name))
        }) {
            self.skip_section(f.start, f.end);
        } else {
            panic!("Function {name} not found???");
        }
        self
    }

    pub fn skip_section(&mut self, start: u32, end: u32) -> &mut Self {
        self.skipped_sections.push((start, end));
        self
    }

    fn is_skipped(&self, ip: u32) -> bool {
        self.skipped_sections
            .iter()
            .copied()
            .any(|(s, e)| s <= ip && ip < e)
    }

    fn is_silent(&self, ip: u32) -> bool {
        self.is_silent || self.is_skipped(ip)
    }

    pub fn break_on_irq(&mut self, irq: crate::interrupts::Interrupt) -> &mut Self {
        self.interrupt_handler.push(InterruptHandler {
            interrupt: Some(irq),
            action: Box::new(|_| PluginWishes {
                pause_execution: true,
                ..Default::default()
            }),
        });
        self
    }

    pub fn emit_interrupt_names(&mut self) -> &mut Self {
        self.interrupt_handler.push(InterruptHandler {
            interrupt: None,
            action: Box::new(|i| {
                println!("Interrupt {i:?} occured");
                Default::default()
            }),
        });
        self
    }
}

impl Plugin for Debugger {
    fn with_args(&mut self, args: GbaArgs) {
        if let Some(log_file) = args.log_file.clone() {
            self.log_file = Some(log_file);
        }
    }

    fn interrupt_occured(&mut self, interrupt: Interrupt) -> Option<PluginWishes> {
        let p = self
            .interrupt_handler
            .iter()
            .map(|i| {
                if i.interrupt.is_none_or(|i| i == interrupt) {
                    (i.action)(interrupt)
                } else {
                    Default::default()
                }
            })
            .fold(PluginWishes::default(), |a, c| a.combined_with(c));
        if p.pause_execution() {
            self.is_stepping = true;
        }
        Some(p)
    }

    fn after_executing(
        &mut self,
        registers: &Registers,
        _mode: Mode,
        instruction: &Instruction,
        ip: u32,
        memory: &mut Memory,
    ) {
        self.history.finish_entry(*registers, memory);
        self.watch_stack_changes_afterwards(registers, instruction, ip);

        let mut messages: Vec<String> = Vec::new();
        let should_print_instruction = !self.is_stepping && !self.is_skipped(ip);
        while let Some((addr, val)) = memory.memory_watcher.reads.pop_front() {
            match self
                .memory_address_watchers
                .iter()
                .find(|m| m.address == addr)
            {
                Some(it) => {
                    if it.print_on_read {
                        messages.push(format!("Read {val:#04x} at {addr:#x}"));
                    }
                    self.is_stepping |= it.step_on_read;
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
                    messages.push(format!(
                        "Memory change at {addr:#x}: {old:#04x} -> {new:#04x}"
                    ));
                    it.last_seen_value = new;
                    self.is_stepping |= it.step_on_write;
                }
                None => {}
            }
        }
        for message in messages {
            self.println(message);
        }
        if self.is_stepping && should_print_instruction {
            let name = if let Some(name) = self.find_function_name_for_address(ip) {
                format!(" // in {name}")
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
                    display_memory_offsets: false,
                })
            ));
            self.println(format!(
                "{ip:04x}: {}{name}",
                instruction.display(DisplayContext {
                    highlighted_registers: RegisterList::default(),
                    is_tty: true,
                    register_values: *registers,
                    use_register_values: true,
                    display_memory_offsets: true,
                })
            ));
        }
        if !self.is_silent(ip) || self.is_stepping {
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
        self.history
            .start_entry(ip, *registers, *instruction, memory);
        let should_continue = !self.is_stepping
            && !self
                .breakpoints
                .iter()
                .any(|b| b.ip == ip && b.condition_met(*registers) && b.action(*registers));
        self.is_stepping = !should_continue;
        if !self.is_silent(ip) || self.is_stepping {
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
                    display_memory_offsets: false,
                })
            ));
            self.println(format!(
                "{ip:04x}: {}{name}",
                instruction.display(DisplayContext {
                    highlighted_registers: RegisterList::default(),
                    is_tty: true,
                    register_values: *registers,
                    use_register_values: true,
                    display_memory_offsets: true,
                })
            ));
        }
        if should_continue {
            return PluginWishes::default();
        }
        self.breakpoints.retain(|b| b.ip != ip || !b.fragile);
        let mut line = String::new();
        println!("Press [c] to continue");
        std::io::stdin().read_line(&mut line).unwrap();
        let cmd = Command::parse(line.to_lowercase().trim());
        match cmd {
            Ok(cmd) => cmd.execute(self, *registers, ip, memory),
            Err(()) => {
                println!("Failed parsing command..");
                PluginWishes::default()
            }
        }
    }
}
