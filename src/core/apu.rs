// const SAMPLE_RATE: u32 = 8000;

// fn get_triangle(sample_rate: u32, mut freq: f32, duration: u32) -> Vec<i8> {

//     let volume: f32 = std::i8::MAX as f32;
//             let period: f32 = 1.0 / freq as f32;
//         let half_period: f32 = period / 2.0;

//     let mut result = Vec::new();

//     let size = (sample_rate as f32 * (duration as f32 / 1000.0) * 2.0) as u32;

//     for x in 0..size {
//         let xf = x as f32;

//         result.push({
//             let value = (volume / half_period) *
//                         (half_period - (((xf / SAMPLE_RATE as f32) % period) - half_period).abs());

//             value as i8
//         })
//     }

//     result
// }

#[derive(Debug, Default)]
struct Pulse {
    duty: u8,
    env_loop_lc_halt: bool,
    constant_volume: bool,
    volume_envelope: u8,

    sweep_enabled: bool,
    sweep_period: u8,
    sweep_negate: bool,
    sweep_shift: u8,

    timer: u8,
    length_counter: u8,
}

#[derive(Debug, Default)]
struct Triangle;

#[derive(Debug, Default)]
struct DMC;

#[derive(Debug, Default)]
pub struct Apu {
    pulse1: Pulse,
    pulse2: Pulse,
    triangle: Triangle,
    dmc: DMC,

    enable: u8,
    frame_counter: u8,
}

impl Apu {
    pub fn new() -> Apu {
        Apu {
            pulse1: Pulse::default(),
            pulse2: Pulse::default(),
            triangle: Triangle,
            dmc: DMC,

            enable: 0,
            frame_counter: 0,
        }
    }
}
