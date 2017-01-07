#[macro_use]
extern crate nom;
extern crate sdl2;

mod core;

use core::Core;
use std::path::Path;


fn main() {

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Usage: igmnes path_to_rom");
        std::process::exit(1);
    }

    let rom_path = Path::new(&args[1]);

    let mut core = Core::load_rom(rom_path).unwrap();
    core.attach_debugger();

    let debugger = core.debugger().unwrap();
    debugger.start_listening();

}