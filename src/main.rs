#[macro_use]
extern crate nom;
extern crate sdl2;
extern crate rfd;

mod core;

use core::Core;
use std::path::{Path, PathBuf};
use rfd::FileDialog;

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
        let mut core = Core::load_rom(rom_path.as_path()).unwrap();
        core.start(attach_debugger, enable_tracing, entry_point);
    } else {
        println!("Usage: igmnes path_to_rom");
        std::process::exit(1);
    }
}