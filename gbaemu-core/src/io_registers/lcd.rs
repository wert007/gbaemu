use std::{
    fmt::Debug,
    sync::{Arc, Mutex},
};

use image::Rgb;

mod obj;

use crate::{
    interrupts::Interrupt,
    io_registers::lcd::obj::Obj,
    memory::{MemoryPlugin, SimpleMemory},
};

use super::{read_byte_from_half_word, write_byte_to_half_word, write_byte_to_word};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotating_iter() {
        let values: Vec<u16> = vert_lines(150, 10).into_iter().collect();
        assert_eq!(
            values,
            &[
                150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9
            ]
        );
    }
}

struct RotatingIterator<const LIMIT: u16> {
    start: u16,
    end: u16,
}

fn vert_lines(from: u16, to: u16) -> RotatingIterator<160> {
    RotatingIterator {
        start: from,
        end: to,
    }
}

impl<const LIMIT: u16> IntoIterator for RotatingIterator<LIMIT> {
    type Item = u16;

    type IntoIter = Box<dyn Iterator<Item = u16>>;

    fn into_iter(self) -> Self::IntoIter {
        if self.start > self.end {
            Box::new((self.start..LIMIT).into_iter().chain(0..self.end))
        } else {
            Box::new(self.start..self.end)
        }
    }
}
pub struct Color {
    r: f32,
    g: f32,
    b: f32,
}

impl Color {
    const RED: Color = Self {
        r: 1.0,
        g: 0.0,
        b: 0.0,
    };

    fn from_raw(half_word: u16) -> Self {
        let r = half_word & 0x001f;
        let g = (half_word & 0x03e0) >> 5;
        let b = (half_word & 0x7c00) >> 10;
        Self {
            r: r as f32 / 31.0,
            g: g as f32 / 31.0,
            b: b as f32 / 31.0,
        }
    }

    fn to_rgb(&self) -> u32 {
        let [r, g, b] = self.to_rgb_bytes();
        u32::from_be_bytes([0, r, g, b])
    }

    fn to_rgb_bytes(&self) -> [u8; 3] {
        let r = (self.r * 255.0).round() as u8;
        let g = (self.g * 255.0).round() as u8;
        let b = (self.b * 255.0).round() as u8;
        [r, g, b]
    }
}

#[repr(usize)]
#[derive(Debug, strum::FromRepr, Clone, Copy)]
pub enum DisplayMode {
    DisplayMode0,
    DisplayMode1,
    DisplayMode2,
    DisplayMode3,
    DisplayMode4,
    DisplayMode5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Bpp4,
    Bpp8,
}

#[derive(Debug, Default)]
pub struct BackgroundControl(u16);
impl BackgroundControl {
    fn priority(&self) -> u16 {
        self.0 & 0x3
    }

    pub fn char_block(&self) -> u32 {
        (((self.0 & 0xc) >> 2) as u32) * 0x4000
    }

    pub fn screen_block(&self) -> u32 {
        (((self.0 & 0x1f00) >> 8) as u32) * 0x800
    }

    pub fn size_regular(&self) -> (u32, u32) {
        match (self.0 & 0xc000) >> 14 {
            0b00 => (256, 256),
            0b01 => (512, 256),
            0b10 => (256, 512),
            0b11 => (512, 512),
            _ => unreachable!(),
        }
    }

    pub fn tile_format(&self) -> (u32, PixelFormat) {
        if self.0 & 0x80 > 0 {
            (2 * 32, PixelFormat::Bpp8)
        } else {
            (32, PixelFormat::Bpp4)
        }
    }
}

struct TileMapEntry(u16);

impl TileMapEntry {
    pub fn tile_index(&self) -> u16 {
        self.0 & 0x03ff
    }

    pub fn x_flip(&self) -> bool {
        (self.0 & 0x0400) > 0
    }

    pub fn y_flip(&self) -> bool {
        (self.0 & 0x0800) > 0
    }

    pub fn palette_bank(&self, pixel_format: PixelFormat) -> u16 {
        match pixel_format {
            PixelFormat::Bpp4 => (self.0 & 0xf000) >> 12,
            PixelFormat::Bpp8 => 0,
        }
    }
}

#[derive(Debug, Default)]
pub struct LcdMemoryInterface {
    ///  4000000h  2    R/W  DISPCNT   LCD Control
    display_control: u16,
    ///  4000002h  2    R/W  -         Undocumented - Green Swap
    green_swap: u16,
    ///  4000004h  2    R/W  DISPSTAT  General LCD Status (STAT,LYC)
    display_stat: u16,
    ///  4000006h  2    R    VCOUNT    Vertical Counter (LY)
    pub vertical_count: u16,
    ///  4000008h  2    R/W  BG0CNT    BG0 Control
    ///  400000Ah  2    R/W  BG1CNT    BG1 Control
    ///  400000Ch  2    R/W  BG2CNT    BG2 Control
    ///  400000Eh  2    R/W  BG3CNT    BG3 Control
    background_control: [BackgroundControl; 4],
    ///  4000010h  2    W    BG0HOFS   BG0 X-Offset
    ///  4000014h  2    W    BG1HOFS   BG1 X-Offset
    ///  4000018h  2    W    BG2HOFS   BG2 X-Offset
    ///  400001Ch  2    W    BG3HOFS   BG3 X-Offset
    background_x_offset: [u16; 4],
    ///  4000012h  2    W    BG0VOFS   BG0 Y-Offset
    ///  4000016h  2    W    BG1VOFS   BG1 Y-Offset
    ///  400001Ah  2    W    BG2VOFS   BG2 Y-Offset
    ///  400001Eh  2    W    BG3VOFS   BG3 Y-Offset
    background_y_offset: [u16; 4],
    ///  4000020h  2    W    BG2PA     BG2 Rotation/Scaling Parameter A (dx)
    ///  4000022h  2    W    BG2PB     BG2 Rotation/Scaling Parameter B (dmx)
    ///  4000024h  2    W    BG2PC     BG2 Rotation/Scaling Parameter C (dy)
    ///  4000026h  2    W    BG2PD     BG2 Rotation/Scaling Parameter D (dmy)
    background_2_rotation_scaling: [u16; 4],
    ///  4000028h  4    W    BG2X      BG2 Reference Point X-Coordinate
    ///  400002Ch  4    W    BG2Y      BG2 Reference Point Y-Coordinate
    background_2_reference_point: [u32; 2],
    ///  4000030h  2    W    BG3PA     BG3 Rotation/Scaling Parameter A (dx)
    ///  4000032h  2    W    BG3PB     BG3 Rotation/Scaling Parameter B (dmx)
    ///  4000034h  2    W    BG3PC     BG3 Rotation/Scaling Parameter C (dy)
    ///  4000036h  2    W    BG3PD     BG3 Rotation/Scaling Parameter D (dmy)
    background_3_rotation_scaling: [u16; 4],
    ///  4000038h  4    W    BG3X      BG3 Reference Point X-Coordinate
    ///  400003Ch  4    W    BG3Y      BG3 Reference Point Y-Coordinate
    background_3_reference_point: [u32; 2],
    ///  4000040h  2    W    WIN0H     Window 0 Horizontal Dimensions
    ///  4000042h  2    W    WIN1H     Window 1 Horizontal Dimensions
    window_horizontal_dimensions: [u16; 2],
    ///  4000044h  2    W    WIN0V     Window 0 Vertical Dimensions
    ///  4000046h  2    W    WIN1V     Window 1 Vertical Dimensions
    window_vertical_dimensions: [u16; 2],
    ///  4000048h  2    R/W  WININ     Inside of Window 0 and 1
    inside_window_0_and_1: u16,
    ///  400004Ah  2    R/W  WINOUT    Inside of OBJ Window & Outside of Windows
    inside_window_obj_and_outside_windows: u16,
    ///  400004Ch  2    W    MOSAIC    Mosaic Size
    mosaic_size: u16,
    ///  4000050h  2    R/W  BLDCNT    Color Special Effects Selection
    color_special_effects: u16,
    ///  4000052h  2    R/W  BLDALPHA  Alpha Blending Coefficients
    alpha_blending: u16,
    ///  4000054h  2    W    BLDY      Brightness (Fade-In/Out) Coefficient
    brightness_coefficient: u16,
    has_render_changes: bool,
}

impl LcdMemoryInterface {
    fn write_background_offset(&mut self, relative_address: usize, byte: u8) {
        self.has_render_changes = true;
        let byte_index = relative_address % 0x4;
        let background_index = relative_address / 4;
        match byte_index {
            0x0..0x2 => write_byte_to_half_word(
                &mut self.background_x_offset[background_index],
                byte_index - 0x0,
                byte,
            ),
            0x2..0x4 => write_byte_to_half_word(
                &mut self.background_y_offset[background_index],
                byte_index - 0x2,
                byte,
            ),
            _ => unreachable!("byte_index is only between 0 and 4"),
        }
    }
}

pub struct Lcd {
    memory_interface: LcdMemoryInterface,
    vram: Arc<Mutex<SimpleMemory>>,
    color_ram: Arc<Mutex<SimpleMemory>>,
    #[allow(dead_code)]
    obj_ram: Arc<Mutex<SimpleMemory>>,
    buffer: [u32; 240 * 160],
    last_rendered_line: u16,
    last_fetched_line: u16,
}

impl Debug for Lcd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#?}", self.memory_interface)
    }
}

impl Lcd {
    pub(crate) fn new(
        vram: Arc<Mutex<SimpleMemory>>,
        color_ram: Arc<Mutex<SimpleMemory>>,
        obj_ram: Arc<Mutex<SimpleMemory>>,
    ) -> Self {
        Self {
            memory_interface: LcdMemoryInterface::default(),
            vram,
            color_ram,
            obj_ram,
            buffer: [0; 240 * 160],
            last_rendered_line: 0,
            last_fetched_line: 0,
        }
    }
    fn display_mode(&self) -> DisplayMode {
        match self.memory_interface.display_control & 0x0007 {
            mode @ 0..=5 => DisplayMode::from_repr(mode as usize).unwrap(),
            err => unreachable!("Invalid background mode {err:x}"),
        }
    }

    pub(crate) fn run_cycle(&mut self, tick: usize) -> Vec<Interrupt> {
        let mut result = Vec::new();
        if tick % 4 == 0 {
            self.memory_interface.vertical_count += 1;
            self.render();
        }
        if self.memory_interface.vertical_count > 228 {
            self.memory_interface.vertical_count = 0;
        }
        if self.memory_interface.vertical_count == 160 {
            result.push(Interrupt::Vblank);
        }
        result
    }

    fn render(&mut self) {
        match self.display_mode() {
            DisplayMode::DisplayMode0 => self.render_display_mode_0(),
            DisplayMode::DisplayMode1 => todo!(),
            DisplayMode::DisplayMode2 => self.render_display_mode_2(),
            DisplayMode::DisplayMode3 => todo!(),
            DisplayMode::DisplayMode4 => todo!(),
            DisplayMode::DisplayMode5 => todo!(),
        }
        self.render_objs();

        self.last_rendered_line = self.memory_interface.vertical_count.min(160);
    }

    fn render_objs(&mut self) {
        let screen_display_obj = (self.memory_interface.display_control & 0x1000) > 0;
        if !screen_display_obj {
            return;
        }
        let objs = self.collect_objs();
        for screen_y in vert_lines(
            self.last_rendered_line,
            self.memory_interface.vertical_count.min(160),
        ) {
            for screen_x in 0..240 {
                for color in objs.iter().filter_map(|o| {
                    o.color_at(
                        screen_x,
                        screen_y,
                        &self.vram.lock().unwrap(),
                        &self.color_ram.lock().unwrap(),
                    )
                }) {
                    let buffer_index = screen_y * 240 + screen_x;
                    self.buffer[buffer_index as usize] = color.to_rgb();
                }
            }
        }
        // dbg!(objs);
        // todo!()
    }

    fn render_display_mode_0(&mut self) {
        let background_color = self.color_ram.lock().unwrap().read_half_word(0x5000000);
        let background_color = Color::from_raw(background_color).to_rgb();
        for screen_y in vert_lines(
            self.last_rendered_line,
            self.memory_interface.vertical_count.min(160),
        ) {
            for screen_x in 0..240 {
                let buffer_index = screen_y as usize * 240 + screen_x;
                self.buffer[buffer_index] = background_color;
            }
        }
        for priority in 0..4 {
            for background in 0..4 {
                let background_priority =
                    self.memory_interface.background_control[background].priority();

                if priority != background_priority {
                    continue;
                }
                let screen_display_bg =
                    (self.memory_interface.display_control & (0x0100 << background)) > 0;
                if !screen_display_bg {
                    continue;
                }
                self.render_background_regular(background);
            }
        }
    }

    fn render_display_mode_2(&mut self) {
        let background_color = self.color_ram.lock().unwrap().read_half_word(0x5000000);
        let background_color = Color::from_raw(background_color).to_rgb();
        for screen_y in vert_lines(
            self.last_rendered_line,
            self.memory_interface.vertical_count.min(160),
        ) {
            for screen_x in 0..240 {
                let buffer_index = screen_y as usize * 240 + screen_x;
                self.buffer[buffer_index] = background_color;
            }
        }
        for priority in 0..4 {
            for background in 0..4 {
                let background_priority =
                    self.memory_interface.background_control[background].priority();

                if priority != background_priority {
                    continue;
                }
                let screen_display_bg =
                    (self.memory_interface.display_control & (0x0100 << background)) > 0;
                if !screen_display_bg {
                    continue;
                }
                self.render_background_regular(background);
            }
        }
    }

    fn render_background_regular(&mut self, background: usize) {
        let horizontal_offset = self.memory_interface.background_x_offset[background] as u32;
        let vertical_offset = self.memory_interface.background_y_offset[background] as u32;
        let tileset_base = self.memory_interface.background_control[background].char_block();
        let tilemap_base = self.memory_interface.background_control[background].screen_block();
        let (tile_size, pixel_format) =
            self.memory_interface.background_control[background].tile_format();
        let (background_width, background_height) =
            self.memory_interface.background_control[background].size_regular();
        let vram = self.vram.lock().unwrap();
        if true {
            // dump palette!
            let color_ram = self.color_ram.lock().unwrap();
            let palette = color_ram.as_bytes();
            dump_palette(background, &palette);
        }
        for screen_y in vert_lines(
            self.last_rendered_line,
            self.memory_interface.vertical_count.min(160),
        ) {
            let screen_y = screen_y as u32;
            let background_y = (screen_y + vertical_offset) % background_height;
            for screen_x in 0..240 {
                let background_x = (screen_x + horizontal_offset) % background_width;
                let screenblock_base = match (background_width, background_height) {
                    (256, 256) => 0,
                    (512, 256) => background_x / 256,
                    (256, 512) => background_y / 256,
                    (512, 512) => 2 * (background_y / 256) + (background_x / 256),
                    _ => unreachable!(),
                };

                let screenblock_entries_row = (background_x / 8) % 32;
                let screenblock_entries_column = (background_y / 8) % 32;

                // this will be non-zero if the h-scroll lands in a middle of a tile
                let tile_pixel_offset_y = background_y % 8;
                let tile_pixel_offset_x = background_x % 8;

                let map_address = tilemap_base
                    + 0x800 * screenblock_base
                    + 2 * (32u32 * screenblock_entries_column + screenblock_entries_row)
                    + 0x6000000
                    + 2 * tile_pixel_offset_x;
                // println!("Reading from 0x{map_address:x}");
                for _ in screenblock_entries_row..32 {
                    let entry = TileMapEntry(vram.read_half_word(map_address as usize));
                    let tile_addr = tileset_base + entry.tile_index() as u32 * tile_size;

                    let index = read_pixel_index(
                        &vram,
                        pixel_format,
                        tile_addr,
                        if entry.x_flip() {
                            7 - tile_pixel_offset_x
                        } else {
                            tile_pixel_offset_x
                        },
                        if entry.y_flip() {
                            7 - tile_pixel_offset_y
                        } else {
                            tile_pixel_offset_y
                        },
                    );
                    let palette_bank = entry.palette_bank(pixel_format);
                    let Some(color) = self.get_palette_color(index, palette_bank, 0) else {
                        continue;
                    };
                    let buffer_index = screen_y * 240 + screen_x;
                    self.buffer[buffer_index as usize] = color.to_rgb();
                    if 240 == screen_x {
                        return;
                    }
                }
            }
        }
    }

    pub fn get_palette_color(&self, index: u8, palette_bank: u16, offset: u32) -> Option<Color> {
        if index == 0 || (palette_bank != 0 && index % 16 == 0) {
            return None;
        }
        let address = offset as usize + 2 * index as usize + 0x20 * palette_bank as usize;
        // dbg!(offset, index, palette_bank);
        // assert!(address >= 0x220 || address == 0, "{address:x}");
        let value = self
            .color_ram
            .lock()
            .unwrap()
            .read_half_word(address + 0x5000000);

        // top bit is ignored
        Some(Color::from_raw(value))
    }

    pub(crate) fn swap_buffers(&mut self, buffer: &mut [u32; 160 * 240]) {
        if self.last_fetched_line > self.last_rendered_line {
            self.last_fetched_line = 0;
        }
        let start_index = self.last_fetched_line as usize * 240;
        let end_index = self.memory_interface.vertical_count.min(159) as usize * 240;
        let end_index = if end_index < start_index {
            buffer[0..end_index].copy_from_slice(&self.buffer[0..end_index]);
            buffer.len()
        } else {
            end_index
        };
        self.last_fetched_line = self.memory_interface.vertical_count.min(159);
        buffer[start_index..end_index].copy_from_slice(&self.buffer[start_index..end_index]);
    }

    pub(crate) fn load_palette(&self) -> Vec<u32> {
        self.color_ram
            .lock()
            .unwrap()
            .as_bytes()
            .chunks_exact(2)
            .map(|b| Color::from_raw(u16::from_le_bytes([b[0], b[1]])).to_rgb())
            .collect()
    }

    pub(crate) fn load_tiles(&self, format: PixelFormat) -> Vec<Vec<u8>> {
        let vram = self.vram.lock().unwrap();
        let bytes = &vram.as_bytes()[0x00000..];
        match format {
            PixelFormat::Bpp4 => {
                bytes
                    .chunks_exact(8 * 8 / 2)
                    .map(|b| {
                        b.into_iter()
                            .flat_map(|b| [b & 0xf0 >> 4, b & 0xf])
                            // .flat_map(|b| [b & 0xf, b & 0xf0 >> 4])
                            .collect::<Vec<u8>>()
                    })
                    .collect()
            }
            PixelFormat::Bpp8 => bytes.chunks_exact(8 * 8).map(|b| b.to_vec()).collect(),
        }
        // tiles
    }

    fn collect_objs(&self) -> Vec<Obj> {
        self.obj_ram
            .lock()
            .unwrap()
            .as_bytes()
            .chunks_exact(8)
            .map(|b| Obj::from_obj_ram(b))
            .enumerate()
            .filter(|(_, o)| o.is_enabled() && o.size().is_some())
            // .inspect(|(u, o)| {
            //     dbg!(u, o);
            // })
            .map(|(_, o)| o)
            .collect()
    }
}

pub fn read_pixel_index(
    vram: &SimpleMemory,
    format: PixelFormat,
    address: u32,
    x: u32,
    y: u32,
) -> u8 {
    match format {
        PixelFormat::Bpp4 => {
            let offset = address + (4 * y + (x / 2));
            let offset = offset as usize;
            let byte = vram.read_byte(offset + 0x6000000);
            if x & 1 != 0 {
                (byte >> 4) as u8
            } else {
                (byte & 0xf) as u8
            }
        }
        PixelFormat::Bpp8 => {
            let offset = address as usize + 0x6000000;
            vram.read_byte(offset + (8 * (y as usize) + (x as usize))) as u8
        }
    }
}

fn dump_palette(background: usize, palette: &[u8]) {
    let entries = 255;
    let width = 16;
    let height = (entries + width - 1) / width;
    let img = image::ImageBuffer::from_par_fn(width, height, |x, y| {
        let index = y * 16 + x;
        let byte_index = index as usize * 2;
        let half_word = u16::from_le_bytes([palette[byte_index], palette[byte_index + 1]]);
        Rgb(Color::from_raw(half_word).to_rgb_bytes())
    });
    img.save(format!("palette-bg{background}.png")).unwrap();
}

impl MemoryPlugin for Lcd {
    fn reset(&mut self) {
        self.memory_interface.reset();
        self.last_fetched_line = 0;
        self.last_rendered_line = 0;
    }
    fn claims_address(&self, address: usize) -> bool {
        (0x4000000..0x4000060).contains(&address)
    }

    fn read_byte(&self, address: usize) -> u8 {
        self.memory_interface.read_byte(address)
    }

    fn write_byte(&mut self, address: usize, byte: u8) {
        self.memory_interface.write_byte(address, byte);
    }
}

impl MemoryPlugin for LcdMemoryInterface {
    fn reset(&mut self) {
        *self = Default::default();
    }
    fn claims_address(&self, address: usize) -> bool {
        (0x4000000..0x4000060).contains(&address)
    }

    fn read_byte(&self, address: usize) -> u8 {
        let relative_address = address & !0x4000000;
        match relative_address {
            0x00..0x02 => read_byte_from_half_word(self.display_control, relative_address),
            0x02..0x04 => read_byte_from_half_word(self.green_swap, relative_address - 0x02),
            0x04..0x06 => read_byte_from_half_word(self.display_stat, relative_address - 0x4),
            0x06..0x08 => read_byte_from_half_word(self.vertical_count, relative_address - 0x06),
            0x8..0xa => {
                read_byte_from_half_word(self.background_control[0].0, relative_address - 0x8)
            }
            0xa..0xc => {
                read_byte_from_half_word(self.background_control[1].0, relative_address - 0xa)
            }
            0xc..0xe => {
                read_byte_from_half_word(self.background_control[2].0, relative_address - 0xc)
            }
            0xe..0x10 => {
                read_byte_from_half_word(self.background_control[3].0, relative_address - 0xe)
            }
            0x10..0x48 | 0x4c..0x50 | 0x54..0x60 => 0,
            0x48..0x4a => {
                read_byte_from_half_word(self.inside_window_0_and_1, relative_address - 0x48)
            }
            0x4a..0x4c => read_byte_from_half_word(
                self.inside_window_obj_and_outside_windows,
                relative_address - 0x4a,
            ),
            0x50..0x52 => {
                read_byte_from_half_word(self.color_special_effects, relative_address - 0x50)
            }
            0x52..0x54 => read_byte_from_half_word(self.alpha_blending, relative_address - 0x52),

            _ => todo!("reading from {address:0x}"),
        }
    }

    fn write_byte(&mut self, address: usize, byte: u8) {
        let relative_address = address & !0x4000000;
        match relative_address {
            0x0..0x2 => {
                println!("Writing byte {byte:x} to {relative_address}!");
                write_byte_to_half_word(&mut self.display_control, relative_address, byte)
            }
            0x2..0x4 => write_byte_to_half_word(&mut self.green_swap, relative_address - 0x2, byte),
            0x4..0x6 => {
                write_byte_to_half_word(&mut self.display_stat, relative_address - 0x4, byte)
            }
            0x6..0x8 => {
                // TODO: This can only be read!
            }
            0x8..0xa => write_byte_to_half_word(
                &mut self.background_control[0].0,
                relative_address - 0x8,
                byte,
            ),
            0xa..0xc => write_byte_to_half_word(
                &mut self.background_control[1].0,
                relative_address - 0xa,
                byte,
            ),
            0xc..0xe => write_byte_to_half_word(
                &mut self.background_control[2].0,
                relative_address - 0xc,
                byte,
            ),
            0xe..0x10 => write_byte_to_half_word(
                &mut self.background_control[3].0,
                relative_address - 0xe,
                byte,
            ),
            0x10..0x20 => self.write_background_offset(relative_address - 0x10, byte),
            0x20..0x28 => write_rotation_scaling(
                &mut self.background_2_rotation_scaling,
                relative_address - 0x20,
                byte,
            ),
            0x28..0x2C => write_byte_to_word(
                &mut self.background_2_reference_point[0],
                relative_address - 0x28,
                byte,
            ),
            0x2C..0x30 => write_byte_to_word(
                &mut self.background_2_reference_point[1],
                relative_address - 0x2C,
                byte,
            ),
            0x30..0x38 => write_rotation_scaling(
                &mut self.background_3_rotation_scaling,
                relative_address - 0x30,
                byte,
            ),
            0x38..0x3C => write_byte_to_word(
                &mut self.background_3_reference_point[0],
                relative_address - 0x38,
                byte,
            ),
            0x3C..0x40 => write_byte_to_word(
                &mut self.background_3_reference_point[1],
                relative_address - 0x3C,
                byte,
            ),
            0x40..0x42 => write_byte_to_half_word(
                &mut self.window_horizontal_dimensions[0],
                relative_address - 0x40,
                byte,
            ),
            0x42..0x44 => write_byte_to_half_word(
                &mut self.window_horizontal_dimensions[1],
                relative_address - 0x42,
                byte,
            ),
            0x44..0x46 => write_byte_to_half_word(
                &mut self.window_vertical_dimensions[0],
                relative_address - 0x44,
                byte,
            ),
            0x46..0x48 => write_byte_to_half_word(
                &mut self.window_vertical_dimensions[1],
                relative_address - 0x46,
                byte,
            ),
            0x48..0x4a => write_byte_to_half_word(
                &mut self.inside_window_0_and_1,
                relative_address - 0x48,
                byte,
            ),
            0x4a..0x4c => write_byte_to_half_word(
                &mut self.inside_window_obj_and_outside_windows,
                relative_address - 0x4a,
                byte,
            ),
            0x4c..0x4e => {
                write_byte_to_half_word(&mut self.mosaic_size, relative_address - 0x4c, byte)
            }
            0x4e..0x50 => {}
            0x50..0x52 => write_byte_to_half_word(
                &mut self.color_special_effects,
                relative_address - 0x50,
                byte,
            ),
            0x52..0x54 => {
                write_byte_to_half_word(&mut self.alpha_blending, relative_address - 0x52, byte)
            }
            0x54..0x56 => write_byte_to_half_word(
                &mut self.brightness_coefficient,
                relative_address - 0x54,
                byte,
            ),
            0x56..0x60 => {}
            _ => todo!("write {byte:x} to {address:0x} in lcd"),
        }
    }
}

fn write_rotation_scaling(
    background_rotation_scaling: &mut [u16; 4],
    relative_address: usize,
    byte: u8,
) {
    let byte_index = relative_address % 2;
    let index = relative_address >> 1;
    write_byte_to_half_word(&mut background_rotation_scaling[index], byte_index, byte)
}
