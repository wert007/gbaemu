use clap::Parser;
use gbaemu_core::{
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
    time::Duration,
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
    let gba = Gba::new(game).with_args(args.clone());
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
            let mut window = minifb::Window::new(
                "Palette",
                16,
                16,
                WindowOptions {
                    scale: minifb::Scale::X16,
                    topmost: true,
                    ..Default::default()
                },
            )
            .unwrap();

            while window.is_open() && is_running.load(std::sync::atomic::Ordering::Relaxed) {
                let palette = gba.lock().unwrap().load_palette();
                window.update_with_buffer(&palette, 16, 16).unwrap();
            }
        })
    };
    let mut buffer = [0; 160 * 240];
    if let Some(window) = &mut window {
        while window.is_open() && is_running.load(std::sync::atomic::Ordering::Relaxed) {
            gba.lock().unwrap().swap_buffers(&mut buffer);
            if window.is_key_down(minifb::Key::S) {
                make_screenshot(&buffer);
            }
            window.update_with_buffer(&buffer, 240, 160).unwrap();
        }
        is_running.store(false, std::sync::atomic::Ordering::Relaxed);
    }
    _ = palette_window.join();
    _ = core.join();
    gba.lock().unwrap().swap_buffers(&mut buffer);
    make_screenshot(&buffer);
}

static mut COUNTER: usize = 0;

fn make_screenshot(buffer: &[u32]) {
    let mut image = image::DynamicImage::new(240, 160, image::ColorType::Rgba8);
    for (x, y, pixel) in image.as_mut_rgba8().unwrap().enumerate_pixels_mut() {
        let [_, r, g, b] = buffer[y as usize * 240 + x as usize].to_be_bytes();
        *pixel = Rgba::<u8>([r, g, b, 0xff]);
    }
    if !std::path::Path::new("screenshots/").exists() {
        std::fs::create_dir("screenshots/").unwrap();
    }
    unsafe {
        image.save(format!("screenshots/s{COUNTER}.png")).unwrap();
        COUNTER += 1;
    }
}
