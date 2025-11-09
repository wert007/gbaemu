use clap::Parser;
use gbaemu_core::{
    lcd::PixelFormat,
    plugins::Debugger,
    registers::{RegisterIndex, RegisterList},
    Cartridge, Gba, GbaArgs,
};
use image::Rgba;
use minifb::WindowOptions;
use std::{
    fs::File,
    io::BufReader,
    sync::{atomic::AtomicBool, Arc, Mutex},
    thread,
};

#[derive(Parser, Debug, Clone)]
struct GbaEmuArgs {
    #[clap(short, long)]
    silent: bool,
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
}

impl Into<GbaArgs> for GbaEmuArgs {
    fn into(self) -> GbaArgs {
        GbaArgs {
            silent: self.silent,
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
    let debugger = Debugger::new();
    // debugger
    // .with_breakpoint(0x3b4)
    // .with_watch_stack();
    gba.with_plugin(debugger);
    let gba = Arc::new(Mutex::new(gba));
    let is_running = Arc::new(AtomicBool::new(true));
    let core = {
        let gba = gba.clone();
        let is_running = is_running.clone();
        thread::spawn(move || {
            'thread: while is_running.load(std::sync::atomic::Ordering::Relaxed) {
                for _ in 0..100 {
                    if !gba.lock().unwrap().run_cycle() && false {
                        break 'thread;
                    }
                }
                // std::thread::sleep(Duration::from_nanos(1));
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
                make_screenshot(&buffer, 32 * 8, 32 * 8, Some("tileset".into()));
                window.update_with_buffer(&buffer, 32 * 8, 32 * 8).unwrap();
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
            window.update_with_buffer(&buffer, 240, 160).unwrap();
        }
        is_running.store(false, std::sync::atomic::Ordering::Relaxed);
    }
    _ = palette_window.join();
    _ = tilemap_window.join();
    _ = core.join();
    gba.lock().unwrap().swap_buffers(&mut buffer);
    make_screenshot(&buffer, 240, 160, None);
}

fn load_tile_data(
    gba: &Arc<Mutex<Gba>>,
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
