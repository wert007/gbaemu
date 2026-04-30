use clap::Parser;
use gbaemu_core::{
    interrupts::Interrupt,
    io_registers::GbaIo,
    lcd::PixelFormat,
    plugins::{debugger::Debugger, function_watcher::FunctionWatcher},
    registers::{RegisterIndex, RegisterList, Registers},
    Cartridge, Gba, GbaArgs,
};
use image::Rgba;
use minifb::WindowOptions;
use std::{
    fs::File,
    io::BufReader,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc, Mutex},
    thread,
    time::Duration,
};

fn parse_hex_u32(s: &str) -> Result<u32, String> {
    let Some(s) = s.strip_prefix("0x") else {
        return s
            .parse()
            .map_err(|e| format!("invalid integer, use prefix 0x for hex!: {e}"));
    };
    u32::from_str_radix(s, 16).map_err(|e| format!("invalid hex: {e}"))
}

#[derive(Parser, Debug, Clone)]
struct GbaEmuArgs {
    #[clap(short, long)]
    silent: bool,
    #[clap(short = 'f', long)]
    trace_functions: bool,
    #[clap(long)]
    stack: bool,
    #[clap(short, long, default_value = "")]
    watch: String,
    #[clap(long, default_value = "false")]
    headless: bool,
    #[clap(long, short = 'p', default_value = "false")]
    show_palette: bool,
    #[clap(long, short = 't', default_value = "false")]
    show_tilemaps: bool,
    #[clap(long="log", default_value=None)]
    log_file: Option<PathBuf>,
    #[clap(long, default_value=None, num_args = 2, value_parser = parse_hex_u32)]
    watch_memoryblock: Option<Vec<u32>>,
}

impl Into<GbaArgs> for GbaEmuArgs {
    fn into(self) -> GbaArgs {
        GbaArgs {
            trace_functions: self.trace_functions,
            log_file: self.log_file,
            watch_stack: self.stack,
            watch_registers: RegisterList::from_registers(
                self.watch
                    .split(',')
                    .filter_map(|r| RegisterIndex::try_from(r).ok()),
            ),
        }
    }
}

fn main() {
    let args = GbaEmuArgs::parse();
    dbg!(&args);
    let game = Cartridge::new(BufReader::new(File::open("./assets/2048_jam.gba").unwrap()));
    // let game = Cartridge::new(BufReader::new(File::open("./assets/2048_jam.gba").unwrap()));
    // gba.dump_bios();
    let mut window = if args.headless {
        None
    } else {
        Some(
            minifb::Window::new(
                "GBAemu",
                240,
                160,
                WindowOptions {
                    scale: minifb::Scale::X2,
                    scale_mode: minifb::ScaleMode::AspectRatioStretch,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
    };
    let mut gba = Gba::new(game).with_args(args.clone());
    let fw = FunctionWatcher::default();
    #[allow(unused_mut)]
    let mut debugger = Debugger::new(args.silent);
    _ = debugger
    .skip_function("abs")
    .skip_function("swi_VBlankIntrWait")
    .skip_function("swi_IntrWait")
    .skip_function("change_wanted_flags")
    .skip_function("load_some_color_palettes")
    .skip_function("swi_Div_t")
    .skip_function("swi_CPUFastSet")
    .skip_function("swi_SoundBiasChange")
    .skip_function("safecopy32")
    .skip_function("swi_HuffUnComp")
    .skip_function("swi_LZ77UnCompWRAM")
    .skip_function("swi_BitUnPack")
    .skip_function("CheckDestInWriteableRange")
    .skip_function("swi_CPUSet")
    .skip_function("copy_data_into_gameboy_logo_buffer")
    .skip_function("LoadLogoIntoGlobalLogoBuffer_wert007")
    .skip_function("loadLogoBlock_wert007")
    .skip_function("simple_hash_wert007")
    .skip_function("swi_RegisterRamReset")
    .skip_function("reset_register_wert007")
    .skip_function("swi_HardReset")
    .skip_function("InitSystemStack")
    .skip_function("jumptable")
    .skip_function("some_parabolic_formulas_maybe_to_make_logos_jump_wert007")
    .skip_function("sub_2D68")
    .skip_function("enable_all_interrupts")
    .skip_function("disable_all_interrupts")
    .skip_function("swi_Diff16bitUnFilter")
    .skip_function("loadGameboyLogoBuffer2IntoVRAM")
    .skip_function("fill2dGradient_wert007")
    .skip_function("copy_color_palette")
    .skip_function("copy_oam_data")
    .skip_function("swi_ObjAffineSet")
    .skip_function("swi_Div")
    .skip_function("swi_DivArm")
    .skip_function("swi_BiosChecksum")
    .skip_function("bios_irq_handler")
    .skip_function("irq_complete")
    .skip_function("irq_vector")
    // .emit_register_on(RegisterIndex::R1, 0x330)
    // .emit_register_on(RegisterIndex::R2, 0x378)
    // .emit_register_on(RegisterIndex::R1, 0x378)
    // .emit_register_on(RegisterIndex::R1, 0x34c)
    // .emit_register_on(RegisterIndex::Lr, 0x328)
    // .emit_register_on(RegisterIndex::R2, 0x364)
    // debugger
    // .with_breakpoint(0x1c40 )
    // .with_breakpoint(0x128)
    // .with_breakpoint(0xaac)
    // .with_breakpoint(0x440)
    // .with_breakpoint_conditionally(0xbc8, |r: Registers| r.read(RegisterIndex::R1) == 0x6016c00)
    // .with_breakpoint(0x198e)
    // .with_breakpoint(0x1992)
    // .with_breakpoint(0x1998)
    // .with_breakpoint(0x199e)
    // .with_breakpoint(0x19a8)
    // .with_breakpoint(0x19b2)
    // .with_breakpoint(0x2b6a)
    // .with_breakpoint(0x2c4c)
    // .with_breakpoint(0x2d70)
    // .with_breakpoint(0x330)
    // .with_breakpoint(0x300)
    // .with_breakpoint(0x344)
    // // .with_breakpoint(0xb96)
    // .break_on_irq(Interrupt::SerialCom)
    .with_watch_memory_address(0x04000128, 2, false, false, false)
    .with_watch_memory_address(0x04000134, 2, false, false, false)
    // .with_watch_memory_address(0x3fffFF8, 2)
    // .with_watch_memory_address(0x300001a, 1, false, true)
    // .with_watch_stack()
    // .with_watch_memory_address(0x60024e0, 0x20)
    // .with_watch_memory_address(0x6002440, 0x80000)

    // This means we need to support V-CounterFlag (is there an Interrupt as well?)
    // .with_watch_memory_address(0x06010000 + 8 * 8 * 434, 8 * 8)
    // .with_watch_memory_address(0x06010000 + 2176, 1)
    // .with_watch_memory_address(0x4000134, 2)
    // .with_watch_memory_address(0x400012a, 2)
    // .with_watch_memory_address(0x4000120, 8)
    // .with_watch_memory_address(0x4000128, 1)
    // .with_watch_memory_address(0x400012a, 2)
    // .with_watch_memory_address(0x4000200, 2)
    // .with_watch_memory_address(0x4000202, 2)
    // .with_watch_memory_address(0x4000208, 2)
    // .with_watch_memory_address(0x4000068, 2)
    // .with_watch_memory_address(0x400006c, 2)
    // .with_watch_memory_address(0x4000070, 6)
    // .with_watch_memory_address(0x4000078, 2)
    // .with_watch_memory_address(0x4000080, 6)
    // .with_watch_memory_address(0x4000088, 2)
    // .with_watch_memory_address(0x4000090, 16)
    // .with_watch_memory_address(0x40000a0, 8)
    ;
    gba.with_plugin(debugger).with_plugin(fw);
    let gba = Arc::new(Mutex::new(gba));
    let is_running = Arc::new(AtomicBool::new(true));
    let core = {
        let gba = gba.clone();
        let is_running = is_running.clone();
        thread::spawn(move || {
            while is_running.load(std::sync::atomic::Ordering::Relaxed) {
                for _ in 0..1000 {
                    gba.lock().unwrap().run_cycle();
                }
                std::thread::sleep(Duration::from_nanos(1));
            }
            is_running.store(false, std::sync::atomic::Ordering::Relaxed);
        })
    };
    let palette_window = {
        let gba = gba.clone();
        let is_running = is_running.clone();
        thread::spawn(move || {
            if !args.show_palette {
                return;
            }
            const SCALE: usize = 16;
            let mut window = minifb::Window::new(
                "Palette",
                16 * SCALE,
                33 * SCALE + 32,
                WindowOptions {
                    // scale: minifb::Scale::X16,
                    // topmost: true,
                    ..Default::default()
                },
            )
            .unwrap();

            while window.is_open() && is_running.load(std::sync::atomic::Ordering::Relaxed) {
                let palette: Vec<u32> = gba
                    .lock()
                    .unwrap()
                    .load_palette()
                    .chunks_exact(16)
                    .enumerate()
                    .flat_map(|(i, c)| {
                        c.iter()
                            .copied()
                            .flat_map(|c| [c; SCALE])
                            .cycle()
                            .take(16 * SCALE * SCALE)
                            .chain([0xff8080; SCALE * 16])
                            .chain(
                                (i == 15)
                                    .then(|| [0x8080ffu32; SCALE * 16 * SCALE])
                                    .into_iter()
                                    .flatten(),
                            )
                    })
                    .collect();

                window
                    .update_with_buffer(&palette, 16 * SCALE, 32 * SCALE + 32)
                    .unwrap();
            }
        })
    };
    let tilemap_window = {
        let gba = gba.clone();
        let is_running = is_running.clone();
        thread::spawn(move || {
            if !args.show_tilemaps {
                return;
            }
            let gba = gba.lock().unwrap().clone_gba_io();
            // const SCALE: usize = 16;
            let mut window = minifb::Window::new(
                "Tilemaps",
                32 * 8,
                32 * 8,
                WindowOptions {
                    scale: minifb::Scale::X2,
                    // scale: minifb::Scale::X16,
                    // topmost: true,
                    ..Default::default()
                },
            )
            .unwrap();

            let mut buffer = load_tile_data(&gba, Some(17), PixelFormat::Bpp8);
            while window.is_open() && is_running.load(std::sync::atomic::Ordering::Relaxed) {
                buffer = if window.is_key_down(minifb::Key::F5) {
                    load_tile_data(&gba, Some(17), PixelFormat::Bpp8)
                } else {
                    buffer
                };
                if window.is_key_down(minifb::Key::F5) {
                    make_screenshot(&buffer, 32 * 8, 32 * 8, Some("tileset".into()));
                }
                // buffer[4096 + 16] = 0xff8888;
                window.update_with_buffer(&buffer, 32 * 8, 32 * 8).unwrap();
            }
        })
    };
    let memory_window = {
        let gba = gba.clone();
        let is_running = is_running.clone();
        thread::spawn(move || {
            const WIDTH: usize = 900;
            const HEIGHT: usize = 600;
            let Some([at, length]) = &args.watch_memoryblock.as_deref() else {
                return;
            };
            // const SCALE: usize = 16;
            let mut window = minifb::Window::new(
                "Memory",
                WIDTH,
                HEIGHT,
                WindowOptions {
                    scale: minifb::Scale::X1,
                    // scale: minifb::Scale::X16,
                    // topmost: true,
                    ..Default::default()
                },
            )
            .unwrap();

            let mut buffer = vec![0; WIDTH * HEIGHT];
            while window.is_open() && is_running.load(std::sync::atomic::Ordering::Relaxed) {
                write_memory_into_buffer::<WIDTH, HEIGHT>(&mut buffer, *at, *length, &gba);
                window.update_with_buffer(&buffer, WIDTH, HEIGHT).unwrap();
            }
        })
    };
    let mut buffer = [0; 160 * 240];
    if let Some(window) = &mut window {
        while window.is_open() && is_running.load(std::sync::atomic::Ordering::Relaxed) {
            gba.lock().unwrap().swap_buffers(&mut buffer);
            if window.is_key_down(minifb::Key::S) {
                make_screenshot(&buffer, 240, 160, None);
            }
            if window.is_key_down(minifb::Key::R) {
                gba.lock().unwrap().reset();
            }
            window.update_with_buffer(&buffer, 240, 160).unwrap();
        }
        is_running.store(false, std::sync::atomic::Ordering::Relaxed);
    }
    _ = palette_window.join();
    _ = tilemap_window.join();
    _ = memory_window.join();
    _ = core.join();
    gba.lock().unwrap().swap_buffers(&mut buffer);
    make_screenshot(&buffer, 240, 160, None);
}

fn write_memory_into_buffer<const WIDTH: usize, const HEIGHT: usize>(
    buf: &mut Vec<u32>,
    at: u32,
    length: u32,
    gba: &Arc<Mutex<Gba>>,
) {
    let text = gba
        .lock()
        .unwrap()
        .read_memory(at, length)
        .into_iter()
        // .map(|b| format!("{b:#02x}"))
        .enumerate()
        .fold(String::new(), |acc, (i, cur)| {
            if i == 0 {
                format!("{cur:#04x}")
            } else if i % 8 != 0 {
                format!("{acc} {cur:#04x}")
            } else if i % 32 != 0 {
                format!("{acc}  {cur:#04x}")
            } else {
                format!("{acc}\n{cur:#04x}")
            }
        });
    minifb_fonts::font5x8::new_renderer(WIDTH, HEIGHT, 0xffffffff).draw_text(buf, 2, 2, &text);
}

fn load_tile_data(
    gba: &Arc<Mutex<GbaIo>>,
    use_palette: Option<usize>,
    format: PixelFormat,
) -> Vec<u32> {
    let tiles = gba.lock().unwrap().load_tiles(format);
    const FALLBACK: [u32; 16] = [
        0x000000u32,
        0xff0000u32,
        0x00ff00u32,
        0x0000ffu32,
        0xffff00u32,
        0xff00ffu32,
        0x00ffffu32,
        0xffffffu32,
        0x808080u32,
        0x800000u32,
        0x008000u32,
        0x000080u32,
        0x808000u32,
        0x800080u32,
        0x008080u32,
        0xc0c0c0u32,
    ];
    let palette = use_palette
        .map(|i| {
            let binding = gba.lock().unwrap().load_palette();
            let (palettes, _): (&[[u32; 16]], _) = binding.as_chunks();
            palettes[i]
        })
        .unwrap_or(FALLBACK);
    let width = match format {
        PixelFormat::Bpp4 => 32,
        PixelFormat::Bpp8 => 16,
    };
    let mut buffer = vec![0xffffffu32; 32 * 32 * 8 * 8];
    for (tile_index, tile) in tiles.into_iter().enumerate() {
        let base_x = (tile_index % width) * 8;
        let base_y = (tile_index / width) * 8;
        for (pixel_index, pixel) in tile.into_iter().enumerate() {
            let pixel_x = pixel_index % 8;
            let pixel_y = pixel_index / 8;

            let color = match format {
                PixelFormat::Bpp4 => palette[pixel as usize],
                PixelFormat::Bpp8 => gba.lock().unwrap().load_palette()[pixel as usize],
            };
            let x = base_x + pixel_x;
            let y = base_y + pixel_y;
            let buffer_index = y * 32 * 8 + x;
            if buffer_index >= buffer.len() {
                break;
            }
            // if buffer_index == 4096 + 16 {
            //     dbg!(x, y, tile_index, base_x, base_y, pixel_x, pixel_y);
            //     panic!();
            // }
            buffer[buffer_index] = color;
        }
    }
    buffer
}

static mut COUNTER: usize = 0;

// fn make_screenshot(buffer: &[u32]) {
//     let mut image = image::DynamicImage::new(240, 160, image::ColorType::Rgba8);
//     for (x, y, pixel) in image.as_mut_rgba8().unwrap().enumerate_pixels_mut() {
//         let [_, r, g, b] = buffer[y as usize * 240 + x as usize].to_be_bytes();
//         *pixel = Rgba::<u8>([r, g, b, 0xff]);
//     }
//     if !std::path::Path::new("screenshots/").exists() {
//         std::fs::create_dir("screenshots/").unwrap();
//     }
//     let counter = unsafe { COUNTER };
//     image.save(format!("screenshots/s{counter}.png")).unwrap();
//     unsafe {
//         COUNTER += 1;
//     }
// }

fn make_screenshot(buffer: &[u32], width: usize, height: usize, name: Option<String>) {
    let mut image = image::DynamicImage::new(width as _, height as _, image::ColorType::Rgba8);
    for (x, y, pixel) in image.as_mut_rgba8().unwrap().enumerate_pixels_mut() {
        let [_, r, g, b] = buffer[y as usize * width + x as usize].to_be_bytes();
        *pixel = Rgba::<u8>([r, g, b, 0xff]);
    }
    if !std::path::Path::new("screenshots/").exists() {
        std::fs::create_dir("screenshots/").unwrap();
    }
    let name = name.unwrap_or_else(|| {
        let counter = unsafe { COUNTER };
        let name = format!("s{counter}");
        unsafe {
            COUNTER += 1;
        }
        name
    });
    image.save(format!("screenshots/{name}.png")).unwrap();
}
