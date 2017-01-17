// const SAMPLE_RATE: u32 = 8000;
use std::process;
use core::memory::MemMapped;
use core::errors::EmulationError;

// Length counter lookup table
const LC_LOOKUP_TABLE: [u8; 32] =
[10, 254, 20, 2, 40, 4, 80, 6,
    160, 8, 60, 10, 14, 12, 26, 14,
    12, 16, 24, 18, 48, 20, 96, 22,
    192, 24, 72, 26, 16, 28, 32, 30];

trait ApuChannel {
    fn step(&mut self);
    fn output(&self) -> u8;
}

#[derive(Debug, Default)]
struct Pulse {
    // Duty for current APU frame
    duty: u8,
    // Volume for current APU frame
    constant_volume: bool,
    volume: u8,
    // Sweep:
    sweep_enabled: bool,
    sweep_period: u8,
    sweep_negate: bool,
    sweep_shift: u8,
    // Timer is 11-bit (bits 11-15 are disregarded)
    timer: u16,
    // Length counter is 5-bit (bits 5-7 are disregarded)
    lc_halt_env_loop: bool,
    length_counter: u8,


}

impl Pulse {
    fn write_ddlcvvvv(&mut self, byte: u8) {
        self.duty = byte >> 6;
        self.constant_volume = byte & 0b0001_0000 != 0;
        self.volume = byte & 0b1111;
        self.lc_halt_env_loop = byte & 0b0010_0000 != 0;
    }

    fn write_epppnsss(&mut self, byte: u8) {
        self.sweep_enabled = byte & 0b1000_0000 != 0;
        self.sweep_period = (byte >> 4) & 0b111;
        self.sweep_negate = byte & 0b0000_1000 != 0;
        self.sweep_shift = byte & 0b111;
    }

    fn write_tttttttt(&mut self, byte: u8) {
        let timer_high = self.timer >> 8;
        let timer_low = byte as u16;
        self.timer = (timer_high << 8) | timer_low;
    }

    fn write_lllllttt(&mut self, byte: u8) {
        let timer_high = (byte & 0b111) as u16;
        let timer_low = self.timer & 0xFF;
        self.timer = (timer_high << 8) | timer_low;

        let length_counter_index = (byte >> 3) as usize;
        self.length_counter = LC_LOOKUP_TABLE[length_counter_index];
    }

}

impl ApuChannel for Pulse {

    fn step(&mut self) {
        self.length_counter -= 1;
    }

    fn output(&self) -> u8 {
        0
    }
}

#[derive(Debug, Default)]
struct Triangle {
    lengthc_halt_linearc_control: bool,
    linear_counter_load: u8,

    timer: u16,
    length_counter: u8,
}

impl Triangle {
    fn write_crrrrrrr(&mut self, byte: u8) {
        self.lengthc_halt_linearc_control = byte & 0b1000_0000 != 0;
        self.linear_counter_load = byte & 0b0111_1111;
    }

    fn write_uuuuuuuu(&mut self, byte: u8) {}

    fn write_tttttttt(&mut self, byte: u8) {
        let timer_high = self.timer >> 8;
        let timer_low = byte as u16;
        self.timer = (timer_high << 8) | timer_low;
    }

    fn write_lllllttt(&mut self, byte: u8) {
        let timer_high = byte & 0b111;
        let timer_low = self.timer & 0xFF;
        self.timer = ((timer_high as u16) << 8) | timer_low;

        let length_counter_index = (byte >> 3) as usize;
        self.length_counter = LC_LOOKUP_TABLE[length_counter_index];
    }
}

impl ApuChannel for Triangle {
    fn step(&mut self) {

    }

    fn output(&self) -> u8 {
        0
    }
}

#[derive(Debug, Default)]
struct Noise {
    volume: u8,
    lc_halt_env_loop: bool,
    constant_volume: bool,
    noise_loop: bool,
    noise_period: u8,

    length_counter: u8,
}

impl Noise {
    fn write_uulcvvvv(&mut self, byte: u8) {
        self.lc_halt_env_loop = byte & 0b0010_0000 != 0;
        self.constant_volume = byte & 0b0001_0000 != 0;
        self.volume = byte & 0b1111;
    }

    fn write_uuuuuuuu(&mut self, byte: u8) {}

    fn write_luuupppp(&mut self, byte: u8) {
        self.noise_loop = byte & 0b1000_0000 != 0;
        self.noise_period = byte & 0b1111;
    }

    fn write_llllluuu(&mut self, byte: u8) {
        let length_counter_index = (byte >> 3) as usize;
        self.length_counter = LC_LOOKUP_TABLE[length_counter_index];
    }
}

impl ApuChannel for Noise {
    fn step(&mut self) {

    }

    fn output(&self) -> u8 {
        0
    }
}

#[derive(Debug, Default)]
struct DMC {
    irq_enable: bool,
    dmc_loop: bool,
    frequency: u8,
    load_counter: u8,
    sample_address: u8,
    sample_length: u8,
}

impl DMC {
    fn write_iluurrrr(&mut self, byte: u8) {
        self.irq_enable = byte & 0b1000_0000 != 0;
        self.dmc_loop = byte & 0b0100_0000 != 0;
        self.frequency = byte & 0b1111;
    }

    fn write_udddddddd(&mut self, byte: u8) {
        self.load_counter = byte & 0b0111_1111;
    }

    fn write_aaaaaaaa(&mut self, byte: u8) {
        self.sample_address = byte;
    }

    fn write_llllllll(&mut self, byte: u8) {
        self.sample_length = byte;
    }
}

impl ApuChannel for DMC {
    fn step(&mut self) {

    }

    fn output(&self) -> u8 {
        0
    }
}

pub struct Apu {
    // Waveform/Sample generators
    pulse1: Pulse,
    pulse2: Pulse,
    triangle: Triangle,
    noise: Noise,
    dmc: DMC,

    // Mixer
    pulse_table: [u8; 31],
    tnd_table: [u8; 203],

    // Status register
    // Enable DMC (D), noise (N), triangle (T), and pulse channels (p2/p1)
    reg_status: u8,
    // Frame counter
    // Mode (M, 0 = 4-step, 1 = 5-step), IRQ inhibit flag (I), unused (U)
    reg_frame_counter: u8,
}

impl Default for Apu {
    fn default() -> Apu {
        Apu {
            pulse1: Pulse::default(),
            pulse2: Pulse::default(),
            triangle: Triangle::default(),
            noise: Noise::default(),
            dmc: DMC::default(),

            pulse_table: [0; 31],
            tnd_table: [0; 203],

            reg_status: 0,
            reg_frame_counter: 0,
        }
    }
}

impl Apu {
    pub fn new() -> Apu {
        let mut pulse_table: [u8; 31] = [0; 31];
        let mut tnd_table: [u8; 203] = [0; 203];

        // Avoid division by 0
        pulse_table[0] = 0;
        for n in 1..31 {
            let pulse_n: f32 = 95.52 / (8128.0 / n as f32 + 100.0);
            pulse_table[n] = (pulse_n * u8::max_value() as f32) as u8;
        }

        tnd_table[0] = 0;
        for n in 1..203 {
            let tnd_n: f32 = 163.67 / (24329.0 / n as f32 + 100.0);
            tnd_table[n] = (tnd_n * u8::max_value() as f32) as u8;
        }

        Apu {
            pulse1: Pulse::default(),
            pulse2: Pulse::default(),
            triangle: Triangle::default(),
            noise: Noise::default(),
            dmc: DMC::default(),

            pulse_table: pulse_table,
            tnd_table: tnd_table,

            reg_status: 0,
            reg_frame_counter: 0,
        }
    }

    fn write_status(&mut self, byte: u8) {
        self.reg_status = byte;
    }

    fn write_frame_counter(&mut self, byte: u8) {
        self.reg_frame_counter = byte;
    }

    fn mix(&self) -> u8 {
        // We add outputs of pulse1 and pulse 2 channels
        // and use that value as an index into the pulse output lookup table
        let pulse_output_index: usize
            = self.pulse1.output() as usize + self.pulse2.output() as usize;

        // We use outputs of triangle, noise and DMC channels
        // as an index into the tnd output lookup table
        let tnd_output_index: usize
            = 3 * self.triangle.output() as usize + 2 * self.noise.output() as usize + self.dmc.output() as usize;

        let pulse_output = self.pulse_table[pulse_output_index];
        let tnd_output = self.tnd_table[tnd_output_index];

        let output: u8 = pulse_output.wrapping_add(tnd_output);

        output
    }

    pub fn step(&mut self) {

    }
}

impl MemMapped for Apu {
    fn read(&self, addr: u16) -> Result<u8, EmulationError> {
        match addr {
            // Status register
            0x4015 => Ok(self.reg_status),
            // The rest of the registers cannot be read from
            _ => Err(EmulationError::MemoryAccess(format!("Attempted invalid read from APU register: 0x{:04X}", addr)))
        }
    }
        
    fn write(&mut self, addr: u16, byte: u8) -> Result<(), EmulationError> {
        match addr {
            // Pulse 1
            // Duty (DD), Envelope loop/Length counter Halt (LC), constant volume (C), volume/envelope (VVVV)
            0x4000 => { self.pulse1.write_ddlcvvvv(byte); Ok(()) },
            // Sweep unit: enabled (E), period (P), negate (N), shift (S)
            0x4001 => { self.pulse1.write_epppnsss(byte); Ok(()) },
            // Timer low  (T)
            0x4002 => { self.pulse1.write_tttttttt(byte); Ok(()) },
            // Length counter load (L), timer high (T)
            0x4003 => { self.pulse1.write_lllllttt(byte); Ok(()) },

            // Pulse2
            // Duty (DD), Envelope loop/Length counter Halt (LC), constant volume (C), volume/envelope (VVVV)
            0x4004 => { self.pulse2.write_ddlcvvvv(byte); Ok(()) },
            // Sweep unit: enabled (E), period (P), negate (N), shift (S)
            0x4005 => { self.pulse2.write_epppnsss(byte); Ok(()) },
            // Timer low  (T)
            0x4006 => { self.pulse2.write_tttttttt(byte); Ok(()) },
            // Length counter load (L), timer high (T)
            0x4007 => { self.pulse2.write_lllllttt(byte); Ok(()) },

            // Triangle
            // Length counter halt / linear counter control (C), linear counter load (R)
            0x4008 => { self.triangle.write_crrrrrrr(byte); Ok(()) },
            // Unused (U), but can still be written to and read from
            0x4009 => { self.triangle.write_uuuuuuuu(byte); Ok(()) },
            // Timer low (T)
            0x400A => { self.triangle.write_tttttttt(byte); Ok(()) },
            // Length counter load (L), timer high (T)
            0x400B => { self.triangle.write_lllllttt(byte); Ok(()) },

            // Noise
            // Unused (U), Envelope loop / length counter halt (L), constant volume (C), volume/envelope (V)
            0x400C => { self.noise.write_uulcvvvv(byte); Ok(()) },
            // Unused (U), but can still be written to
            0x400D => { self.noise.write_uuuuuuuu(byte); Ok(()) },
            // Loop noise (L), unused (U), noise period (P)
            0x400E => { self.noise.write_luuupppp(byte); Ok(()) },
            // Length counter load (L), unused (U)
            0x400F => { self.noise.write_llllluuu(byte); Ok(()) },

            // DMC
            // IRQ enable (I), loop (L), unused (U), frequency (R)
            0x4010 => { self.dmc.write_iluurrrr(byte); Ok(()) },
            // Unused (U), load counter (D)
            0x4011 => { self.dmc.write_udddddddd(byte); Ok(()) },
            // Sample address (A)
            0x4012 => { self.dmc.write_aaaaaaaa(byte); Ok(()) },
            // Sample length (L)
            0x4013 => { self.dmc.write_llllllll(byte); Ok(()) },
            //
            // 0x4014 is skipped, it's not part of the APU,
            // but rather the OMA DMA register
            //

            // Status register
            0x4015 => { self.write_status(byte); Ok(()) },

            // Frame counter
            // This register is used for both APU and I/O manipulation
            // The APU only uses bits 6 and 7
            0x4017 => { self.write_frame_counter(byte); Ok(()) },

            _ => unreachable!()
        }
    }
}
