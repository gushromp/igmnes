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

    let attach_debugger = (args.len() == 3) && (&args[2] == "--attach-debugger");

    let mut core = Core::load_rom(rom_path).unwrap();
    core.start(attach_debugger);

}