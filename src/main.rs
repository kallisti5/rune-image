/*
 * Rune - OS Image preperation tool
 *
 * Copyright, 2017 Haiku, Inc. All rights Reserved.
 * Released under the terms of the MIT license.
 *
 * Authors:
 *   Alexander von Gluck IV <kallisti5@unixzen.com>
 */

extern crate getopts;
extern crate mbr;
extern crate fatr;

#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

use std::error::Error;
use std::process;
use std::path::PathBuf;
use std::env;
use getopts::Options;
use mbr::partition;
use fatr::fat;
use apperror::AppError;

mod boards;
mod image_tools;
mod apperror;

fn print_usage(program: &str, opts: Options) {
	let brief = format!("rune - bless and write Haiku mmc images\nUsage: {} [options] <output>", program);
	print!("{}", opts.usage(&brief));
}

fn flag_error(program: &str, opts: Options, error: &str) {
	print!("Error: {}\n\n", error);
	print_usage(&program, opts);
}

/// Validate disk as containing Haiku and locate "boot" partition.
fn locate_boot_partition(disk: PathBuf) -> Result<partition::Partition,Box<Error>> {
	let partitions = partition::read_partitions(disk.clone())?;
    for (_, partition) in partitions.iter().enumerate() {
        let sector_size = 512;
        if partition.p_type != 12 {
            // Ignore non-fat partitions
            continue;
        }
        let image = fat::Image::from_file_offset(disk.clone(),
            (partitions[0].p_lba as usize * sector_size), partitions[0].p_size as usize * sector_size)?;
        let volume_id = match image.volume_label() {
            Ok(v) => v,
            Err(_) => continue,
        };
        if !volume_id.to_uppercase().contains("HAIKU") {
            continue;
        }
        // TODO: More checks?
	    return Ok(partition.clone());
    }
    return Err(From::from("no Haiku boot partitions"));
}

fn main() {
	let args: Vec<String> = env::args().collect();
	let program = args[0].clone();
	let mut opts = Options::new();
	opts.optopt("b", "board", "target board", "<board>");
	opts.optopt("i", "image", "source OS image", "<image>");
	opts.optflag("l", "list", "list supported target boards");
	opts.optflag("v", "verbose", "increase verbosity");
	opts.optflag("h", "help", "print this help");

	let matches = match opts.parse(&args[1..]) {
		Ok(m) => { m },
		Err(f) => {
			println!("Error: {}", f.to_string());
			process::exit(1);
		}
	};

	let verbose = matches.opt_present("v");

	// Validate flags
	if matches.opt_present("h") {
		print_usage(&program, opts);
		return;
	} else if matches.opt_present("l") {
		boards::print("arm".to_string());
		process::exit(1);
	}

	let output_file = if !matches.free.is_empty() {
		PathBuf::from(&matches.free[0])
	} else {
		print_usage(&program, opts);
		process::exit(1);
	};

	let board_id = match matches.opt_str("b") {
		Some(x) => x,
		None => {
			flag_error(&program, opts, "Target board not provided!");
			process::exit(1);
		},
	};
	let board = match boards::get_board(board_id) {
		Ok(x) => x,
		Err(AppError::NotFound) => {
			flag_error(&program, opts, "Unknown target board!");
			process::exit(1);
		},
		Err(e) => {
			println!("Error: {}", e.description());
			process::exit(1);
		},
	};

	print!("Creating bootable {} ({}) media.\n", board.name, board.soc);

	// If an input image was provided, write it out.
	match matches.opt_str("i") {
		Some(x) => {
			// Go ahead and write out base image to destination
			print!("Writing image to {:?}...\n", output_file);
			let source_image = PathBuf::from(&x);
			let written = match image_tools::write(source_image, output_file.clone()) {
				Ok(x) => x,
				Err(e) => {
					print!("Error: {}\n", e);
					process::exit(1);
				}
			};
			if verbose {
				print!("Wrote {} bytes to {:?}\n", written, output_file);
			}
		},
		None => {
			print!("No source image. Attempting to make target media bootable..\n");
		},
	}

	if verbose {
		print!("Scan partition table in OS image...\n");
	}

	let boot_partition = match locate_boot_partition(output_file.clone()) {
		Ok(x) => x,
		Err(e) => {
			print!("Error: {}\n", e);
			process::exit(1);
		},
	};

	if verbose {
		println!("Found: {:?}", boot_partition);
	}

	let file_count = match boards::get_files(board.clone(), output_file) {
		Ok(x) => x,
		Err(e) => {
			print!("Error: {}\n", e);
			process::exit(1);
		}
	};
	print!("Obtained {} boot-related files.\n", file_count);
}
