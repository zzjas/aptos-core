// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Configuration for the MoveSmith fuzzer.

use serde::Deserialize;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

/// The configuration for the MoveSmith fuzzer.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub fuzz: FuzzConfig,
    pub generation: GenerationConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FuzzConfig {
    /// The list of errors to suppress due to imprecision in the generation process
    pub ignore_strs: Vec<String>,
    /// The directory containing known errors
    pub known_error_dir: PathBuf,
    /// List of possible compiler settings to use
    pub compiler_settings: BTreeMap<String, CompilerSetting>,
    /// The list of compiler settings to run in current fuzzing session
    pub runs: Vec<String>,
    // Transactional test timeout
    pub transactional_timeout_sec: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CompilerSetting {
    /// The list of experiments to enable
    pub enable: Vec<String>,
    /// The list of experiments to disable
    pub disable: Vec<String>,
}

/// MoveSmith will randomly pick within [0..max_num_XXX] during generation.
#[derive(Debug, Clone, Deserialize)]
pub struct GenerationConfig {
    /// The number of `//# run 0xCAFE::ModuleX::funX` to invoke
    pub num_runs_per_func: usize,
    /// The number of functions that can have `inline`
    pub max_num_inline_funcs: usize,

    pub max_num_modules: usize,
    pub max_num_functions_in_module: usize,
    pub max_num_structs_in_module: usize,

    pub max_num_fields_in_struct: usize,
    /// The maximum total number of fields in all structs that can have
    /// type of another struct
    pub max_num_fields_of_struct_type: usize,

    // Includes all kinds of statements
    pub max_num_stmts_in_func: usize,
    // Addtionally insert some resource or vector operations
    pub max_num_additional_operations_in_func: usize,

    pub max_num_params_in_func: usize,

    // This has lowest priority
    // i.e. if the block is a function body
    // max_num_stmts_in_func will override this
    pub max_num_stmts_in_block: usize,

    pub max_num_calls_in_script: usize,

    // Maximum depth of nested expression
    pub max_expr_depth: usize,
    // Maximum depth of nested type instantiation
    pub max_type_depth: usize,

    // Maximum number of type parameters in a function
    pub max_num_type_params_in_func: usize,
    // Maximum number of type parameters in a struct definition
    pub max_num_type_params_in_struct: usize,

    // Timeout in seconds
    pub generation_timeout_sec: usize, // MoveSmith generation timeout

    // Allow recursive calls in the generated code
    pub allow_recursive_calls: bool,

    // Maximum number of bytes to construct hex or byte string
    pub max_hex_byte_str_size: usize,
}

impl Default for Config {
    /// Load default configuration from MoveSmith.default.toml
    fn default() -> Self {
        let file_content = include_str!("../MoveSmith.default.toml");
        toml::from_str(file_content).expect("Cannot parse default config TOML")
    }
}

impl Config {
    pub fn from_toml_file_or_default(file_path: &Path) -> Self {
        if file_path.exists() {
            Self::from_toml_file(file_path)
        } else {
            Config::default()
        }
    }

    pub fn from_toml_file(file_path: &Path) -> Self {
        let config_str = std::fs::read_to_string(file_path).expect("Cannot read from config file");
        let config: Config = toml::from_str(&config_str).expect("Cannot parse config file");
        config
    }

    /// Returns (Name, Compiler Configurations) for each run
    pub fn runs(&self) -> Vec<(String, CompilerSetting)> {
        let mut runs = vec![];
        for r in self.fuzz.runs.iter() {
            if let Some(setting) = self.fuzz.compiler_settings.get(r) {
                runs.push((r.clone(), setting.clone()));
            }
        }
        runs
    }

    pub fn get_compiler_setting(&self, name: &str) -> Option<&CompilerSetting> {
        self.fuzz.compiler_settings.get(name)
    }
}

impl CompilerSetting {
    pub fn to_expriments(&self) -> Vec<(String, bool)> {
        let mut exp = vec![];
        for e in self.enable.iter() {
            exp.push((e.clone(), true));
        }
        for e in self.disable.iter() {
            exp.push((e.clone(), false));
        }
        exp
    }
}
