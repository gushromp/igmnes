use std::array;
use std::convert::TryFrom;
use std::fs::File;
use std::io::{ErrorKind, Read};
use std::ops::Index;
use std::path::Path;
use core::errors::EmulationError;
use core::memory::MemMapped;

const DEFAULT_PALETTE_SUBPATH: &str = "palette/DigitalPrime.pal";

const PALETTE_COLOR_BYTE_LEN: usize = 3;

#[derive(Debug, Default, Copy, Clone)]
pub struct PpuPaletteColor {
    red: u8,
    green: u8,
    blue: u8
}

impl Index<usize> for PpuPaletteColor {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        match index {
            0 => &self.red,
            1 => &self.green,
            2 => &self.blue,
            _ => unreachable!()
        }
    }
}

impl From<&[u8]> for PpuPaletteColor {
    fn from(triplet: &[u8]) -> Self {
        PpuPaletteColor {
            red: triplet[0],
            green: triplet[1],
            blue: triplet[2]
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
            Err(std::io::Error::new(ErrorKind::UnexpectedEof,"PpuPalette needs at least 64 color triplets (192 bytes)"))
        } else {
            let colors: [PpuPaletteColor; 64] = array::from_fn(|index| {
                PpuPaletteColor::from(&bytes[index..index + PALETTE_COLOR_BYTE_LEN])
            });

            Ok(PpuPalette {
                colors: Box::new(colors),
                mapping: [0; 32]
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
        let mut default_palette_path = std::env::current_dir()?
            .to_path_buf();
        default_palette_path.push(DEFAULT_PALETTE_SUBPATH);
        Self::load(&default_palette_path)
    }
}

impl MemMapped for PpuPalette {
    fn read(&mut self, index: u16) -> Result<u8, EmulationError> {

        Ok(self.mapping[index as usize] as u8)
    }

    fn write(&mut self, index: u16, byte: u8) -> Result<(), EmulationError> {
        let value = byte as usize;
        if index == 0 || index == 10 {
            self.mapping[0] = value;
            self.mapping[10] = value;
        } else {
            self.mapping[index as usize] = value;
        }
        Ok(())
    }
}