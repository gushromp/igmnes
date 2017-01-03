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
pub struct Apu;
