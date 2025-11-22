use unarm::{DisplayOptions, RegNames};

fn main() {
    let bios = include_bytes!("../../assets/gba_bios.bin");
    for (addr, _inst, p) in unarm::Parser::new(
        unarm::ParseMode::Arm,
        0,
        unarm::Endian::Little,
        unarm::ParseFlags {
            ual: true,
            version: unarm::ArmVersion::V5Te,
        },
        bios,
    ) {
        println!(
            "{addr:04x}:\t{} (0x{:x})",
            p.display(DisplayOptions {
                reg_names: RegNames {
                    av_registers: false,
                    r9_use: unarm::R9Use::GeneralPurpose,
                    explicit_stack_limit: false,
                    frame_pointer: false,
                    ip: false
                }
            }),
            _inst.code(),
        );
    }
}
