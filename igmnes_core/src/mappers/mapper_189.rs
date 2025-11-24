use crate::mappers::{CpuMapper, Mapper, MapperIrq, PpuMapper};
use crate::memory::{MemMapped, Ram};
use crate::rom::{MirroringMode, Rom};
use std::ops::Range;

// Mapper189 Bank Sizes
const PRG_BANK_SIZE: usize = 0x8000; // 32 KB
const CHR_BANK_SIZE_1KB: usize = 0x0400; // 1 KB
const PRG_RAM_SIZE: usize = 0x2000; // 8 KB (if present)

// These boards are modified MMC3 boards that bank PRG-ROM in 32 KiB amounts, like AxROM, BNROM and GNROM
#[derive(Clone)]
pub struct Mapper189 {
    vram: Ram,
    prg_rom_bytes: Vec<u8>,
    chr_rom_bytes: Vec<u8>,
    prg_ram_bytes: Vec<u8>,

    prg_bank_index: usize,
    bank_index: u8,
    r: [u8; 8],

    chr_inversion: bool,
    mirroring_mode: MirroringMode,

    irq_latch: u8,
    irq_counter: u8,
    irq_enabled: bool,
    irq_pending: bool,
    irq_reload: bool,
    prev_chr_a12: bool,
}

impl Mapper189 {
    pub fn new(rom: &Rom) -> Mapper189 {
        let prg_rom_bytes = rom.prg_rom_bytes.clone(); // TODO use references!
        let prg_ram_size = if rom.header.prg_ram_size == 0 {
            PRG_RAM_SIZE
        } else {
            rom.header.prg_ram_size
        };
        let prg_ram_bytes: Vec<u8> = vec![0; prg_ram_size as usize];

        Mapper189 {
            vram: Ram::default(),
            prg_rom_bytes: prg_rom_bytes,
            chr_rom_bytes: rom.chr_rom_bytes.clone(),
            prg_ram_bytes: prg_ram_bytes,

            prg_bank_index: 0,
            bank_index: 0,
            r: [0; 8],

            chr_inversion: false,

            mirroring_mode: MirroringMode::Vertical,

            irq_latch: 0,
            irq_counter: 0,
            irq_enabled: false,
            irq_pending: false,
            irq_reload: false,
            prev_chr_a12: false,
        }
    }

    fn get_prg_rom_index(&self, index: u16) -> usize {
        // Banks
        //     CPU $8000-$FFFF: 32 KB switchable PRG ROM bank
        (self.prg_bank_index as usize * PRG_BANK_SIZE) + (index as usize & 0x7FFF)
    }

    fn get_prg_ram_index(&self, index: u16) -> usize {
        (index - 0x6000) as usize % self.prg_ram_bytes.len()
    }

    fn get_chr_rom_index(&self, index: u16) -> usize {
        let addr = index as usize;
        let chr_banks = self.chr_rom_bytes.len() / CHR_BANK_SIZE_1KB;

        let (bank_register_value, addr_offset_mask) = if self.chr_inversion {
            match addr {
                // 2KB banks (R[0] -> $1000, R[1] -> $1800)
                0x1000..=0x17FF => (self.r[0], 0x07FF),
                0x1800..=0x1FFF => (self.r[1], 0x07FF),

                // 1KB banks (R[2]..R[5] -> $0000, $0400, $0800, $0C00)
                0x0000..=0x03FF => (self.r[2], 0x03FF),
                0x0400..=0x07FF => (self.r[3], 0x03FF),
                0x0800..=0x0BFF => (self.r[4], 0x03FF),
                0x0C00..=0x0FFF => (self.r[5], 0x03FF),
                _ => unreachable!(),
            }
        } else {
            match addr {
                // 2KB banks (R[0] -> $0000, R[1] -> $0800)
                0x0000..=0x07FF => (self.r[0], 0x07FF),
                0x0800..=0x0FFF => (self.r[1], 0x07FF),

                // 1KB banks (R[2]..R[5] -> $1000, $1400, $1800, $1C00)
                0x1000..=0x13FF => (self.r[2], 0x03FF),
                0x1400..=0x17FF => (self.r[3], 0x03FF),
                0x1800..=0x1BFF => (self.r[4], 0x03FF),
                0x1C00..=0x1FFF => (self.r[5], 0x03FF),
                _ => unreachable!(),
            }
        };

        let bank_index = if addr_offset_mask == 0x07FF {
            (bank_register_value as usize) & 0xFE // Must be even for 2KB alignment
        } else {
            bank_register_value as usize // Full 8 bits used for 1KB bank
        };

        let bank_start_offset = (bank_index % chr_banks) * CHR_BANK_SIZE_1KB;
        let addr_offset = addr & addr_offset_mask;

        (bank_start_offset + addr_offset) % self.chr_rom_bytes.len()
    }

    fn set_prg_bank_index(&mut self, byte: u8) {
        self.prg_bank_index = (byte | byte >> 4) as usize;
    }
}

impl Mapper for Mapper189 {
    fn hard_reset(&mut self, rom: &Rom) {
        *self = Mapper189::new(rom);
    }
}

impl CpuMapper for Mapper189 {
    #[inline(always)]
    fn read_prg_rom(&self, index: u16) -> u8 {
        let index = self.get_prg_rom_index(index);
        self.prg_rom_bytes[index]
    }

    #[inline(always)]
    fn read_prg_ram(&self, index: u16) -> u8 {
        let index = self.get_prg_ram_index(index);
        self.prg_ram_bytes[index]
    }

    #[inline(always)]
    fn write_prg_ram(&mut self, index: u16, byte: u8) {
        let index = self.get_prg_ram_index(index);
        self.prg_ram_bytes[index] = byte;
    }
}

impl PpuMapper for Mapper189 {
    #[inline(always)]
    fn read_chr_rom(&self, index: u16) -> u8 {
        let index = self.get_chr_rom_index(index);
        self.chr_rom_bytes[index]
    }

    #[inline(always)]
    fn read_chr_rom_range(&self, range: Range<u16>) -> &[u8] {
        // panic!("MMC3 range reads must check bank boundaries!");
        let start_physical_index = self.get_chr_rom_index(range.start);
        let len = (range.end - range.start) as usize;
        let end_physical_index = start_physical_index + len;

        &self.chr_rom_bytes[start_physical_index..end_physical_index]
    }

    fn read_chr_ram(&self, _index: u16) -> u8 {
        panic!("MMC3 does not support CHR RAM at this level.")
    }
    fn read_chr_ram_range(&self, _range: Range<u16>) -> &[u8] {
        panic!("MMC3 does not support CHR RAM at this level.")
    }
    fn write_chr_ram(&mut self, _index: u16, _byte: u8) {
        panic!("MMC3 does not support CHR RAM at this level.")
    }

    #[inline(always)]
    fn get_mirrored_index(&self, index: u16) -> u16 {
        let index = index - 0x2000;
        match self.mirroring_mode {
            MirroringMode::Horizontal => ((index / 0x800) * 0x400) + (index % 0x400),
            MirroringMode::Vertical => index & 0x7FF,
        }
    }
}

impl MapperIrq for Mapper189 {
    fn clock_irq(&mut self, index: u16) {
        let curr_a12 = (index & 0x1000) != 0;
        if curr_a12 && !self.prev_chr_a12 {
            self.irq_tick();
        }
        self.prev_chr_a12 = curr_a12;
    }

    #[inline(always)]
    fn irq_pending(&self) -> bool {
        self.irq_pending
    }
}

impl Mapper189 {
    fn irq_tick(&mut self) {
        if self.irq_counter == 0 || self.irq_reload {
            self.irq_counter = self.irq_latch;
        } else {
            self.irq_counter -= 1;
        }
        if self.irq_counter == 0 && self.irq_enabled {
            self.irq_pending = true;
        }
        self.irq_reload = false;
    }
}

impl MemMapped for Mapper189 {
    #[inline(always)]
    fn read(&mut self, index: u16) -> u8 {
        match index {
            0x0000..=0x1FFF => {
                self.clock_irq(index);
                self.read_chr_rom(index)
            }
            0x2000..=0x3FFF => {
                let index = self.get_mirrored_index(index);
                self.vram.read(index)
            }
            0x6000..=0x7FFF => self.read_prg_ram(index),
            // MMC3 registers are write-only from 0x8000-0xFFFF, so this is PRG ROM read
            0x8000..=0xFFFF => self.read_prg_rom(index),
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    fn write(&mut self, index: u16, byte: u8) {
        match index {
            0x0000..=0x1FFF => return,
            0x2000..=0x3FFF => {
                let index = self.get_mirrored_index(index);
                self.vram.write(index, byte)
            }
            0x4020..=0x5FFF => {
                self.set_prg_bank_index(byte);
            }
            0x6000..=0x7FFF => {
                self.set_prg_bank_index(byte);
                self.write_prg_ram(index, byte)
            }
            0x8000..=0xFFFF => {
                match index {
                    0x8000..=0x9FFF if index % 2 == 0 => {
                        self.chr_inversion = (byte & 0x80) != 0;
                        self.bank_index = byte & 7;
                    }
                    0x8000..=0x9FFF if index % 2 != 0 => {
                        self.r[self.bank_index as usize] = byte;
                    }
                    0xA000..=0xBFFF if index % 2 == 0 => {
                        self.mirroring_mode = match byte & 1 {
                            1 => MirroringMode::Horizontal,
                            0 => MirroringMode::Vertical,
                            _ => unreachable!(),
                        }
                    }
                    0xA001 | 0xA003 | 0xA005 | 0xA007 => {
                        // Note: MMC3 games rarely rely on the protection bits (6, 7), focusing only on enable.
                        // Bit 7: Protection (ignored here), Bit 6: PRG RAM Chip Enable
                        // self.prg_ram_enabled = (byte & 0x40) != 0;
                        // We will assume PRG RAM is always enabled for simplicity based on common practice.
                    }
                    0xC000..=0xDFFF if index % 2 == 0 => {
                        self.irq_latch = byte;
                    }
                    0xC000..=0xDFFF if index % 2 != 0 => {
                        self.irq_reload = true;
                    }
                    0xE000..=0xFFFF if index % 2 == 0 => {
                        self.irq_enabled = false;
                        self.irq_pending = false;
                    }
                    0xE000..=0xFFFF if index % 2 != 0 => {
                        self.irq_enabled = true;
                    }
                    _ => unreachable!(),
                }
            }
            _ => return,
        }
    }

    #[inline(always)]
    fn read_range(&mut self, range: Range<u16>) -> &[u8] {
        // Only the CHR ROM range (0x0000-0x1FFF) is currently supported for range reads.
        match range.start {
            0x0000..=0x1FFF => {
                for addr in range.start..range.end {
                    self.clock_irq(addr);
                }
                self.read_chr_rom_range(range)
            }
            _ => unimplemented!(),
        }
    }
}
