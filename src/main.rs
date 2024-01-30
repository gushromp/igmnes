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

    let mut attach_debugger = false;
    let mut enable_tracing = false;
    let mut entry_point: Option<u16> = None;

    let mut arg_index = 2;
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
            arg_index += 1;
        }
    }

    let mut core = Core::load_rom(rom_path).unwrap();
    core.start(attach_debugger, enable_tracing, entry_point);
}