// const SAMPLE_RATE: u32 = 8000;
use std::cell::Cell;
use core::memory::MemMapped;
use core::errors::EmulationError;

// Actually it's (super::MASTER_CLOCK_NTSC / super::CLOCK_DIVISOR_NTSC) but
// we need something divisible by 240
const APU_SAMPLE_RATE: u32 = 1_789_920;
const OUTPUT_SAMPLE_RATE: u32 = 44_100;
const STEP_FREQUENCY: u32 = 240; // 240hz steps
const SAMPLES_PER_STEP: u32 = APU_SAMPLE_RATE / STEP_FREQUENCY;
const CPU_CYCLES_PER_STEP: u64 = 7_457; // 7457.5 so we will add 1 on odd cpu cycles
const APU_CYCLES_PER_STEP: u64 = 3_728; // 3728.5

const NUM_CHANNELS: usize = 5;
const PULSE_1: usize = 0;
const PULSE_2: usize = 1;
const TRIANGLE: usize = 2;
const NOISE: usize = 3;
const DMC: usize = 4;

// Length counter lookup table
const LC_LOOKUP_TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6,
    160, 8, 60, 10, 14, 12, 26, 14,
    12, 16, 24, 18, 48, 20, 96, 22,
    192, 24, 72, 26, 16, 28, 32, 30];

// Pulse waveform table
const PULSE_DUTY: [[u8; 8]; 4] = [
    [0, 1, 0, 0, 0, 0, 0, 0],
    [0, 1, 1, 0, 0, 0, 0, 0],
    [0, 1, 1, 1, 1, 0, 0, 0],
    [1, 0, 0, 1, 1, 1, 1, 1]];


trait ApuChannel {
    fn write_reg(&mut self, reg_index: usize, byte: u8);

    fn is_enabled(&self) -> bool;
    fn toggle_enabled(&mut self, enabled: bool);

    fn clock_timer(&mut self);
    fn clock_length_counter(&mut self);

    fn generate_samples(&self) -> Vec<u8>;
}

//
// Pulse channels
//

#[derive(Debug, Default)]
struct Pulse {
    enabled: bool,
    // Duty for current APU frame
    duty: u8,
    waveform_counter: usize,
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
    timer_counter: u16,
    // Length counter is 5-bit (bits 5-7 are disregarded)
    lc_halt_env_loop: bool,
    length_counter: u8,
}

impl Pulse {
    fn write_ddlcvvvv(&mut self, byte: u8) {
        self.duty = byte >> 6;
        self.lc_halt_env_loop = byte & 0b0010_0000 != 0;
        self.constant_volume = byte & 0b0001_0000 != 0;
        self.volume = byte & 0b1111;
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
        self.length_counter = if self.enabled {
            LC_LOOKUP_TABLE[length_counter_index]
        } else {
            0
        };

        println!("{}: {} (enabled: {})", length_counter_index, self.length_counter, self.enabled);
    }
}

impl ApuChannel for Pulse {
    fn write_reg(&mut self, reg_index: usize, byte: u8) {
        match reg_index {
            0 => self.write_ddlcvvvv(byte),
            1 => self.write_epppnsss(byte),
            2 => self.write_tttttttt(byte),
            3 => self.write_lllllttt(byte),
            _ => unreachable!(),
        }
    }

    fn is_enabled(&self) -> bool {
        return self.enabled && self.length_counter > 0;
    }

    fn toggle_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;

        if !enabled {
            self.length_counter = 0;
        }
    }

    fn clock_timer(&mut self) {
        if self.timer_counter == 0 {
            self.timer_counter = self.timer + 1;

            if self.waveform_counter == 0 {
                self.waveform_counter == 7;
            } else {
                self.waveform_counter -= 1;
            }
        } else {
            self.timer_counter -= 1;
        }
    }

    fn clock_length_counter(&mut self) {
        if self.is_enabled() && !self.lc_halt_env_loop {
            self.length_counter -= 1;
        }
    }

    fn generate_samples(&self) -> Vec<u8> {
        let sample_count = SAMPLES_PER_STEP;
        let period_in_cycles = 8 * (self.timer as u32 + 1);
        let periods_in_step: f32 = APU_CYCLES_PER_STEP as f32 / period_in_cycles as f32;

        //        println!("Pulse: {}, {}", periods_in_step, SAMPLES_PER_STEP);
        let mut samples: Vec<u8> = Vec::new();

        for _i in 0..sample_count {
            samples.push(0);
        }
        //        if self.enabled && self.length_counter > 0 {
        //            self.volume
        //        } else {
        //            0
        //        }

        samples
    }
}

//
// Triangle channel
//

#[derive(Debug, Default)]
struct Triangle {
    enabled: bool,

    waveform_counter: usize,

    lengthc_halt_linearc_control: bool,
    linear_counter_load: u8,

    timer: u16,
    timer_counter: u16,

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
        self.length_counter = if self.enabled {
            LC_LOOKUP_TABLE[length_counter_index]
        } else {
            0
        };
    }
}

impl ApuChannel for Triangle {
    fn write_reg(&mut self, reg_index: usize, byte: u8) {
        match reg_index {
            0 => self.write_crrrrrrr(byte),
            1 => self.write_uuuuuuuu(byte),
            2 => self.write_tttttttt(byte),
            3 => self.write_lllllttt(byte),
            _ => unreachable!()
        }
    }

    fn is_enabled(&self) -> bool {
        return self.enabled && self.length_counter > 0;
    }

    fn toggle_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;

        if !enabled {
            self.length_counter = 0;
        }
    }

    fn clock_timer(&mut self) {
        if self.timer_counter == 0 {
            self.timer_counter = self.timer + 1;

            if self.waveform_counter == 0 {
                self.waveform_counter == 7;
            } else {
                self.waveform_counter -= 1;
            }
        } else {
            self.timer_counter -= 1;
        }
    }

    fn clock_length_counter(&mut self) {
        if self.is_enabled() && !self.lengthc_halt_linearc_control {
            self.length_counter -= 1;
        }
    }

    fn generate_samples(&self) -> Vec<u8> {
        let sample_count = SAMPLES_PER_STEP;
        let period_in_cycles = 8 * (self.timer as u32 + 1);
        let periods_in_step: f32 = APU_CYCLES_PER_STEP as f32 / period_in_cycles as f32;

        //        println!("Triangle: {}", periods_in_step);
        let mut samples: Vec<u8> = Vec::new();

        for _i in 0..sample_count {
            samples.push(0);
        }
        //        if self.enabled && self.length_counter > 0 {
        //            self.volume
        //        } else {
        //            0
        //        }

        samples
    }
}

//
// Noise channel
//

#[derive(Debug, Default)]
struct Noise {
    enabled: bool,

    volume: u8,
    lc_halt_env_loop: bool,
    constant_volume: bool,
    looping: bool,
    period: u8,

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
        self.looping = byte & 0b1000_0000 != 0;
        self.period = byte & 0b1111;
    }

    fn write_llllluuu(&mut self, byte: u8) {
        let length_counter_index = (byte >> 3) as usize;
        self.length_counter = if self.enabled {
            LC_LOOKUP_TABLE[length_counter_index]
        } else {
            0
        };
    }
}

impl ApuChannel for Noise {
    fn write_reg(&mut self, reg_index: usize, byte: u8) {
        match reg_index {
            0 => self.write_uulcvvvv(byte),
            1 => self.write_uuuuuuuu(byte),
            2 => self.write_luuupppp(byte),
            3 => self.write_llllluuu(byte),
            _ => unreachable!()
        }
    }

    fn is_enabled(&self) -> bool {
        return self.enabled && self.length_counter > 0;
    }

    fn toggle_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;

        if !enabled {
            self.length_counter = 0;
        }
    }

    fn clock_timer(&mut self) {}

    fn clock_length_counter(&mut self) {
        if self.is_enabled() & !self.lc_halt_env_loop {
            self.length_counter -= 1;
        }
    }

    fn generate_samples(&self) -> Vec<u8> {
        let sample_count = SAMPLES_PER_STEP;

        let mut samples: Vec<u8> = Vec::new();

        for _i in 0..sample_count {
            samples.push(0);
        }
        //        if self.enabled && self.length_counter > 0 {
        //            self.volume
        //        } else {
        //            0
        //        }

        samples
    }
}

//
// Delta-Modulation Channel (DMC)
//

#[derive(Debug, Default)]
struct DMC {
    enabled: bool,

    irq_enable: bool,
    looping: bool,
    frequency: u8,
    load_counter: u8,
    sample_address: u8,
    sample_length: u8,
}

impl DMC {
    fn write_iluurrrr(&mut self, byte: u8) {
        self.irq_enable = byte & 0b1000_0000 != 0;
        self.looping = byte & 0b0100_0000 != 0;
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
    fn write_reg(&mut self, reg_index: usize, byte: u8) {
        match reg_index {
            0 => self.write_iluurrrr(byte),
            1 => self.write_udddddddd(byte),
            2 => self.write_aaaaaaaa(byte),
            3 => self.write_llllllll(byte),
            _ => unreachable!()
        }
    }

    fn is_enabled(&self) -> bool {
        return self.enabled;
    }

    fn toggle_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn clock_timer(&mut self) {}

    fn clock_length_counter(&mut self) {}

    fn generate_samples(&self) -> Vec<u8> {
        let sample_count = SAMPLES_PER_STEP;

        let mut samples: Vec<u8> = Vec::new();

        for _i in 0..sample_count {
            samples.push(0);
        }
        //        if self.enabled && self.length_counter > 0 {
        //            self.volume
        //        } else {
        //            0
        //        }

        samples
    }
}

//
// APU and Frame Counter implementation
//

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
enum FrameCounterMode {
    Mode4Step,
    Mode5Step,
}

impl FrameCounterMode {
    fn steps(&self) -> i32 {
        match *self {
            FrameCounterMode::Mode4Step => 4,
            FrameCounterMode::Mode5Step => 5,
        }
    }
}

impl Default for FrameCounterMode {
    fn default() -> FrameCounterMode {
        FrameCounterMode::Mode4Step
    }
}

#[derive(Default)]
struct FrameCounter {
    mode: FrameCounterMode,
    current_step: i32,
    frame_completed: bool,

    clock_envelope: bool,
    clock_sweep: bool,
    clock_linear_counter: bool,
    clock_length_counter: bool,
}

impl FrameCounter {
    fn new() -> FrameCounter {
        let mut frame_counter = FrameCounter::default();
        frame_counter.reset();

        frame_counter
    }

    fn reset(&mut self) {
        self.current_step = -1;
    }

    fn set_mode(&mut self, mode: FrameCounterMode) {
        self.mode = mode;
    }

    fn step(&mut self) -> bool {
        let steps = self.mode.steps();
        self.current_step = (self.current_step + 1) % steps;

        // Step 5 in 5-step mode essentially does nothing
        // it is only there to alter timing
        if self.current_step != 4 {
            if self.current_step % 2 == 0 {
                self.clock_envelope = true;
                self.clock_sweep = false;
                self.clock_linear_counter = true;
                self.clock_length_counter = false;
            } else {
                self.clock_envelope = true;
                self.clock_sweep = true;
                self.clock_linear_counter = true;
                self.clock_length_counter = true;
            }
        }

        let final_step = self.current_step == steps - 1;
        self.frame_completed = final_step;

        let irq = self.mode == FrameCounterMode::Mode4Step && final_step;

        irq
    }
}

pub struct Apu {
    // Waveform/Sample generators
    channels: [Box<ApuChannel>; 5],

    // Mixer
    pulse_table: [u8; 31],
    tnd_table: [u8; 203],

    // Status register
    // Enable DMC (D), noise (N), triangle (T), and pulse channels (p2/p1)

    // Frame counter
    // Mode (M, 0 = 4-step, 1 = 5-step), IRQ inhibit flag (I), unused (U)
    frame_counter: FrameCounter,
    frame_counter_new_mode: bool,

    irq_inhibit: bool,
    frame_irq: Cell<bool>,
    dmc_irq: bool,

    apu_cycles: u64,
    remaining_cycles: u64,

    channel_samples: Vec<Vec<u8>>,
    nes_samples: Vec<u8>,
    pub out_samples: Vec<u8>,
}

impl Default for Apu {
    fn default() -> Apu {
        let channels = [
            Box::new(Pulse::default()) as Box<ApuChannel>,
            Box::new(Pulse::default()) as Box<ApuChannel>,
            Box::new(Triangle::default()) as Box<ApuChannel>,
            Box::new(Noise::default()) as Box<ApuChannel>,
            Box::new(DMC::default()) as Box<ApuChannel>
        ];

        let mut channel_samples = Vec::with_capacity(NUM_CHANNELS);
        for _i in 0..NUM_CHANNELS {
            channel_samples.push(Vec::new());
        }

        Apu {
            channels: channels,

            pulse_table: [0; 31],
            tnd_table: [0; 203],

            frame_counter: FrameCounter::default(),
            frame_counter_new_mode: false,

            irq_inhibit: false,
            frame_irq: Cell::new(false),
            dmc_irq: false,

            apu_cycles: 0,
            remaining_cycles: 0,

            channel_samples: channel_samples,
            nes_samples: Vec::new(),
            out_samples: Vec::new(),
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

        let mut apu = Apu::default();
        apu.pulse_table = pulse_table;
        apu.tnd_table = tnd_table;
        apu.frame_counter.reset();

        apu
    }

    fn read_status(&self) -> u8 {
        let pulse1_enabled = self.channels[PULSE_1].is_enabled();
        let pulse2_enabled = self.channels[PULSE_2].is_enabled();
        let triangle_enabled = self.channels[TRIANGLE].is_enabled();
        let noise_enabled = self.channels[NOISE].is_enabled();
        let dmc_enabled = self.channels[DMC].is_enabled();

        let frame_irq = self.frame_irq.get();
        let dmc_irq = self.dmc_irq;

        let mut byte: u8 = 0;

        // TODO DMC bytes remaining
        byte = byte | dmc_irq as u8;
        byte = (byte << 1) | frame_irq as u8;
        byte = (byte << 1) | 0; // unused
        byte = (byte << 1) | dmc_enabled as u8;
        byte = (byte << 1) | noise_enabled as u8;
        byte = (byte << 1) | triangle_enabled as u8; // unused
        byte = (byte << 1) | pulse2_enabled as u8;
        byte = (byte << 1) | pulse1_enabled as u8;

        println!("Status read: {:08b}", byte);

        byte
    }

    fn write_status(&mut self, byte: u8) {
        let dmc_enabled = (byte >> 4) & 0b1 != 0;
        let noise_enabled = (byte >> 3) & 0b1 != 0;
        let triangle_enabled = (byte >> 2) & 0b1 != 0;
        let pulse2_enabled = (byte >> 1) & 0b1 != 0;
        let pulse1_enabled = byte & 0b1 != 0;

        self.channels[PULSE_1].toggle_enabled(pulse1_enabled);
        self.channels[PULSE_2].toggle_enabled(pulse2_enabled);
        self.channels[TRIANGLE].toggle_enabled(triangle_enabled);
        self.channels[NOISE].toggle_enabled(noise_enabled);
        self.channels[DMC].toggle_enabled(dmc_enabled);

        println!("Status write: {:08b}", byte);
    }

    fn write_frame_counter(&mut self, byte: u8) {
        let frame_counter_mode = byte >> 7;
        let frame_counter_mode = match frame_counter_mode {
            0 => FrameCounterMode::Mode4Step,
            1 => FrameCounterMode::Mode5Step,
            _ => unreachable!()
        };

        let irq_inhibit = (byte >> 6) & 0b1 != 0;
        self.irq_inhibit = irq_inhibit;

        if irq_inhibit {
            self.frame_irq.set(false);
        }

        self.frame_counter.set_mode(frame_counter_mode);

        self.frame_counter.reset();
        if self.frame_counter.mode == FrameCounterMode::Mode5Step {
            self.frame_counter.current_step = 0;
            self.step_frame_counter();
        }

        self.frame_counter_new_mode = true;
        //println!("Frame sequencer write: {:08b}", byte);
    }

    fn clear_channel_samples(&mut self) {
        for i in 0..NUM_CHANNELS {
            self.channel_samples[i as usize].clear();
        }
    }

    fn generate_samples(&mut self) {
        for i in 0..NUM_CHANNELS {
            let mut generated_samples = self.channels[i as usize].generate_samples();

            self.channel_samples[i as usize].append(&mut generated_samples);
        }

        self.mix();
        self.clear_channel_samples();
    }

    fn mix(&mut self) {
        // All channel sample buffers have same sizes
        let buffer_len = self.channel_samples[0].len();

        for i in 0..buffer_len {
            // We add outputs of pulse1 and pulse 2 channels
            // and use that value as an index into the pulse output lookup table
            let pulse_output_index: usize
            = self.channel_samples[PULSE_1][i] as usize + self.channel_samples[PULSE_2][i] as usize;

            // We use outputs of triangle, noise and DMC channels
            // as an index into the tnd output lookup table
            let tnd_output_index: usize
            = 3 * self.channel_samples[TRIANGLE][i] as usize + 2 * self.channel_samples[NOISE][i] as usize
            + self.channel_samples[DMC][i] as usize;

            let pulse_output = self.pulse_table[pulse_output_index];
            let tnd_output = self.tnd_table[tnd_output_index];

            let output: u8 = pulse_output.wrapping_add(tnd_output);

            self.nes_samples.push(output);
        }
    }

    fn step_timers(&mut self) {
        for i in 0..NUM_CHANNELS {
            self.channels[i].clock_timer();
        }
    }

    fn step_frame_counter(&mut self) {
        let irq = self.frame_counter.step();

        for channel in &mut self.channels {
            if self.frame_counter.clock_envelope {
                //
            }
            if self.frame_counter.clock_sweep {
                //
            }
            if self.frame_counter.clock_linear_counter {
                //
            }
            if self.frame_counter.clock_length_counter {
                channel.clock_length_counter();
            }
        }

        self.generate_samples();

        if irq && !self.irq_inhibit {
            self.frame_irq.set(true);
        }
    }

    pub fn step(&mut self, cpu_cycles: u64) -> bool {
        let odd_cycle = cpu_cycles % 2 != 0;
        let target_cycles = self.remaining_cycles + cpu_cycles / 2;

        if self.apu_cycles > target_cycles {
            return false;
        }

        let cycles_to_run: u64 = target_cycles - self.apu_cycles;
        let mut apu_cycles_per_step = APU_CYCLES_PER_STEP;
        if odd_cycle {
            apu_cycles_per_step += 1;
        }

        let frame_steps_to_run = cycles_to_run / apu_cycles_per_step;

        let remaining_cycles = cycles_to_run % apu_cycles_per_step;
        for i in 0..frame_steps_to_run {
            self.step_timers();
            self.step_frame_counter();

            if self.frame_counter.frame_completed {
                // output
            }
        }
        //
        self.apu_cycles += frame_steps_to_run * APU_CYCLES_PER_STEP;
        self.remaining_cycles = remaining_cycles;

        let irq = self.frame_irq.get() && !self.irq_inhibit;

        irq
    }
}

impl MemMapped for Apu {
    fn read(&self, addr: u16) -> Result<u8, EmulationError> {
        match addr {
            // Status register
            0x4015 => {
                let status = self.read_status();
                // Clear frame_irq on read
                self.frame_irq.set(false);
                println!("{}", self.frame_irq.get());
                Ok(status)
            },
            // The rest of the registers cannot be read from
            _ => Err(EmulationError::MemoryAccess(format!("Attempted invalid read from APU register: 0x{:04X}", addr)))
        }
    }

    fn write(&mut self, addr: u16, byte: u8) -> Result<(), EmulationError> {
        match addr {
            // Pulse 1
            // Duty (DD), Envelope loop/Length counter Halt (LC), constant volume (C), volume/envelope (VVVV)
            0x4000 => {
                self.channels[PULSE_1].write_reg(0, byte);
                Ok(())
            },
            // Sweep unit: enabled (E), period (P), negate (N), shift (S)
            0x4001 => {
                self.channels[PULSE_1].write_reg(1, byte);
                Ok(())
            },
            // Timer low  (T)
            0x4002 => {
                self.channels[PULSE_1].write_reg(2, byte);
                Ok(())
            },
            // Length counter load (L), timer high (T)
            0x4003 => {
                self.channels[PULSE_1].write_reg(3, byte);
                Ok(())
            },

            // Pulse2
            // Duty (DD), Envelope loop/Length counter Halt (LC), constant volume (C), volume/envelope (VVVV)
            0x4004 => {
                self.channels[PULSE_2].write_reg(0, byte);
                Ok(())
            },
            // Sweep unit: enabled (E), period (P), negate (N), shift (S)
            0x4005 => {
                self.channels[PULSE_2].write_reg(1, byte);
                Ok(())
            },
            // Timer low  (T)
            0x4006 => {
                self.channels[PULSE_2].write_reg(2, byte);
                Ok(())
            },
            // Length counter load (L), timer high (T)
            0x4007 => {
                self.channels[PULSE_2].write_reg(3, byte);
                Ok(())
            },

            // Triangle
            // Length counter halt / linear counter control (C), linear counter load (R)
            0x4008 => {
                self.channels[TRIANGLE].write_reg(0, byte);
                Ok(())
            },
            // Unused (U), but can still be written to and read from
            0x4009 => {
                self.channels[TRIANGLE].write_reg(1, byte);
                Ok(())
            },
            // Timer low (T)
            0x400A => {
                self.channels[TRIANGLE].write_reg(2, byte);
                Ok(())
            },
            // Length counter load (L), timer high (T)
            0x400B => {
                self.channels[TRIANGLE].write_reg(3, byte);
                Ok(())
            },

            // Noise
            // Unused (U), Envelope loop / length counter halt (L), constant volume (C), volume/envelope (V)
            0x400C => {
                self.channels[NOISE].write_reg(0, byte);
                Ok(())
            },
            // Unused (U), but can still be written to
            0x400D => {
                self.channels[NOISE].write_reg(1, byte);
                Ok(())
            },
            // Loop noise (L), unused (U), noise period (P)
            0x400E => {
                self.channels[NOISE].write_reg(2, byte);
                Ok(())
            },
            // Length counter load (L), unused (U)
            0x400F => {
                self.channels[NOISE].write_reg(3, byte);
                Ok(())
            },

            // DMC
            // IRQ enable (I), loop (L), unused (U), frequency (R)
            0x4010 => {
                self.channels[DMC].write_reg(0, byte);
                Ok(())
            },
            // Unused (U), load counter (D)
            0x4011 => {
                self.channels[DMC].write_reg(1, byte);
                Ok(())
            },
            // Sample address (A)
            0x4012 => {
                self.channels[DMC].write_reg(2, byte);
                Ok(())
            },
            // Sample length (L)
            0x4013 => {
                self.channels[DMC].write_reg(3, byte);
                Ok(())
            },
            //
            // 0x4014 is skipped, it's not part of the APU,
            // but rather the OMA DMA register
            //

            // Status register
            0x4015 => {
                self.write_status(byte);
                Ok(())
            },

            // Frame counter
            // This register is used for both APU and I/O manipulation
            // The APU only uses bits 6 and 7
            0x4017 => {
                self.write_frame_counter(byte);
                Ok(())
            },

            _ => unreachable!()
        }
    }
}
