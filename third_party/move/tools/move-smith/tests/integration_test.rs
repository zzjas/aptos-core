// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use arbitrary::Unstructured;
use move_smith::{
    ast::*,
    codegen::*,
    config::*,
    move_smith::*,
    names::{Identifier, IdentifierKind as IDKind},
    runner::Runner,
    types::*,
    utils::*,
};
use num_bigint::BigUint;
use std::{cell::RefCell, collections::BTreeSet};

fn simple_module() -> Module {
    Module {
        uses: Vec::new(),
        name: Identifier::new_str("SimpleModule", IDKind::Module),
        functions: vec![RefCell::new(Function {
            signature: FunctionSignature {
                inline: false,
                type_parameters: TypeParameters::default(),
                name: Identifier::new_str("fun1", IDKind::Function),
                parameters: vec![
                    (Identifier::new_str("param1", IDKind::Var), Type::U64),
                    (Identifier::new_str("param2", IDKind::Var), Type::U8),
                ],
                return_type: Some(Type::U32),
                acquires: BTreeSet::new(),
            },
            visibility: Visibility { public: true },
            body: Some(Block {
                name: Identifier::new_str("_block0", IDKind::Function),
                stmts: vec![
                    Statement::Expr(Expression::NumberLiteral(NumberLiteral {
                        value: BigUint::from(42u32),
                        typ: Type::U32,
                    })),
                    Statement::Expr(Expression::AddressLiteral("@0xBEEF".to_string())),
                ],
                return_expr: Some(Expression::NumberLiteral(NumberLiteral {
                    value: BigUint::from(111u32),
                    typ: Type::U32,
                })),
            }),
        })],
        structs: Vec::new(),
        constants: Vec::new(),
    }
}

fn simple_script() -> Script {
    Script {
        main: vec![FunctionCall {
            name: Identifier::new_str("0xCAFE::SimpleModule::fun1", IDKind::Function),
            type_args: TypeArgs::default(),
            args: vec![
                Expression::NumberLiteral(NumberLiteral {
                    value: BigUint::from(555u64),
                    typ: Type::U64,
                }),
                Expression::NumberLiteral(NumberLiteral {
                    value: BigUint::from(255u8),
                    typ: Type::U8,
                }),
            ],
        }],
    }
}

fn simple_compile_unit() -> CompileUnit {
    CompileUnit {
        modules: vec![simple_module()],
        scripts: vec![simple_script()],
        runs: vec![],
    }
}

#[test]
fn test_emit_code() {
    let lines = simple_module().emit_code_lines();
    println!("{}", lines.join("\n"));
    assert_eq!(lines.len(), 8);
    assert_eq!(lines[0], "//# publish");
    assert_eq!(lines[1], "module 0xCAFE::SimpleModule {");
    assert_eq!(
        lines[2],
        "    public fun fun1(param1: u64, param2: u8): u32 { /* _block0 */"
    );
    assert_eq!(lines[3], "        42u32;");
    assert_eq!(lines[4], "        @0xBEEF;");
    assert_eq!(lines[5], "        111u32");
    assert_eq!(lines[6], "    }");
    assert_eq!(lines[7], "}\n");
}

#[test]
fn test_generation_and_compile() {
    let raw_data = get_random_bytes(12345, 8192);
    let mut u = Unstructured::new(&raw_data);
    let mut smith = MoveSmith::new(&Config::default().generation);
    smith.generate(&mut u).unwrap();
    let compile_unit = smith.get_compile_unit();
    let lines = compile_unit.emit_code();
    println!("{}", lines);

    assert!(compile_move_code(lines, true, true));
}

#[test]
fn test_generation_and_check_compile() {
    let raw_data = get_random_bytes(1234, 1024 * 32);
    let mut u = Unstructured::new(&raw_data);
    let mut smith = MoveSmith::new(&Config::default().generation);
    smith.generate(&mut u).unwrap();
    let compile_unit = smith.get_compile_unit();
    let code = compile_unit.emit_code();
    println!("{}", code);

    let (package_path, dir) = create_tmp_move_package(code.clone());
    let config = create_compiler_config_v1();
    let result = compile_with_config(&package_path, config, "v1");
    assert!(result);

    let config = create_compiler_config_v2();
    let result = compile_with_config(&package_path, config, "v2");
    assert!(result);
    dir.close().unwrap();
}

#[test]
fn test_run_transactional_test() {
    let runner = Runner::new(&Config::default().fuzz);
    let code = simple_compile_unit().emit_code();
    runner.run_transactional_test_unwrap(&code);
}

#[test]
fn test_run_transactional_test_should_fail() {
    let code = r#" //# publish
module 0xCAFE::Module0 {
    struct HasCopyDrop has copy, drop {}

    struct C2<T1: drop, T2: copy> has copy, drop, store {}

    fun m1<T1: copy+drop, T2: copy+drop>(x: T1) {
        m2<C2<HasCopyDrop, T2>, HasCopyDrop>(C2{});
    }
    fun m2<T3: copy+drop, T4: copy+drop>(x: T3): T3 {
        m1<T3, T4>(x);
        x
    }
}"#;
    let runner = Runner::new(&Config::default().fuzz);
    let results = runner.run_transactional_test(code);
    assert_eq!(results.len(), 1);
    assert!(results[0].result.is_err());
}

#[test]
fn test_expr_collector() {
    let module = simple_module();
    let fref = module.functions[0].borrow();
    let all_exprs = fref.all_exprs(None);
    assert!(all_exprs.len() == 3);
    let num_exprs = fref.all_exprs(Some(|e| matches!(e, Expression::NumberLiteral(_))));
    assert!(num_exprs.len() == 2);
    let addr_exprs = fref.all_exprs(Some(|e| matches!(e, Expression::AddressLiteral(_))));
    assert!(addr_exprs.len() == 1);
    let call_exprs = fref.all_exprs(Some(|e| matches!(e, Expression::FunctionCall(_))));
    assert!(call_exprs.is_empty());
}
