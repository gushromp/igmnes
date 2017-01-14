// const SAMPLE_RATE: u32 = 8000;
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
    // Duty (DD), Envelope loop/Length counter Halt (LC), constant volume (C), volume/envelope (VVVV)
    reg_ddlcvvvv: u8,
    // Sweep unit: enabled (E), period (P), negate (N), shift (S)
    reg_epppnsss: u8,
    // Timer low  (T)
    reg_tttttttt: u8,
    // Length counter load (L), timer high (T)
    reg_lllllttt: u8,

    // Duty for current APU frame
    pub duty: u8,
    // Volume for current APU frame
    pub constant_volume: bool,
    pub volume: u8,
    // Sweep:
    pub sweep_enabled: bool,
    pub sweep_period: u8,
    pub sweep_negate: bool,
    pub sweep_shift: u8,
    // Timer is 11-bit (bits 11-15 are disregarded)
    pub timer: u16,
    // Length counter is 5-bit (bits 5-7 are disregarded)
    pub lc_halt_env_loop: bool,
    pub length_counter: u8,
}

impl Pulse {
    fn write_ddlcvvvv(&mut self, byte: u8) {
        self.duty = byte >> 6;
        self.constant_volume = byte & 0b0001_0000 != 0;
        self.volume = byte & 0b1111;
        self.lc_halt_env_loop = byte & 0b0010_0000 != 0;

        self.reg_ddlcvvvv = byte;
    }

    fn write_epppnsss(&mut self, byte: u8) {
        self.sweep_enabled = byte & 0b1000_0000 != 0;
        self.sweep_period = (byte >> 4) & 0b111;
        self.sweep_negate = byte & 0b0000_1000 != 0;
        self.sweep_shift = byte & 0b111;

        self.reg_epppnsss;
    }

    fn write_tttttttt(&mut self, byte: u8) {
        let timer_high = self.timer >> 8;
        let timer_low = byte as u16;
        self.timer = (timer_high << 8) | timer_low;

        self.reg_tttttttt = byte;
    }

    fn write_lllllttt(&mut self, byte: u8) {
        let timer_high = (byte & 0b111) as u16;
        self.timer = (timer_high << 8) | self.reg_tttttttt as u16;

        let length_counter_index = (byte >> 3) as usize;
        self.length_counter = LC_LOOKUP_TABLE[length_counter_index];

        self.reg_lllllttt = byte;
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
    // Length counter halt / linear counter control (C), linear counter load (R)
    reg_crrrrrrr: u8,
    // Unused (U), but can still be written to and read from
    reg_uuuuuuuu: u8,
    // Timer low (T)
    reg_tttttttt: u8,
    // Length counter load (L), timer high (T)
    reg_lllllttt: u8,

    pub lengthc_halt_linearc_control: bool,
    pub linear_counter_load: u8,

    pub timer: u16,
    pub length_counter: u8,
}

impl Triangle {
    fn write_crrrrrrr(&mut self, byte: u8) {
        self.lengthc_halt_linearc_control = byte & 0b1000_0000 != 0;
        self.linear_counter_load = byte & 0b0111_1111;

        self.reg_crrrrrrr = byte;
    }

    fn write_uuuuuuuu(&mut self, byte: u8) {
        self.reg_uuuuuuuu = byte;
    }

    fn write_tttttttt(&mut self, byte: u8) {
        let timer_high = self.timer >> 8;
        let timer_low = byte as u16;
        self.timer = (timer_high << 8) | timer_low;

        self.reg_tttttttt = byte;
    }

    fn write_lllllttt(&mut self, byte: u8) {
        let timer_high = byte & 0b111;
        self.timer = ((timer_high as u16) << 8) | self.reg_tttttttt as u16;

        let length_counter_index = (byte >> 3) as usize;
        self.length_counter = LC_LOOKUP_TABLE[length_counter_index];

        self.reg_lllllttt = byte;
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
    // Unused (U), Envelope loop / length counter halt (L), constant volume (C), volume/envelope (V)
    reg_uulcvvvv: u8,
    // Unused (U), but can still be written to and read from
    reg_uuuuuuuu: u8,
    // Loop noise (L), unused (U), noise period (P)
    reg_luuupppp: u8,
    // Length counter load (L), unused (U)
    reg_llllluuu: u8,

    pub volume: u8,
    pub lc_halt_env_loop: bool,
    pub constant_volume: bool,
    pub noise_loop: bool,
    pub noise_period: u8,

    pub length_counter: u8,
}

impl Noise {
    fn write_uulcvvvv(&mut self, byte: u8) {
        self.lc_halt_env_loop = byte & 0b0010_0000 != 0;
        self.constant_volume = byte & 0b0001_0000 != 0;
        self.volume = byte & 0b1111;

        self.reg_uulcvvvv = byte;
    }

    fn write_uuuuuuuu(&mut self, byte: u8) {
        self.reg_uuuuuuuu = byte;
    }

    fn write_luuupppp(&mut self, byte: u8) {
        self.noise_loop = byte & 0b1000_0000 != 0;
        self.noise_period = byte & 0b1111;

        self.reg_luuupppp = byte;
    }

    fn write_llllluuu(&mut self, byte: u8) {
        let length_counter_index = (byte >> 3) as usize;
        self.length_counter = LC_LOOKUP_TABLE[length_counter_index];

        self.reg_llllluuu = byte;
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
    // IRQ enable (I), loop (L), unused (U), frequency (R)
    reg_iluurrrr: u8,
    // Unused (U), load counter (D)
    reg_uddddddd: u8,
    // Sample address (A)
    reg_aaaaaaaa: u8,
    // Sample length (L)
    reg_llllllll: u8,

    pub irq_enable: bool,
    pub dmc_loop: bool,
    pub frequency: u8,
    pub load_counter: u8,
    pub sample_address: u8,
    pub sample_length: u8,
}

impl DMC {
    fn write_iluurrrr(&mut self, byte: u8) {
        self.irq_enable = byte & 0b1000_0000 != 0;
        self.dmc_loop = byte & 0b0100_0000 != 0;
        self.frequency = byte & 0b1111;

        self.reg_iluurrrr = byte;
    }

    fn write_udddddddd(&mut self, byte: u8) {
        self.load_counter = byte & 0b0111_1111;

        self.reg_uddddddd = byte;
    }

    fn write_aaaaaaaa(&mut self, byte: u8) {
        self.sample_address = byte;

        self.reg_aaaaaaaa = byte;
    }

    fn write_llllllll(&mut self, byte: u8) {
        self.sample_length = byte;

        self.reg_llllllll = byte;
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
    reg_uuudntpp: u8,
    // Frame counter
    // Mode (M, 0 = 4-step, 1 = 5-step), IRQ inhibit flag (I), unused (U)
    reg_miuuuuuu: u8,
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

            reg_uuudntpp: 0,
            reg_miuuuuuu: 0,
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
            pulse_table[n] = pulse_n as u8
        }

        tnd_table[0] = 0;
        for n in 1..203 {
            let tnd_n: f32 = 163.67 / (24329.0 / n as f32 + 100.0);
            tnd_table[n] = tnd_n as u8;
        }

        Apu {
            pulse1: Pulse::default(),
            pulse2: Pulse::default(),
            triangle: Triangle::default(),
            noise: Noise::default(),
            dmc: DMC::default(),

            pulse_table: pulse_table,
            tnd_table: tnd_table,

            reg_uuudntpp: 0,
            reg_miuuuuuu: 0,
        }
    }

    pub fn mix(&self) -> u8 {
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
}

impl MemMapped for Apu {
    fn read(&self, addr: u16) -> Result<u8, EmulationError> {
        match addr {
            // Pulse 1
            0x4000 => Ok(self.pulse1.reg_ddlcvvvv),
            0x4001 => Ok(self.pulse1.reg_epppnsss),
            0x4002 => Ok(self.pulse1.reg_tttttttt),
            0x4003 => Ok(self.pulse1.reg_lllllttt),
            // Pulse2
            0x4004 => Ok(self.pulse2.reg_ddlcvvvv),
            0x4005 => Ok(self.pulse2.reg_epppnsss),
            0x4006 => Ok(self.pulse2.reg_tttttttt),
            0x4007 => Ok(self.pulse2.reg_lllllttt),
            // Triangle
            0x4008 => Ok(self.triangle.reg_crrrrrrr),
            0x4009 => Ok(self.triangle.reg_uuuuuuuu),
            0x400A => Ok(self.triangle.reg_tttttttt),
            0x400B => Ok(self.triangle.reg_lllllttt),
            // Noise
            0x400C => Ok(self.noise.reg_uulcvvvv),
            0x400D => Ok(self.noise.reg_uuuuuuuu),
            0x400E => Ok(self.noise.reg_luuupppp),
            0x400F => Ok(self.noise.reg_llllluuu),
            // DMC
            0x4010 => Ok(self.dmc.reg_iluurrrr),
            0x4011 => Ok(self.dmc.reg_uddddddd),
            0x4012 => Ok(self.dmc.reg_aaaaaaaa),
            0x4013 => Ok(self.dmc.reg_llllllll),
            //
            // 0x4014 is skipped, it's not part of the APU,
            // but rather the OMA DMA register
            //
            // Status register
            0x4015 => Ok(self.reg_uuudntpp),
            // Frame counter
            // This register is used for both APU and I/O manipulation
            // The APU only uses bits 6 and 7
            0x4017 => Ok(self.reg_miuuuuuu),

            _ => unreachable!()
        }
    }
        
    fn write(&mut self, addr: u16, byte: u8) -> Result<(), EmulationError> {
        match addr {
            0x4000 => { self.pulse1.write_ddlcvvvv(byte); Ok(()) },
            0x4001 => { self.pulse1.write_epppnsss(byte); Ok(()) },
            0x4002 => { self.pulse1.write_tttttttt(byte); Ok(()) },
            0x4003 => { self.pulse1.write_lllllttt(byte); Ok(()) },
            // Pulse2
            0x4004 => { self.pulse2.write_ddlcvvvv(byte); Ok(()) },
            0x4005 => { self.pulse2.write_epppnsss(byte); Ok(()) },
            0x4006 => { self.pulse2.write_tttttttt(byte); Ok(()) },
            0x4007 => { self.pulse2.write_lllllttt(byte); Ok(()) },
            // Triangle
            0x4008 => { self.triangle.write_crrrrrrr(byte); Ok(()) },
            0x4009 => { self.triangle.write_uuuuuuuu(byte); Ok(()) },
            0x400A => { self.triangle.write_tttttttt(byte); Ok(()) },
            0x400B => { self.triangle.write_lllllttt(byte); Ok(()) },
            // Noise
            0x400C => { self.noise.write_uulcvvvv(byte); Ok(()) },
            0x400D => { self.noise.write_uuuuuuuu(byte); Ok(()) },
            0x400E => { self.noise.write_luuupppp(byte); Ok(()) },
            0x400F => { self.noise.write_llllluuu(byte); Ok(()) },
            // DMC
            0x4010 => { self.dmc.write_iluurrrr(byte); Ok(()) },
            0x4011 => { self.dmc.write_udddddddd(byte); Ok(()) },
            0x4012 => { self.dmc.write_aaaaaaaa(byte); Ok(()) },
            0x4013 => { self.dmc.write_llllllll(byte); Ok(()) },
            //
            // 0x4014 is skipped, it's not part of the APU,
            // but rather the OMA DMA register
            //
            // Status register
            0x4015 => { self.reg_uuudntpp = byte; Ok(()) },
            // Frame counter
            // This register is used for both APU and I/O manipulation
            // The APU only uses bits 6 and 7
            0x4017 => { self.reg_miuuuuuu = byte; Ok(()) },

            _ => unreachable!()
        }
    }
}
