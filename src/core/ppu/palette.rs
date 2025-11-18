use crate::core::errors::EmulationError;
use crate::core::memory::MemMapped;
use std::array;
use std::convert::TryFrom;
use std::fs::File;
use std::io::{ErrorKind, Read};
use std::ops::Index;
use std::path::Path;

const DEFAULT_PALETTE_SUBPATH: &str = "palette/DigitalPrime.pal";

const PALETTE_COLOR_BYTE_LEN: usize = 3;

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
pub struct PpuPaletteColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Index<usize> for PpuPaletteColor {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        match index {
            0 => &self.red,
            1 => &self.green,
            2 => &self.blue,
            _ => unreachable!(),
        }
    }
}

impl From<&[u8]> for PpuPaletteColor {
    fn from(triplet: &[u8]) -> Self {
        PpuPaletteColor {
            red: triplet[0],
            green: triplet[1],
            blue: triplet[2],
        }
    }
}

#[derive(Clone, Debug)]
pub struct PpuPalette {
    colors: Box<[PpuPaletteColor; 64]>,
    mapping: [usize; 32],
}

impl Default for PpuPalette {
    fn default() -> Self {
        PpuPalette::load_default().unwrap()
    }
}

impl TryFrom<&[u8]> for PpuPalette {
    type Error = std::io::Error;

    fn try_from(bytes: &[u8]) -> Result<Self, std::io::Error> {
        if bytes.len() < 64 * PALETTE_COLOR_BYTE_LEN {
            Err(std::io::Error::new(
                ErrorKind::UnexpectedEof,
                "PpuPalette needs at least 64 color triplets (192 bytes)",
            ))
        } else {
            let colors: [PpuPaletteColor; 64] = array::from_fn(|index| {
                PpuPaletteColor::from(&bytes[index * 3..(index * 3) + PALETTE_COLOR_BYTE_LEN])
            });

            Ok(PpuPalette {
                colors: Box::new(colors),
                mapping: [0; 32],
            })
        }
    }
}

impl PpuPalette {
    pub fn load(file_path: &Path) -> Result<PpuPalette, std::io::Error> {
        let mut file = File::open(file_path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        PpuPalette::try_from(bytes.iter().as_ref())
    }

    pub fn load_default() -> Result<PpuPalette, std::io::Error> {
        let mut default_palette_path = std::env::current_dir()?.to_path_buf();
        default_palette_path.push(DEFAULT_PALETTE_SUBPATH);
        Self::load(&default_palette_path)
    }

    #[inline]
    pub fn get_background_color(&self, palette_index: u8, color_index: u8) -> PpuPaletteColor {
        if color_index == 0 {
            self.get_transparent_color()
        } else {
            let base_mapping_index = match palette_index {
                0 => 0x1,
                1 => 0x5,
                2 => 0x9,
                3 => 0xD,
                _ => unreachable!(),
            };
            let mapping_index = base_mapping_index + color_index as usize - 1;
            let color_index = self.mapping[mapping_index];
            self.colors[color_index]
        }
    }

    pub fn get_sprite_color(&self, palette_index: u8, color_index: u8) -> PpuPaletteColor {
        if color_index == 0 {
            self.get_transparent_color()
        } else {
            let base_mapping_index = match palette_index {
                0 => 0x11,
                1 => 0x15,
                2 => 0x19,
                3 => 0x1D,
                _ => unreachable!(),
            };
            let mapping_index = base_mapping_index + color_index as usize - 1;
            let color_index = self.mapping[mapping_index];
            self.colors[color_index]
        }
    }

    pub fn get_transparent_color(&self) -> PpuPaletteColor {
        self.colors[self.mapping[0]]
    }

    pub fn is_transparent_color(&self, color: &PpuPaletteColor) -> bool {
        *color == self.colors[self.mapping[0]]
    }
}

impl MemMapped for PpuPalette {
    fn read(&mut self, index: u16) -> Result<u8, EmulationError> {
        Ok(self.mapping[index as usize] as u8)
    }

    fn write(&mut self, index: u16, byte: u8) -> Result<(), EmulationError> {
        let value = byte as usize;
        if index == 0x0 || index == 0x10 {
            self.mapping[0x0] = value;
            self.mapping[0x10] = value;
        } else if index == 0x04 || index == 0x14 {
            self.mapping[0x04] = value;
            self.mapping[0x14] = value;
        } else if index == 0x08 || index == 0x18 {
            self.mapping[0x08] = value;
            self.mapping[0x18] = value;
        } else if index == 0x0C || index == 0x1C {
            self.mapping[0x0C] = value;
            self.mapping[0x1C] = value;
        } else {
            self.mapping[index as usize] = value;
        }
        Ok(())
    }
}
