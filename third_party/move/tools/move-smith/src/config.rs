// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Configuration for the MoveSmith fuzzer.

use move_compiler_v2::Experiment;
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path};

/// The configuration for the MoveSmith fuzzer.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub fuzz: FuzzConfig,
    pub generation: GenerationConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FuzzConfig {
    /// The list of errors to suppress due to imprecision in the generation process
    pub ignore_errors: Vec<String>,
    /// The list of known errors to ignore
    /// This is aggresive: if the diff contains any of these strings,
    /// the report will be ignored
    pub known_errors: Vec<String>,
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
    fn default() -> Self {
        Self {
            fuzz: FuzzConfig::default(),
            generation: GenerationConfig::default(),
        }
    }
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            num_runs_per_func: 3,
            max_num_inline_funcs: 1,

            max_num_modules: 1,
            max_num_functions_in_module: 8,
            max_num_structs_in_module: 5,

            max_num_fields_in_struct: 5,
            max_num_fields_of_struct_type: 3,

            max_num_stmts_in_func: 5,
            max_num_additional_operations_in_func: 5,
            max_num_params_in_func: 7,

            max_num_stmts_in_block: 5,

            max_num_calls_in_script: 20,

            max_expr_depth: 3,
            max_type_depth: 3,
            max_num_type_params_in_func: 3,
            max_num_type_params_in_struct: 2,

            generation_timeout_sec: 5,

            allow_recursive_calls: false,

            max_hex_byte_str_size: 32,
        }
    }
}

impl Default for FuzzConfig {
    fn default() -> Self {
        let mut config = Self {
            ignore_errors: vec![
                "exceeded maximal local count".to_string(),
                "unassigned variable".to_string(),
                "unbound type".to_string(),
                "incompatible types".to_string(),
                "recursion during function inlining".to_string(),
                "still mutably borrowed".to_string(),
                "mutable ownership violated".to_string(),
                "ambiguous usage of variable".to_string(),
                "cannot assign to borrowed local".to_string(),
                "requires exclusive access but is borrowed".to_string(),
                "cannot implicitly freeze local".to_string(),
                "same mutable reference in value is also used".to_string(),
                "could create dangling a reference".to_string(),
                "referential transparency violated".to_string(),
                "invalid transfer of references".to_string(),
            ],
            known_errors: vec![
                "MOVELOC_UNAVAILABLE_ERROR".to_string(),
                "MOVELOC_EXISTS_BORROW_ERROR".to_string(),
                "STLOC_UNSAFE_TO_DESTROY_ERROR".to_string(),
                "EXTRANEOUS_ACQUIRES_ANNOTATION".to_string(),
            ],
            compiler_settings: BTreeMap::new(),
            runs: vec!["opt".to_string()],
            transactional_timeout_sec: 10,
        };
        config
            .compiler_settings
            .insert("opt".to_string(), CompilerSetting {
                enable: vec![Experiment::OPTIMIZE.to_string()],
                disable: vec![],
            });
        config
            .compiler_settings
            .insert("no-opt".to_string(), CompilerSetting {
                enable: vec![],
                disable: vec![Experiment::OPTIMIZE.to_string()],
            });
        config
            .compiler_settings
            .insert("opt-no-simp".to_string(), CompilerSetting {
                enable: vec![Experiment::OPTIMIZE.to_string()],
                disable: vec![
                    Experiment::AST_SIMPLIFY.to_string(),
                    Experiment::ACQUIRES_CHECK.to_string(),
                ],
            });
        config
    }
}

impl Config {
    pub fn from_toml_file(file_path: &Path) -> Self {
        let config_str = std::fs::read_to_string(file_path).expect("Cannot read from config file");
        let config: Config = toml::from_str(&config_str).expect("Cannot parse config file");
        config
    }

    pub fn all_errors(&self) -> Vec<String> {
        let mut errors = self.fuzz.ignore_errors.clone();
        errors.extend(self.fuzz.known_errors.clone());
        errors
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
