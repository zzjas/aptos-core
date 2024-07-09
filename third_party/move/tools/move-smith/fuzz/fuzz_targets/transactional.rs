// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

#![no_main]

use arbitrary::Unstructured;
use libfuzzer_sys::fuzz_target;
use move_smith::{
    utils::{run_transactional_test, TransactionalResult},
    CodeGenerator, MoveSmith,
};
use once_cell::sync::Lazy;
use std::{env, fs::OpenOptions, io::Write, sync::Mutex, time::Instant};

static FILE_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

fuzz_target!(|data: &[u8]| {
    let u = &mut Unstructured::new(data);
    let mut smith = MoveSmith::default();
    let do_profile = match env::var("MOVE_SMITH_PROFILING") {
        Ok(v) => v == "1",
        Err(_) => false,
    };
    if do_profile {
        let mut profile_s = String::new();

        let start = Instant::now();
        match smith.generate(u) {
            Ok(()) => (),
            Err(_) => return,
        };
        let elapsed = start.elapsed();
        profile_s.push_str(&format!(
            "move-smith-profile::time::generation::{}ms\n",
            elapsed.as_millis()
        ));

        let code = smith.get_compile_unit().emit_code();
        let start = Instant::now();
        let result = run_transactional_test(code, &smith.config.take());
        let elapsed = start.elapsed();
        profile_s.push_str(&format!(
            "move-smith-profile::time::transactional::{}ms\n",
            elapsed.as_millis()
        ));

        let status = match result {
            TransactionalResult::Ok => "Ok",
            TransactionalResult::Timeout => "Timeout",
            TransactionalResult::WarningsOnly => "WarningsOnly",
            TransactionalResult::IgnoredErr(_) => "IgnoredErr",
            TransactionalResult::Err(_) => "Err",
        };
        profile_s.push_str(&format!("move-smith-profile::status::{}\n", status));

        let _lock = FILE_MUTEX.lock().unwrap();
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("move-smith-profile.txt")
            .unwrap();
        file.write_all(profile_s.as_bytes()).unwrap();
        result.unwrap();
    } else {
        match smith.generate(u) {
            Ok(()) => (),
            Err(_) => return,
        };
        let code = smith.get_compile_unit().emit_code();
        run_transactional_test(code, &smith.config.take()).unwrap();
    }
});
