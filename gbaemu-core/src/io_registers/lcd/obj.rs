use crate::{
    io_registers::lcd::{Color, PixelFormat},
    memory::{MemoryPlugin, SimpleMemory},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjMode {
    Normal,
    SemiTransparent,
    ObjWindow,
    Invalid,
}

impl TryFrom<u16> for ObjMode {
    type Error = u16;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Normal),
            1 => Ok(Self::SemiTransparent),
            2 => Ok(Self::ObjWindow),
            3 => Ok(Self::Invalid),
            value => Err(value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjShape {
    Square,
    Horizontal,
    Vertical,
    Invalid,
}

impl TryFrom<u16> for ObjShape {
    type Error = u16;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Square),
            1 => Ok(Self::Horizontal),
            2 => Ok(Self::Vertical),
            3 => Ok(Self::Invalid),
            value => Err(value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjPriority {
    Max,
    High,
    Low,
    Min,
}

impl TryFrom<u16> for ObjPriority {
    type Error = u16;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Max),
            1 => Ok(Self::High),
            2 => Ok(Self::Low),
            3 => Ok(Self::Min),
            value => Err(value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjSize {
    Small,
    Normal,
    Big,
    Huge,
}

impl TryFrom<u16> for ObjSize {
    type Error = u16;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Small),
            1 => Ok(Self::Normal),
            2 => Ok(Self::Big),
            3 => Ok(Self::Huge),
            value => Err(value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RotationScaleIndex(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaletteNumber(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Obj {
    y: i8,
    x: i16,
    enabled: bool,
    priority: ObjPriority,
    doubled_size: bool,
    shape: ObjShape,
    size: ObjSize,
    mode: ObjMode,
    mosaic: bool,
    rotation_scale_index: Option<RotationScaleIndex>,
    horizontal_flip: bool,
    vertical_flip: bool,
    tile_number: u32,
    palette_number: Option<PaletteNumber>,
}

impl Obj {
    pub fn size(&self) -> Option<(usize, usize)> {
        Some(match (self.size, self.shape) {
            (ObjSize::Small, ObjShape::Square) => (8, 8),
            (ObjSize::Small, ObjShape::Horizontal) => (16, 8),
            (ObjSize::Small, ObjShape::Vertical) => (8, 16),
            (ObjSize::Normal, ObjShape::Square) => (16, 16),
            (ObjSize::Normal, ObjShape::Horizontal) => (32, 8),
            (ObjSize::Normal, ObjShape::Vertical) => (8, 32),
            (ObjSize::Big, ObjShape::Square) => (32, 32),
            (ObjSize::Big, ObjShape::Horizontal) => (32, 16),
            (ObjSize::Big, ObjShape::Vertical) => (16, 32),
            (ObjSize::Huge, ObjShape::Square) => (64, 64),
            (ObjSize::Huge, ObjShape::Horizontal) => (64, 32),
            (ObjSize::Huge, ObjShape::Vertical) => (32, 64),
            (_, ObjShape::Invalid) => return None,
        })
    }

    pub fn from_obj_ram(bytes: &[u8]) -> Obj {
        let attribute_1 = u16::from_le_bytes([bytes[0], bytes[1]]);
        let attribute_2 = u16::from_le_bytes([bytes[2], bytes[3]]);
        let attribute_3 = u16::from_le_bytes([bytes[4], bytes[5]]);
        let y = attribute_1 as u8 as i8;
        let (doubled_size, enabled, rotation_scale_index, horizontal_flip, vertical_flip) =
            if attribute_1 & 0x10 > 0 {
                // let a = 0xffffu16;
                let index = (attribute_2 & 0x3f00) >> 0;
                (
                    attribute_1 & 0x20 > 0,
                    true,
                    Some(RotationScaleIndex(index as u8)),
                    false,
                    false,
                )
            } else {
                (
                    false,
                    attribute_1 & 0x20 > 0,
                    None,
                    attribute_2 & 0x4000 > 0,
                    attribute_2 & 0x8000 > 0,
                )
            };
        let x = attribute_2 & 0x1ff;
        let x = if x & 0x100 > 0 { x | 0xff00 } else { x } as i16;
        let priority = (attribute_3 & 0xc00) >> 10;
        assert!(matches!(priority, 0 | 1 | 2 | 3));
        let priority = priority.try_into().unwrap();
        let shape = (attribute_1 & 0xc000) >> 14;
        assert!(matches!(shape, 0 | 1 | 2 | 3));
        let shape = shape.try_into().unwrap();
        let size = (attribute_2 & 0xc000) >> 14;
        assert!(matches!(size, 0 | 1 | 2 | 3));
        let size = size.try_into().unwrap();
        let mode = (attribute_1 & 0xc00) >> 10;
        assert!(matches!(mode, 0 | 1 | 2 | 3));
        let mode = mode.try_into().unwrap();
        let mosaic = (attribute_2 & 0x1000) > 0;
        let palette_number = if (attribute_1 & 0x200) > 0 {
            let palette = (attribute_3 & 0xf000) >> 12;
            Some(PaletteNumber(palette as u8))
        } else {
            None
        };
        let tile_number = attribute_3 & 0x3ff;

        // let mode =
        Obj {
            y,
            x,
            enabled,
            priority,
            doubled_size,
            shape,
            size,
            mode,
            mosaic,
            rotation_scale_index,
            horizontal_flip,
            vertical_flip,
            tile_number: tile_number as _,
            palette_number,
        }
    }

    pub fn tile_number(&self) -> u32 {
        if self.palette_number.is_some() {
            self.tile_number
        } else {
            self.tile_number >> 1
        }
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn color_at(
        &self,
        screen_x: u16,
        screen_y: u16,
        vram: &SimpleMemory,
        color_ram: &SimpleMemory,
    ) -> Option<Color> {
        let (w, h) = self.size()?;
        let (x, y) = (self.x(), self.y());
        let (screen_x, screen_y) = (screen_x as usize, screen_y as usize);
        // dbg!(x, y, w, h, screen_x, screen_y);
        if x > screen_x as i16
            || x + (w as i16) < screen_x as i16
            || y > screen_y as i16
            || y + (h as i16) < screen_y as i16
        {
            return None;
        }

        let tile_offset_y = screen_y as i16 - self.y();
        let tile_offset_x = screen_x as i16 - self.x();
        if tile_offset_x < 0 || tile_offset_y < 0 {
            return None;
        }
        let tile_pixel_offset_y = tile_offset_y as u32 % 8;
        let tile_pixel_offset_x = tile_offset_x as u32 % 8;
        let tile_offset_y = tile_offset_y as u32 / 8;
        let tile_offset_x = tile_offset_x as u32 / 8;
        let (tile_size, pixel_format) = if self.palette_number.is_some() {
            (32, PixelFormat::Bpp4)
        } else {
            (64, PixelFormat::Bpp8)
        };

        let index = match pixel_format {
            PixelFormat::Bpp4 => {
                vram.read_byte(
                    (0x6010000
                    + self.tile_number() * tile_size
                    // + 1 * tile_size
                    // + 1 * tile_size * 16
                    + tile_offset_y * tile_size * 16
                    + tile_pixel_offset_y * 8
                    + tile_offset_x * tile_size
                    + tile_pixel_offset_x) as usize,
                )
                // 1
            }
            PixelFormat::Bpp8 => vram.read_byte(
                (0x6010000
                    + self.tile_number() * tile_size
                    // + 1 * tile_size
                    // + 1 * tile_size * 16
                    + tile_offset_y * tile_size * 16
                    + tile_pixel_offset_y * 8
                    + tile_offset_x * tile_size
                    + tile_pixel_offset_x) as usize,
            ),
        };
        if index == 0 {
            return None;
        }
        let palette_bank = self.palette_number.map(|p| p.0).unwrap_or_default();
        let address = 2 * index as usize + 0x20 * palette_bank as usize;
        let value = color_ram.read_half_word(address + 0x05000200);

        Some(Color::from_raw(value))
    }

    pub(crate) fn x(&self) -> i16 {
        self.x as i16
    }

    pub(crate) fn y(&self) -> i16 {
        self.y as i8 as i16
    }
}
