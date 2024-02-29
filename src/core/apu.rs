use core::memory::MemMapped;
use core::errors::EmulationError;

// Actually it's (super::MASTER_CLOCK_NTSC / super::CLOCK_DIVISOR_NTSC) but
// we need something divisible by 240
// const APU_SAMPLE_RATE: usize = 1_789_773;
// const APU_SAMPLE_RATE: usize = 1_776_000;
// const APU_SAMPLE_RATE: usize = 1_719_900;

const OUTPUT_SAMPLE_RATE: usize = 44_100;

const SAMPLE_RATE_REMAINDER: f32 = 0.5844217687;

// const SAMPLE_AVERAGE_COUNT: usize = 4;
// const SAMPLE_RATE_RATIO: usize = (APU_SAMPLE_RATE / (OUTPUT_SAMPLE_RATE * SAMPLE_AVERAGE_COUNT)) + 1;

const FC_4STEP_CYCLE_TABLE_NTSC: &'static [u64; 4] = &[7457, 14913, 22371, 29829];
const FC_5STEP_CYCLE_TABLE_NTSC: &'static [u64; 4] = &[7457, 14913, 22371, 37281];
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


// Triangle waveform table
const TRIANGLE_WAVEFORM: [u8; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

// Noise period table
const NOISE_PERIOD_CYCLES: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068];


const DELAY_CYCLES_PER_IRQ_WRITE: u64 = 29835;

trait ApuChannel {
    fn write_reg(&mut self, reg_index: usize, byte: u8);

    fn is_enabled(&self) -> bool;
    fn toggle_enabled(&mut self, enabled: bool);

    fn is_audible(&self) -> bool;

    fn clock_timer(&mut self);
    fn clock_length_counter(&mut self);

    fn clock_envelope(&mut self);

    fn clock_sweep(&mut self);

    fn set_muted(&mut self, is_muted: bool);

    fn output(&self) -> u8;
}

//
// APU Envelope
//
#[derive(Debug, Default, Clone, Copy)]
struct Envelope {
    start: bool,
    period: u8,
    decay: u8,
}

impl Envelope {
    fn clock(&mut self, volume: u8, env_loop: bool) {
        if self.start {
            self.start = false;
            self.period = volume;
            self.decay = 15;
        } else {
            if self.period == 0 {
                self.period = volume;
                if self.decay > 0 {
                    self.decay -= 1;
                } else if env_loop {
                    self.decay = 15;
                }
            } else {
                self.period -= 1;
            }
        }
    }
}

//
// APU Sweep
//
#[derive(Debug, Default, Clone, Copy)]
struct Sweep {
    enabled: bool,
    period: u8,
    negate: bool,
    is_twos_complement_negate: bool,
    shift: u8,

    divider: u8,
    reload_flag: bool,

    should_mute: bool,
    new_timer: u16,
}

impl Sweep {
    fn clock(&mut self) -> bool {
        let result = self.enabled && self.divider == 0 && self.shift > 0 && !self.should_mute;

        if self.divider == 0 || self.reload_flag {
            self.divider = self.period;
            self.reload_flag = false;
        } else {
            self.divider -= 1;
        }

        result
    }

    pub fn set_target_period(&mut self, timer: u16) {
        let change_amount = timer >> (self.shift as usize);
        let result = if self.negate {
            timer.saturating_sub(change_amount + (!self.is_twos_complement_negate as u16))
        } else {
            timer.saturating_add(change_amount)
        };

        self.new_timer = result;
        self.should_mute = timer < 8 || self.new_timer > 0x7FF;

    }
}

//
// Pulse channels
//

#[derive(Debug, Default, Clone, Copy)]
struct Pulse {
    enabled: bool,
    // Duty for current APU frame
    duty: u8,
    waveform_counter: usize,
    // Volume for current APU frame
    constant_volume: bool,
    volume: u8,
    // Envelope
    envelope: Envelope,
    // Sweep:
    sweep: Sweep,
    // Timer is 11-bit (bits 11-15 are disregarded)
    timer: u16,
    timer_counter: u16,
    // Length counter is 5-bit (bits 5-7 are disregarded)
    should_toggle_halt_lc: bool,
    lc_halt_env_loop: bool,
    length_counter: u8,

    is_muted: bool,
}

impl Pulse {
    fn new(is_sweep_twos_complement_negate: bool) -> Pulse {
        let mut pulse = Pulse::default();
        pulse.sweep.is_twos_complement_negate = is_sweep_twos_complement_negate;
        pulse
    }
    fn write_ddlcvvvv(&mut self, byte: u8) {
        self.duty = byte >> 6;

        self.constant_volume = byte & 0b0001_0000 != 0;
        self.volume = byte & 0b1111;

        let lc_halt_env_loop = byte & 0b0010_0000 != 0;

        // self.should_toggle_halt_lc = lc_halt_env_loop != self.lc_halt_env_loop;
        self.lc_halt_env_loop = lc_halt_env_loop;
    }

    fn write_epppnsss(&mut self, byte: u8) {
        self.sweep.enabled = byte & 0b1000_0000 != 0;
        self.sweep.period = (byte >> 4) & 0b111;
        self.sweep.negate = byte & 0b0000_1000 != 0;
        self.sweep.shift = byte & 0b111;
        self.sweep.reload_flag = true;
        self.sweep.set_target_period(self.timer);
    }

    fn write_tttttttt(&mut self, byte: u8) {
        let timer_high = self.timer >> 8;
        let timer_low = byte as u16;
        self.timer = (timer_high << 8) | timer_low;
        self.sweep.set_target_period(self.timer);
    }

    fn write_lllllttt(&mut self, byte: u8) {
        let timer_high = (byte & 0b111) as u16;
        let timer_low = self.timer & 0xFF;
        self.timer = (timer_high << 8) | timer_low;
        self.sweep.set_target_period(self.timer);

        self.timer_counter = self.timer;
        self.waveform_counter = 0;

        let length_counter_index = (byte >> 3) as usize;
        self.length_counter = if self.enabled {
            LC_LOOKUP_TABLE[length_counter_index]
        } else {
            0
        };

        self.envelope.start = true;
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

    fn is_audible(&self) -> bool {
        self.enabled && self.length_counter > 0 && !self.sweep.should_mute && !self.is_muted
    }

    fn clock_timer(&mut self) {
        if self.timer_counter == 0 {
            self.timer_counter = self.timer;

            if self.waveform_counter == 0 {
                self.waveform_counter = 7;
            } else {
                self.waveform_counter -= 1;
            }
        } else {
            self.timer_counter -= 1;
        }
    }

    fn clock_length_counter(&mut self) {
        if !self.enabled { return }
        if self.length_counter > 0 && !self.lc_halt_env_loop {
            self.length_counter -= 1;
        }
    }

    fn clock_envelope(&mut self) {
        self.envelope.clock(self.volume, self.lc_halt_env_loop);
    }

    fn clock_sweep(&mut self) {
        let should_set_timer = self.sweep.clock();

        if should_set_timer {
            self.timer = self.sweep.new_timer;
            self.sweep.set_target_period(self.timer);
        }
    }

    fn set_muted(&mut self, is_muted: bool) {
        self.is_muted = is_muted
    }

    fn output(&self) -> u8 {
        if self.is_audible() {
            let waveform = PULSE_DUTY[self.duty as usize][self.waveform_counter];
            let volume = if self.constant_volume {
                self.volume
            } else {
                self.envelope.decay
            };
            waveform * volume
        } else {
            0
        }
    }
}

//
// Triangle channel
//

#[derive(Debug, Default, Clone)]
struct Triangle {
    enabled: bool,

    waveform_counter: usize,

    lengthc_halt_linearc_control: bool,
    linear_counter_load: u8,
    should_load_linear_counter: bool,
    linear_counter: u8,

    timer: u16,
    timer_counter: u16,

    length_counter: u8,

    is_muted: bool
}

impl Triangle {
    fn write_crrrrrrr(&mut self, byte: u8) {
        self.lengthc_halt_linearc_control = byte & 0b1000_0000 != 0;
        self.linear_counter_load = byte & 0b0111_1111;
    }

    fn write_uuuuuuuu(&mut self, _byte: u8) {}

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
        self.should_load_linear_counter = true;
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

    fn is_audible(&self) -> bool {
        self.is_enabled() && self.linear_counter > 0 && !self.is_muted
    }

    fn clock_timer(&mut self) {
        if self.timer_counter == 0 {
            self.timer_counter = self.timer;

            if self.waveform_counter == 0 {
                self.waveform_counter = TRIANGLE_WAVEFORM.len() - 1;
            } else {
                self.waveform_counter -= 1;
            }
        } else {
            self.timer_counter -= 1;
        }
    }

    fn clock_length_counter(&mut self) {
        if !self.enabled { return; }
        if self.should_load_linear_counter {
            self.linear_counter = self.linear_counter_load;
            if !self.lengthc_halt_linearc_control {
                self.should_load_linear_counter = false;
            }
        } else {
            if self.linear_counter >= 2 {
                self.linear_counter -= 2;
            } else {
                self.linear_counter = 0;
            }
        }
        if self.length_counter > 0 && !self.lengthc_halt_linearc_control {
            self.length_counter -= 1;
        }
    }

    fn clock_envelope(&mut self) {
        // No envelope on this channel
    }

    fn clock_sweep(&mut self) {
        // No sweep on this channel
    }

    fn set_muted(&mut self, is_muted: bool) {
        self.is_muted = is_muted
    }

    fn output(&self) -> u8 {
        if self.is_audible() {
            let waveform = TRIANGLE_WAVEFORM[self.waveform_counter];
            waveform
        } else {
            0
        }
    }
}

//
// Noise channel
//

#[derive(Debug, Clone)]
struct NoiseShiftRegister {
    shift_register: u16,
}

impl Default for NoiseShiftRegister {
    fn default() -> Self {
        NoiseShiftRegister {
            shift_register: 0b1
        }
    }
}

#[derive(Debug, Default, Clone)]
struct Noise {
    enabled: bool,

    volume: u8,
    lc_halt_env_loop: bool,
    constant_volume: bool,
    // Envelope
    envelope: Envelope,
    looping: bool,
    period: u16,
    period_counter: u16,

    shift_register: NoiseShiftRegister,
    length_counter: u8,

    is_muted: bool,
}

impl Noise {
    fn write_uulcvvvv(&mut self, byte: u8) {
        self.lc_halt_env_loop = byte & 0b0010_0000 != 0;
        self.constant_volume = byte & 0b0001_0000 != 0;
        self.volume = byte & 0b1111;
    }

    fn write_uuuuuuuu(&mut self, _byte: u8) {}

    fn write_luuupppp(&mut self, byte: u8) {
        self.looping = byte & 0b1000_0000 != 0;
        let period_index: usize = (byte & 0b1111) as usize;
        self.period = NOISE_PERIOD_CYCLES[period_index];
    }

    fn write_llllluuu(&mut self, byte: u8) {
        let length_counter_index = (byte >> 3) as usize;
        self.length_counter = if self.enabled {
            LC_LOOKUP_TABLE[length_counter_index]
        } else {
            0
        };
        self.envelope.start = true;
    }

    fn clock_shift_register(&mut self) {
        let mut shift_register = self.shift_register.shift_register;
        let bit0 = shift_register & 0b1;
        let feedback = if self.looping {
            bit0 ^ ((shift_register >> 6) & 0b1)
        } else {
            bit0 ^ ((shift_register >> 1) & 0b1)
        };
        shift_register = shift_register >> 1;
        shift_register = shift_register | (feedback << 14);
        self.shift_register.shift_register = shift_register;
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

    fn is_audible(&self) -> bool {
        self.enabled && self.length_counter > 0 && (self.shift_register.shift_register & 0b1) as u8 == 0 && !self.is_muted
    }

    fn clock_timer(&mut self) {
        if self.period_counter == 0 {
            self.clock_shift_register();
            self.period_counter = self.period;
        } else {
            self.period_counter -= 2;
        }
    }

    fn clock_length_counter(&mut self) {
        if !self.enabled { return; }
        if self.length_counter > 0 && !self.lc_halt_env_loop {
            self.length_counter -= 1;
        }
    }

    fn clock_envelope(&mut self) {
        if !self.enabled { return; }
        self.envelope.clock(self.volume, self.lc_halt_env_loop);
    }

    fn clock_sweep(&mut self) {
        // No sweep on this channel
    }

    fn set_muted(&mut self, is_muted: bool) {
        self.is_muted = is_muted
    }

    fn output(&self) -> u8 {
        if self.is_audible() {
            if self.constant_volume {
                self.volume
            } else {
                self.envelope.decay
            }
        } else {
            0
        }
    }
}

//
// Delta-Modulation Channel (DMC)
//

#[derive(Debug, Default, Clone)]
struct DMC {
    enabled: bool,

    irq_enable: bool,
    looping: bool,
    frequency: u8,
    load_counter: u8,
    sample_address: u8,
    sample_length: u8,

    is_muted: bool,
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

    fn is_audible(&self) -> bool {
        false
    }

    fn clock_timer(&mut self) {}

    fn clock_length_counter(&mut self) {}

    fn clock_envelope(&mut self) {
        // No envelope on this channel
    }

    fn clock_sweep(&mut self) {
        // No sweep on this channel
    }

    fn set_muted(&mut self, is_muted: bool) {
        self.is_muted = is_muted
    }

    fn output(&self) -> u8 {
        0
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

impl Default for FrameCounterMode {
    fn default() -> FrameCounterMode {
        FrameCounterMode::Mode4Step
    }
}

#[derive(Default, Clone)]
struct FrameCounter {
    mode: FrameCounterMode,
    cycle_table: Vec<u64>,
    cycles: u64,
    delayed_reset: bool,
    reset_after_cycles: u64,
    cycles_since_interrupt: u64,
    odd_frame: bool,

    clock_envelope: bool,
    clock_sweep: bool,
    clock_linear_counter: bool,
    clock_length_counter: bool,
}

impl FrameCounter {
    fn new() -> FrameCounter {
        let mut frame_counter = FrameCounter::default();

        frame_counter.set_mode(FrameCounterMode::Mode4Step);
        frame_counter.cycles = 2;

        frame_counter
    }

    fn reset(&mut self) {
        self.cycles = 0;
    }

    fn set_mode(&mut self, mode: FrameCounterMode) {
        self.mode = mode;

        self.cycle_table = if mode == FrameCounterMode::Mode4Step {
            FC_4STEP_CYCLE_TABLE_NTSC.to_vec()
        } else {
            FC_5STEP_CYCLE_TABLE_NTSC.to_vec()
        }
    }

    fn quarter_frame(&self) -> bool {
        self.cycles == self.cycle_table[0]
            || self.cycles == self.cycle_table[1]
            || self.cycles == self.cycle_table[2]
            || self.cycles == self.cycle_table[3]
    }

    fn half_frame(&self) -> bool {
        self.cycles == self.cycle_table[1]
            || self.cycles == self.cycle_table[3]
    }

    fn irq(&self) -> bool {
        self.mode == FrameCounterMode::Mode4Step &&
            self.cycles == self.cycle_table[3] ||
            self.cycles == self.cycle_table[3] - 1 ||
            self.cycles == 0
    }
}

pub struct Apu {
    // Waveform/Sample generators
    channels: [Box<dyn ApuChannel>; 5],

    // Mixer
    pulse_table: [f32; 31],
    tnd_table: [f32; 203],

    // Status register
    // Enable DMC (D), noise (N), triangle (T), and pulse channels (p2/p1)

    // Frame counter
    // Mode (M, 0 = 4-step, 1 = 5-step), IRQ inhibit flag (I), unused (U)
    frame_counter: FrameCounter,

    irq_inhibit: bool,
    frame_irq: bool,
    dmc_irq: bool,

    cpu_cycles: u64,
    apu_cycles: f64,
    next_irq_cycles: u64,

    nes_samples: Vec<f32>,
    out_samples: Vec<f32>,
    sample_rate_current_remainder: f32,
}

impl Default for Apu {
    fn default() -> Apu {
        let channels = [
            Box::new(Pulse::new(false)) as Box<dyn ApuChannel>,
            Box::new(Pulse::new(true)) as Box<dyn ApuChannel>,
            Box::new(Triangle::default()) as Box<dyn ApuChannel>,
            Box::new(Noise::default()) as Box<dyn ApuChannel>,
            Box::new(DMC::default()) as Box<dyn ApuChannel>
        ];

        Apu {
            channels,

            pulse_table: [0.0; 31],
            tnd_table: [0.0; 203],

            frame_counter: FrameCounter::new(),

            irq_inhibit: false,
            frame_irq: false,
            dmc_irq: false,

            cpu_cycles: 0,
            apu_cycles: 0.0,
            next_irq_cycles: 0,

            nes_samples: Vec::new(),
            out_samples: Vec::new(),
            sample_rate_current_remainder: 0.0,
        }
    }
}

impl Clone for Apu {
    fn clone(&self) -> Self {
        unreachable!()
    }
}

impl Apu {
    pub fn new() -> Apu {
        let mut pulse_table: [f32; 31] = [0.0; 31];
        let mut tnd_table: [f32; 203] = [0.0; 203];

        // Avoid division by 0
        pulse_table[0] = 0.0;
        for n in 1..31 {
            let pulse_n: f32 = 95.52 / (8128.0 / n as f32 + 100.0);
            pulse_table[n] = pulse_n;
        }

        tnd_table[0] = 0.0;
        for n in 1..203 {
            let tnd_n: f32 = 163.67 / (24329.0 / n as f32 + 100.0);
            tnd_table[n] = tnd_n;
        }

        let mut apu = Apu::default();
        apu.pulse_table = pulse_table;
        apu.tnd_table = tnd_table;

        apu
    }

    pub fn is_output_ready(&self) -> bool {
        self.out_samples.len() >= OUTPUT_SAMPLE_RATE / 60
    }

    pub fn get_out_samples(&mut self) -> Vec<f32> {
        let samples = self.out_samples.clone();
        self.out_samples.clear();
        samples
    }

    fn read_status(&mut self) -> u8 {
        let pulse1_enabled = self.channels[PULSE_1].is_enabled();
        let pulse2_enabled = self.channels[PULSE_2].is_enabled();
        let triangle_enabled = self.channels[TRIANGLE].is_enabled();
        let noise_enabled = self.channels[NOISE].is_enabled();
        let dmc_enabled = self.channels[DMC].is_enabled();

        let frame_irq = self.frame_irq;
        let dmc_irq = self.dmc_irq;

        self.frame_irq = false;

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
            self.frame_irq = false;
        }
        self.next_irq_cycles = self.cpu_cycles + DELAY_CYCLES_PER_IRQ_WRITE;

        self.frame_counter.set_mode(frame_counter_mode);
        self.frame_counter.delayed_reset = true;
    }

    fn clock_channel_output(&mut self) {
        // We add outputs of pulse1 and pulse 2 channels
        // and use that value as an index into the pulse output lookup table
        let pulse_output_index: usize
            = self.channels[PULSE_1].output() as usize + self.channels[PULSE_2].output() as usize;

        // We use outputs of triangle, noise and DMC channels
        // as an index into the tnd output lookup table
        let tnd_output_index: usize
            = 3 * self.channels[TRIANGLE].output() as usize + 2 * self.channels[NOISE].output() as usize
            + self.channels[DMC].output() as usize;

        let pulse_output = self.pulse_table[pulse_output_index];
        let tnd_output = self.tnd_table[tnd_output_index];

        let output = pulse_output + tnd_output;

        self.nes_samples.push(output);
    }

    fn generate_output_samples(&mut self) {
        let target_samples = if self.sample_rate_current_remainder > 1.0 {
            self.sample_rate_current_remainder -= 1.0;
            42
        } else {
            41
        };
        if self.nes_samples.len() < target_samples { return; }
        self.sample_rate_current_remainder += SAMPLE_RATE_REMAINDER;

        let sum = self.nes_samples.iter().cloned().reduce(|a, b| a + b);
        if let Some(sum) = sum {
            let avg = sum / self.nes_samples.len() as f32;
            self.out_samples.push(avg);
        }

        self.nes_samples.clear();
    }

    fn clock_frame_counter(&mut self) {
        let cycles_per_frame = *self.frame_counter.cycle_table.last().unwrap();

        self.frame_counter.cycles += 1;

        if self.frame_counter.cycles == cycles_per_frame + 1 {
            self.frame_counter.reset();
        }

        if self.frame_counter.irq() && !self.irq_inhibit {
            self.frame_irq = true;
        }
    }

    fn clock_length_counters(&mut self, forced: bool) {
        for channel in self.channels.iter_mut() {
            if self.frame_counter.quarter_frame() || forced {
                // Envelope / Triangle Linear counter
                channel.clock_envelope();
            }

            if self.frame_counter.half_frame() || forced {
                channel.clock_length_counter();

                // Sweep
                channel.clock_sweep();
            }
        }
    }

    fn clock_timers(&mut self) {
        // Triangle's timer is clocked on every CPU clock, the rest of the channels' timers
        // are clocked on every other CPU clock

        self.channels[TRIANGLE].clock_timer();

        if self.cpu_cycles % 2 == 0 {
            self.channels[PULSE_1].clock_timer();
            self.channels[PULSE_2].clock_timer();
            self.channels[NOISE].clock_timer();
            self.channels[DMC].clock_timer();
        }
    }

    pub fn step(&mut self, cpu_cycles: u64) -> bool {
        let cycles_to_run = cpu_cycles - self.cpu_cycles;
        let even_cycle = cpu_cycles % 2 == 0;

        // Delayed reset of the frame counter after a write occurs to $4017
        if self.frame_counter.delayed_reset {
            self.frame_counter.reset_after_cycles = if even_cycle {
                3
            } else {
                4
            };
            self.frame_counter.delayed_reset = false;
        }

        for _ in 0..cycles_to_run {
            self.cpu_cycles += 1;

            self.clock_frame_counter();

            if self.frame_counter.reset_after_cycles > 0 {
                self.frame_counter.reset_after_cycles -= 1;
                if self.frame_counter.reset_after_cycles == 0 {
                    self.frame_counter.reset();
                    if self.frame_counter.mode == FrameCounterMode::Mode5Step {
                        self.clock_length_counters(true);
                    }
                }
            }

            self.clock_length_counters(false);
            self.clock_timers();
            self.clock_channel_output();
            self.generate_output_samples();
        }

        self.apu_cycles = self.cpu_cycles as f64 / 2.0;

        let irq = self.frame_irq && !self.irq_inhibit && self.cpu_cycles > self.next_irq_cycles;
        if irq {
            self.next_irq_cycles = 0;
        }
        irq
    }
}

impl MemMapped for Apu {
    fn read(&mut self, addr: u16) -> Result<u8, EmulationError> {
        match addr {
            // Status register
            0x4015 => {
                let status = self.read_status();
                // Clear frame_irq on read but only if the interrupt hasn't occurred
                // at the same time the status register is being read
                // if self.frame_counter.cycles_since_interrupt > 0 {
                self.frame_irq = false;
                // }
                Ok(status)
            }
            // The rest of the registers cannot be read from
            _ => {
                //println!("Attempted invalid read from APU register: 0x{:04X}", addr);
                Ok(0)
            }
        }
    }

    fn write(&mut self, addr: u16, byte: u8) -> Result<(), EmulationError> {
        match addr {
            // Pulse 1
            // Duty (DD), Envelope loop/Length counter Halt (LC), constant volume (C), volume/envelope (VVVV)
            0x4000 => {
                self.channels[PULSE_1].write_reg(0, byte);
                Ok(())
            }
            // Sweep unit: enabled (E), period (P), negate (N), shift (S)
            0x4001 => {
                self.channels[PULSE_1].write_reg(1, byte);
                Ok(())
            }
            // Timer low  (T)
            0x4002 => {
                self.channels[PULSE_1].write_reg(2, byte);
                Ok(())
            }
            // Length counter load (L), timer high (T)
            0x4003 => {
                self.channels[PULSE_1].write_reg(3, byte);
                Ok(())
            }

            // Pulse2
            // Duty (DD), Envelope loop/Length counter Halt (LC), constant volume (C), volume/envelope (VVVV)
            0x4004 => {
                self.channels[PULSE_2].write_reg(0, byte);
                Ok(())
            }
            // Sweep unit: enabled (E), period (P), negate (N), shift (S)
            0x4005 => {
                self.channels[PULSE_2].write_reg(1, byte);
                Ok(())
            }
            // Timer low  (T)
            0x4006 => {
                self.channels[PULSE_2].write_reg(2, byte);
                Ok(())
            }
            // Length counter load (L), timer high (T)
            0x4007 => {
                self.channels[PULSE_2].write_reg(3, byte);
                Ok(())
            }

            // Triangle
            // Length counter halt / linear counter control (C), linear counter load (R)
            0x4008 => {
                self.channels[TRIANGLE].write_reg(0, byte);
                Ok(())
            }
            // Unused (U), but can still be written to and read from
            0x4009 => {
                self.channels[TRIANGLE].write_reg(1, byte);
                Ok(())
            }
            // Timer low (T)
            0x400A => {
                self.channels[TRIANGLE].write_reg(2, byte);
                Ok(())
            }
            // Length counter load (L), timer high (T), linear counter reload flag
            0x400B => {
                self.channels[TRIANGLE].write_reg(3, byte);
                Ok(())
            }

            // Noise
            // Unused (U), Envelope loop / length counter halt (L), constant volume (C), volume/envelope (V)
            0x400C => {
                self.channels[NOISE].write_reg(0, byte);
                Ok(())
            }
            // Unused (U), but can still be written to
            0x400D => {
                self.channels[NOISE].write_reg(1, byte);
                Ok(())
            }
            // Loop noise (L), unused (U), noise period (P)
            0x400E => {
                self.channels[NOISE].write_reg(2, byte);
                Ok(())
            }
            // Length counter load (L), unused (U)
            0x400F => {
                self.channels[NOISE].write_reg(3, byte);
                Ok(())
            }

            // DMC
            // IRQ enable (I), loop (L), unused (U), frequency (R)
            0x4010 => {
                self.channels[DMC].write_reg(0, byte);
                Ok(())
            }
            // Unused (U), load counter (D)
            0x4011 => {
                self.channels[DMC].write_reg(1, byte);
                Ok(())
            }
            // Sample address (A)
            0x4012 => {
                self.channels[DMC].write_reg(2, byte);
                Ok(())
            }
            // Sample length (L)
            0x4013 => {
                self.channels[DMC].write_reg(3, byte);
                Ok(())
            }
            //
            // 0x4014 is skipped, it's not part of the APU,
            // but rather the OMA DMA register
            //

            // Status register
            0x4015 => {
                self.write_status(byte);
                Ok(())
            }

            // Frame counter
            // This register is used for both APU and I/O manipulation
            // The APU only uses bits 6 and 7
            0x4017 => {
                self.write_frame_counter(byte);
                Ok(())
            }

            _ => unreachable!()
        }
    }
}
