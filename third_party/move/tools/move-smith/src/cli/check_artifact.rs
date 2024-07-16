// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Simple CLI tool that checks if a Move transactional test works as expected

use arbitrary::Unstructured;
use clap::Parser;
use move_smith::{utils::run_transactional_test, CodeGenerator, MoveSmith};
use std::{fs, path::PathBuf};

#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Args {
    /// The input file to check
    #[clap(short('f'), long)]
    input_file: PathBuf,
}

fn main() {
    env_logger::init();
    let args = Args::parse();
    let bytes = fs::read(&args.input_file).unwrap();
    let mut u = Unstructured::new(&bytes);
    let mut smith = MoveSmith::default();
    match smith.generate(&mut u) {
        Ok(()) => println!("Parsed raw input successfully"),
        Err(e) => {
            println!("Failed to parse raw input: {:?}", e);
            std::process::exit(1);
        },
    };
    let code = smith.get_compile_unit().emit_code();
    println!("Loaded code from file: {:?}", args.input_file);

    let result = run_transactional_test(code, &smith.config.take());
    if result.is_err() {
        println!("check_artifact failed");
        println!("{}", result);
        std::process::exit(1);
    } else {
        println!("check_artifact passed successfully");
    }
}
