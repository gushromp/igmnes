use core::errors::EmulationError;
use core::memory::{CpuMemMap, MemMapped};
use core::ppu::PpuMemMap;

#[derive(Default)]
pub struct Dma {
    page_index: u8,
    dma_cycle_count: u8,

    pub dma_type: Option<DmaType>
}

pub enum DmaType {
    OAM,
    DMC
}

impl Dma {
    pub fn new() -> Dma {
        Dma::default()
    }
    pub fn start_dma(&mut self, dma_type: DmaType, page_index: u8) {
        self.dma_type = Some(dma_type);
        self.page_index = page_index;
        self.dma_cycle_count = 0;
    }

    pub fn step(&mut self, mem_map: &mut CpuMemMap) -> Result<(), EmulationError> {
        if self.dma_type.is_none() {
            return Ok(())
        }

        if self.dma_cycle_count == 0 {
            let range_start = self.page_index as u16 * 0x100;
            let range_end = range_start + 0x100;
            let cpu_mem = mem_map.ram.read_range(range_start..range_end)?;
            mem_map.ppu_mem_map.oam_table.write(cpu_mem)?;
        }
        self.dma_cycle_count += 2;
        Ok(())
    }

    pub fn is_dma_active(&self) -> bool {
        self.dma_type.is_some()
    }
}