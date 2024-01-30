use std::path::Path;
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use nom::*;

const PRG_ROM_BYTES_PER_CHUNK: usize = 16384;
const CHR_ROM_BYTES_PER_CHUNK: usize = 8192;
const PRG_RAM_BYTES_PER_CHUNK: usize = 8192;

#[derive(Debug)]
pub enum TVSystem {
    NTSC,
    PAL,
    DualCompatible,
}

impl Default for TVSystem {
    fn default() -> TVSystem {
        TVSystem::NTSC
    }
}

#[derive(Debug)]
pub enum HeaderType {
    Standard,
    Extended,
}

impl Default for HeaderType {
    fn default() -> HeaderType {
        HeaderType::Standard
    }
}

#[derive(Debug)]
pub enum MirroringMode {
    Horizontal,
    Vertical,
}

impl Default for MirroringMode {
    fn default() -> MirroringMode {
        MirroringMode::Horizontal
    }
}

#[derive(Debug, Default)]
pub struct HeaderExtension {
    pub mapper_number: u16,
    pub submapper_number: u8,
}

#[derive(Debug, Default)]
pub struct Header {
    pub header_type: HeaderType,
    pub prg_rom_size: usize,
    pub chr_rom_size: usize,
    pub prg_ram_size: usize,
    pub mapper_number: u16,
    pub four_screen_mode: bool,
    pub trainer_present: bool,
    pub sram_present: bool,
    pub mirroring_mode: MirroringMode,
    pub is_playchoice_10: bool,
    pub is_vs_unisystem: bool,
    pub tv_system: TVSystem,
    pub extension: Option<HeaderExtension>,
}

#[derive(Debug, Default)]
pub struct Rom {
    pub header: Header,
    pub trainer_bytes: Option<Vec<u8>>,
    pub prg_rom_bytes: Vec<u8>,
    pub chr_rom_bytes: Vec<u8>,
}

impl Rom {
    pub fn load_rom(file_path: &Path) -> Result<Rom, Box<dyn Error>> {
        let mut file = File::open(file_path)?;
        let mut bytes = Vec::new();

        file.read_to_end(&mut bytes)?;

        let rom = parse_rom(&bytes).unwrap().1;
        Ok(rom)
    }
}

fn parse_header(input: &[u8]) -> IResult<&[u8], Header> {
    do_parse!(input,
        tag!("\x4E\x45\x53\x1A")        >>
        prg_rom_chunk_count: le_u8      >>
        chr_rom_chunk_count: le_u8      >>
        flags_6: le_u8                  >>
        flags_7: le_u8                  >>
        byte_8: le_u8                   >>
        flags_9: le_u8                  >>
        flags_10: le_u8                 >>
        flags_11: le_u8                 >>
        flags_12: le_u8                 >>
        flags_13: le_u8                 >>
        rest: take!(2)                  >>
        (
            {
                let header_type = detect_header_type(flags_7);

                let prg_rom_size = prg_rom_chunk_count as usize * PRG_ROM_BYTES_PER_CHUNK;
                let chr_rom_size = chr_rom_chunk_count as usize * CHR_ROM_BYTES_PER_CHUNK;

                let four_screen_mode = ((flags_6 >> 3) & 0b1) == 0b1;
                let trainer_present = ((flags_6 >> 2) & 0b1) == 0b1;
                let sram_present = ((flags_6 >> 1) & 0b1) == 0b1;
                let mirroring_mode = match flags_6 & 0b1 == 0b1 {
                    false => MirroringMode::Horizontal,
                    true => MirroringMode::Vertical,
                };

                let (mut prg_ram_chunk_count, mapper_number, _submapper_number) = match header_type {
                    HeaderType::Standard => {
                        (
                            byte_8,
                            ((flags_7 & 0b11110000) as u16 | (flags_6 >> 4) as u16) as u16,
                            0
                        )
                    },
                    HeaderType::Extended =>  {
                        let flags_8 = byte_8;
                        (
                            0,
                            (((flags_8 as u16 & 0b00001111) << 8) | (flags_7 as u16 & 0b11110000) | (flags_6 as u16 >> 4)) as u16,
                            flags_8 >> 4
                        )
                    }
                };

                if prg_ram_chunk_count == 0 {
                 prg_ram_chunk_count = 1;
                }
                let prg_ram_size = prg_ram_chunk_count as usize * PRG_RAM_BYTES_PER_CHUNK;

                let is_playchoice_10 = (flags_7 >> 1) & 0b1 == 0b1;
                let is_vs_unisystem = flags_7 & 0b1 == 0b1;

                let tv_system = {
                    let byte_to_check = match header_type {
                        HeaderType::Standard => flags_9,
                        HeaderType::Extended => flags_12,
                    };

                    match byte_to_check & 0b00000011 {
                        0b00 => TVSystem::NTSC,
                        0b10 => TVSystem::PAL,
                        _ => TVSystem::DualCompatible,
                    }
                };

                Header {
                    header_type: header_type,
                    prg_rom_size: prg_rom_size,
                    chr_rom_size: chr_rom_size,
                    prg_ram_size: prg_ram_size,
                    mapper_number: mapper_number,
                    four_screen_mode: four_screen_mode,
                    trainer_present: trainer_present,
                    sram_present: sram_present,
                    mirroring_mode: mirroring_mode,
                    is_playchoice_10: is_playchoice_10,
                    is_vs_unisystem: is_vs_unisystem,
                    tv_system: tv_system,
                    extension: None,
                }
                // TODO support NES 2.0 file format (Extended)
            }
        )
    )
}

fn parse_trainer(input: &[u8], trainer_present: bool) -> IResult<&[u8], Option<Vec<u8>>> {
    if trainer_present {
        do_parse!(input,
            bytes: take!(512) >>
            ( Some(bytes.to_vec()) )
        )
    } else {
        IResult::Done(input, None)
    }
}

fn parse_rom(input: &[u8]) -> IResult<&[u8], Rom> {
    do_parse!(input,
        header: parse_header                                    >>
        trainer_bytes: apply!(parse_trainer, header.trainer_present)  >>
        prg_rom_bytes: take!(header.prg_rom_size)               >>
        chr_rom_bytes: take!(header.chr_rom_size)               >>
        (
            Rom {
                header: header,
                trainer_bytes: trainer_bytes,
                prg_rom_bytes: prg_rom_bytes.to_vec(),
                chr_rom_bytes: chr_rom_bytes.to_vec(),
            }
        )
    )
}

fn detect_header_type(flags_7: u8) -> HeaderType {
    if flags_7 & 0b00001100 == 0b00001000 {
        HeaderType::Extended
    } else {
        HeaderType::Standard
    }
}
