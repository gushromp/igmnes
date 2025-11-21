use igmnes_core::debug::Tracer;
use igmnes_core::debugger::Debugger;
use igmnes_core::Core;
use rfd::FileDialog;
use std::path::{Path, PathBuf};
use std::ptr;
use std::time::{Duration, Instant};

use sdl2::audio::AudioSpecDesired;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::render::{TextureCreator, WindowCanvas};
use sdl2::video::FullscreenType;

const WINDOW_SCALING: u32 = 3;
const PIXELS_PER_SCANLINE: usize = 256_usize;
const BYTES_PER_SCANLINE: usize = PIXELS_PER_SCANLINE * 3;
const SCANLINES: usize = 240;
const SCANLINES_OFFSET: usize = 8;

const NANOS_PER_FRAME: u128 = 16_666_667;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut rom_path: Option<PathBuf> = None;

    let mut attach_debugger = false;
    let mut enable_tracing = false;
    let mut entry_point: Option<u16> = None;

    let mut arg_index = 1;
    while arg_index < args.len() {
        let arg = &args[arg_index];
        if arg == "--attach-debugger" {
            attach_debugger = true;
            arg_index += 1;
        } else if arg == "--trace" {
            enable_tracing = true;
            arg_index += 1;
        } else if arg == "--entry" {
            let entry_address_hex: &String = &args[arg_index + 1];
            let without_prefix = entry_address_hex.trim_start_matches("0x");
            let entry_point_addr = u16::from_str_radix(without_prefix, 16).unwrap();
            entry_point = Some(entry_point_addr);
            arg_index += 2;
        } else {
            rom_path = Some(PathBuf::from(&args[1]));
            arg_index += 1;
        }
    }

    if rom_path.is_none() {
        let working_dir = std::env::current_dir().unwrap();
        let file = FileDialog::new()
            .add_filter("ROM", &["nes"])
            .set_directory(working_dir)
            .pick_file();

        if let Some(path) = file {
            rom_path = Some(path);
        }
    }

    if let Some(rom_path) = rom_path {
        let core = Core::load_rom(rom_path.as_path()).unwrap();
        start(core, attach_debugger, enable_tracing, entry_point);
    } else {
        println!("Usage: igmnes path_to_rom");
        std::process::exit(1);
    }
}

pub fn start(
    mut core: Core,
    attach_debugger: bool,
    enable_tracing: bool,
    entry_point: Option<u16>,
) {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let audio_subsystem = sdl_context.audio().unwrap();

    let audio_spec_desired = AudioSpecDesired {
        freq: Some(44_100),
        channels: Some(1),
        samples: Some(1),
    };

    let audio_queue = audio_subsystem
        .open_queue::<f32, _>(None, &audio_spec_desired)
        .unwrap();
    audio_queue.resume();

    let mut events = sdl_context.event_pump().unwrap();

    let window = video_subsystem
        .window("IGMNes", 256 * WINDOW_SCALING, 240 * WINDOW_SCALING)
        .resizable()
        .position_centered()
        .build()
        .unwrap();

    let mut renderer = window.into_canvas().build().unwrap();
    renderer.set_logical_size(256, 232).unwrap();

    let texture_creator = renderer.texture_creator();

    if attach_debugger {
        let debugger = core.attach_debugger();
        debugger.start_listening();
    }

    let mut tracer = Tracer::default();
    tracer.set_enabled(enable_tracing);

    if let Some(entry_point) = entry_point {
        core.set_entry_point(entry_point);
    }

    let start_time = Instant::now();

    'running: loop {
        let frame_start = Instant::now();

        let mut did_change_fullscreen_state = false;
        // Events
        for event in events.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::KeyDown {
                    keycode: Some(Keycode::F9),
                    ..
                } => {
                    if did_change_fullscreen_state {
                        break;
                    }
                    let new_state =
                        if renderer.window().fullscreen_state() == FullscreenType::Desktop {
                            FullscreenType::Off
                        } else {
                            FullscreenType::Desktop
                        };
                    renderer.window_mut().set_fullscreen(new_state).unwrap();
                    did_change_fullscreen_state = true;
                }
                Event::KeyDown {
                    keycode: Some(Keycode::F12),
                    ..
                } => {
                    let debugger = core.attach_debugger();

                    if !debugger.is_listening() {
                        debugger.start_listening();
                    }
                }
                _ => {}
            }
        }

        // Input
        let keyboard_state = events.keyboard_state();
        let pressed_scancodes = keyboard_state.pressed_scancodes();
        let keys: Vec<Keycode> = pressed_scancodes
            .filter_map(Keycode::from_scancode)
            .collect();

        set_controllers_state(&mut core, keys.iter());

        // Run emulation until PPU frame ready
        while !core.is_ppu_frame_ready() {
            core.step(&mut tracer)
        }

        // Render frame
        render_frame(&mut core, &mut renderer, &texture_creator);

        // Audio
        while !core.is_apu_output_ready() {
            // Keep running (if necessary) until we have audio enough samples for this frame
            core.step(&mut tracer);
        }
        let samples = core.apu_output_samples();
        audio_queue.queue_audio(&samples).unwrap();

        // Sleep
        let frame_duration = Instant::now().duration_since(frame_start);
        let frame_duration_nanos = frame_duration.as_nanos();
        if frame_duration_nanos < NANOS_PER_FRAME {
            // Sleep for a certain amount to alleviate CPU usage, then use busy loop for rest for accurate timing
            let frame_duration_millis = frame_duration.as_millis();
            let ms_to_sleep = 16 - frame_duration_millis as u64 - 1;

            let duration_to_sleep = Duration::from_millis(ms_to_sleep);
            std::thread::sleep(duration_to_sleep);
            while Instant::now().duration_since(frame_start).as_nanos() < NANOS_PER_FRAME {}
        }
    }

    if tracer.has_traces() {
        tracer.write_to_file(Path::new("./trace.log"));
    }

    let cur_time = Instant::now();
    let seconds = cur_time.duration_since(start_time).as_millis() as f64 / 1000.0;
    println!("Cycles: {}", core.cpu_cycles());
    println!("Seconds: {}", seconds);
    if seconds > 0.0 {
        println!(
            "Cycles per second: {}",
            (core.cpu_cycles() as f64 / seconds).floor()
        );
    }
}

pub fn set_controllers_state<'a, I>(core: &mut Core, state: I)
where
    I: Iterator<Item = &'a Keycode>,
{
    use igmnes_core::{ControllerButton, ControllerIndex};
    let mut controller_1_state: Vec<ControllerButton> = vec![];

    for key_state in state {
        let button_state = match *key_state {
            Keycode::X => Some(ControllerButton::A),
            Keycode::Z => Some(ControllerButton::B),
            Keycode::RShift => Some(ControllerButton::SELECT),
            Keycode::Return => Some(ControllerButton::START),
            Keycode::Up => Some(ControllerButton::UP),
            Keycode::Down => Some(ControllerButton::DOWN),
            Keycode::Left => Some(ControllerButton::LEFT),
            Keycode::Right => Some(ControllerButton::RIGHT),
            _ => None,
        };

        if let Some(button_state) = button_state {
            controller_1_state.push(button_state);
        }
    }

    core.set_controller_button_state(ControllerIndex::First, &controller_1_state);
}

fn render_frame<T>(
    core: &mut Core,
    renderer: &mut WindowCanvas,
    texture_creator: &TextureCreator<T>,
) {
    let frame = core.ppu_frame();
    unsafe {
        let pointer = ptr::addr_of!(*frame);
        let pointer_arr = pointer as *mut [u8; BYTES_PER_SCANLINE * SCANLINES];
        let mut data = *pointer_arr;

        let offset = BYTES_PER_SCANLINE * SCANLINES_OFFSET;
        let data_slice = &mut data[offset..];
        let surface = sdl2::surface::Surface::from_data(
            data_slice,
            256,
            240 - (SCANLINES_OFFSET as u32 * 2),
            BYTES_PER_SCANLINE as u32,
            PixelFormatEnum::RGB24,
        )
        .unwrap();
        let tex = surface.as_texture(texture_creator).unwrap();
        renderer.copy(&tex, None, None).unwrap();
        renderer.present();
    }
}
