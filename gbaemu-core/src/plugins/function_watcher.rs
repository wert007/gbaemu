use crate::plugins::{DebugSymbols, Plugin};

#[derive(Debug, Default)]
pub struct FunctionWatcher {
    watch_functions: bool,
    last_function_name: Option<String>,
    debug_symbols: Vec<DebugSymbols>,
}

impl FunctionWatcher {
    pub fn find_function_name_for_address(&self, address: u32) -> Option<&str> {
        self.debug_symbols
            .iter()
            .flat_map(|d| d.functions.iter())
            .filter(|f| f.start <= address && address < f.end)
            .find_map(|f| f.name.as_ref().map(|s| s.as_str()))
    }
}

impl Plugin for FunctionWatcher {
    fn with_args(&mut self, args: crate::GbaArgs) {
        self.watch_functions = args.trace_functions;
        self.debug_symbols = vec![
            serde_json5::from_str(include_str!("../../../assets/bios_symbols.json5")).unwrap(),
        ];
    }

    fn after_executing(
        &mut self,
        _registers: &crate::registers::Registers,
        _mode: crate::registers::Mode,
        _instruction: &crate::instructions::Instruction,
        ip: u32,
        _memory: &mut crate::memory::Memory,
    ) {
        if !self.watch_functions {
            return;
        }
        let current: Option<String> = self.find_function_name_for_address(ip).map(Into::into);
        if current != self.last_function_name {
            self.last_function_name = current;
            if let Some(name) = &self.last_function_name {
                println!("[{ip:04x}] {name}");
            }
        }
    }
}
