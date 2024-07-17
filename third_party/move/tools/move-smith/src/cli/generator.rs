// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Simple CLI tool that statically generates Move source files or packages
//! with random content controlled by a seed.

use clap::Parser;
use move_smith::{
    utils::{create_move_package, raw_to_compile_unit},
    CodeGenerator,
};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::{fs, path::PathBuf};

#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Args {
    /// The output directory to store the generated Move files
    #[clap(short, long)]
    output_dir: PathBuf,

    /// An optional number as seed, the default should be 0
    #[clap(short, long, default_value = "0")]
    seed: u64,

    /// An optional number as the number of files to generate, the default should be 100
    #[clap(short, long, default_value = "100")]
    num_files: usize,

    /// A boolean flag to create a package, default to false
    #[clap(short, long)]
    package: bool,
}

const BUFFER_SIZE_START: usize = 1024 * 8;
const BUFFER_SIZE_MAX: usize = 1024 * 32;

fn main() {
    env_logger::init();
    let args = Args::parse();
    fs::create_dir_all(&args.output_dir).expect("Failed to create output directory");

    println!("Using seed: {}", args.seed);
    let mut rng = StdRng::seed_from_u64(args.seed);

    for i in 0..args.num_files {
        println!("MoveSmith: generating file #{}", i);
        let mut buffer_size = BUFFER_SIZE_START;
        let mut buffer = vec![];
        let code = loop {
            if buffer_size > buffer.len() {
                let diff = buffer_size - buffer.len();
                let mut new_buffer = vec![0u8; diff];
                rng.fill(&mut new_buffer[..]);
                buffer.extend(new_buffer);
            }

            match raw_to_compile_unit(&buffer) {
                Ok(module) => break module.emit_code(),
                Err(e) => {
                    if buffer_size > BUFFER_SIZE_MAX {
                        println!("Failed to parse raw bytes: {}", e);
                        break String::from("not enough data");
                    }
                },
            }
            buffer_size *= 2;

            println!("Doubling buffer size to {} bytes", buffer_size);
        };
        println!("Generated MoveSmith instance with {} bytes", buffer_size);

        let mut buffer_file_path = args.output_dir.join(format!("buffer_{}.raw", i));
        if args.package {
            let package_dir = args.output_dir.join(format!("Package_{}", i));
            create_move_package(code, &package_dir);
            buffer_file_path = package_dir.join("buffer.raw");
        }
        fs::write(&buffer_file_path, buffer).expect("Failed to write the raw buffer file");
    }

    let output_format = if args.package { "packages" } else { "files" };
    println!(
        "Generated {} {} in {:?}",
        args.num_files, output_format, args.output_dir
    );
}
