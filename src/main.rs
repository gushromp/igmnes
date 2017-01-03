#[macro_use]
extern crate nom;
extern crate sdl2;

mod core;
use core::Core;




fn main() {

    // let sdl_context = sdl2::init().unwrap();
    // let sdl_audio = sdl_context.audio().unwrap();

    // let audio_spec = AudioSpecDesired {
    //     freq: Some(SAMPLE_RATE as i32),
    //     channels: Some(2),
    //     samples: None,
    // };

    // let device = sdl_audio.open_queue::<i8>(None, &audio_spec).unwrap();

    // let wave = get_triangle(SAMPLE_RATE, 30.0, 1000);
    // device.queue(&wave);

    // // Start playback
    // device.resume();

    let core = Core::new();
    // Play for 2 second
    // std::thread::sleep(Duration::from_millis(2000));
}