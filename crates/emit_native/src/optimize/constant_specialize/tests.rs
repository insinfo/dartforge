use super::*;
fn integer(value: i64) -> Operand {
    Operand::Constant(Constant::Int(value))
}
fn leaf(symbol: &str) -> Function {
    Function {
        symbol: symbol.into(),
        name: symbol.into(),
        params: vec![],
        return_ty: Type::I64,
        debug_info: None,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![],
            terminator: Terminator::Return(Some(integer(0))),
        }],
    }
}
fn target() -> Function {
    let mut f = leaf("df.test.choose");
    f.params = vec![
        (ValueId(0), "value".into(), Type::I64),
        (ValueId(1), "mode".into(), Type::I64),
    ];
    f.blocks = vec![
        BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(2),
                    Instruction::CallRuntime {
                        name: "visible_effect".into(),
                        args: vec![],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
                (
                    ValueId(3),
                    Instruction::ICmp(ICmpOp::Eq, Operand::Val(ValueId(1)), integer(0)),
                    Type::I1,
                ),
            ],
            terminator: Terminator::CondBranch {
                cond: Operand::Val(ValueId(3)),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        },
        BasicBlock {
            id: BlockId(1),
            instructions: vec![(
                ValueId(4),
                Instruction::Add(Operand::Val(ValueId(0)), integer(3)),
                Type::I64,
            )],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(4)))),
        },
        BasicBlock {
            id: BlockId(2),
            instructions: vec![(
                ValueId(5),
                Instruction::CallRuntime {
                    name: "throw_error".into(),
                    args: vec![],
                    ret_ty: Type::Void,
                },
                Type::Void,
            )],
            terminator: Terminator::Return(Some(integer(0))),
        },
    ];
    f
}
fn call(symbol: &str, value: i64, mode: Operand) -> Instruction {
    Instruction::CallStatic {
        symbol: symbol.into(),
        args: vec![integer(value), mode],
        ret_ty: Type::I64,
    }
}
fn fixture() -> Module {
    let mut module = Module::new();
    let mut caller = leaf("dart_main");
    caller.params = vec![(ValueId(0), "unknown".into(), Type::I64)];
    caller.blocks[0].instructions = vec![
        (
            ValueId(1),
            Instruction::CallRuntime {
                name: "argument_effect".into(),
                args: vec![],
                ret_ty: Type::I64,
            },
            Type::I64,
        ),
        (ValueId(2), call("df.test.choose", 5, integer(0)), Type::I64),
        (
            ValueId(3),
            Instruction::CallRuntime {
                name: "dartforge_exception_pending".into(),
                args: vec![],
                ret_ty: Type::I8,
            },
            Type::I8,
        ),
        (ValueId(4), call("df.test.choose", 5, integer(0)), Type::I64),
        (ValueId(5), call("df.test.choose", 5, integer(1)), Type::I64),
        (
            ValueId(6),
            call("df.test.choose", 5, Operand::Val(ValueId(0))),
            Type::I64,
        ),
        (
            ValueId(7),
            Instruction::TearOff {
                code_symbol: "df.test.choose".into(),
            },
            Type::Ref,
        ),
    ];
    module.functions = vec![caller, target()];
    module
}
fn call_symbol(module: &Module, value: u32) -> &str {
    let Instruction::CallStatic { symbol, .. } = &module.functions[0].blocks[0]
        .instructions
        .iter()
        .find(|(id, _, _)| id.0 == value)
        .unwrap()
        .1
    else {
        panic!("static call")
    };
    symbol
}
#[test]
fn clones_reduce_branches_reuse_keys_keep_abi_effects_and_generic_entry() {
    let mut module = fixture();
    let original = format!("{:?}", module.functions[1]);
    assert_eq!(rewrite_enabled(&mut module), 2);
    assert_eq!(call_symbol(&module, 2), call_symbol(&module, 4));
    assert_ne!(call_symbol(&module, 2), call_symbol(&module, 5));
    assert_eq!(call_symbol(&module, 6), "df.test.choose");
    assert_eq!(format!("{:?}", module.functions[1]), original);
    assert!(matches!(module.functions[0].blocks[0].instructions[0].1,
        Instruction::CallRuntime { ref name, .. } if name == "argument_effect"));
    for clone in module
        .functions
        .iter()
        .filter(|f| is_specialized_symbol(&f.symbol))
    {
        assert_eq!(clone.params, module.functions[1].params);
        assert_eq!(branches(clone), 0);
        assert!(clone.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)|
            matches!(i, Instruction::CallRuntime { name, .. } if name == "visible_effect")));
    }
    let throwing = module
        .functions
        .iter()
        .find(|f| f.symbol == call_symbol(&module, 5))
        .unwrap();
    assert!(throwing.blocks.iter().flat_map(|b| &b.instructions).any(
        |(_, i, _)| matches!(i, Instruction::CallRuntime { name, .. } if name == "throw_error")
    ));
}
#[test]
fn nonrecursive_specialized_guards_keep_original_pending_check() {
    let mut module = fixture();
    assert_eq!(rewrite_enabled(&mut module), 2);
    let no_throw = super::super::effects::do_not_throw(&module);
    assert!(!no_throw.contains(call_symbol(&module, 2)));
    super::super::simplify::dead_checks(&mut module.functions[0], &no_throw);
    assert!(module.functions[0].blocks[0]
        .instructions
        .iter()
        .any(|(id, _, _)| id.0 == 3));
}
#[test]
fn unknown_recursive_ambiguous_debug_and_never_inline_targets_remain_generic() {
    for kind in 0..5 {
        let mut module = fixture();
        match kind {
            0 => {
                module.functions.pop();
            }
            1 => module.functions[1].blocks[0].instructions.push((
                ValueId(9),
                call("df.test.choose", 5, integer(0)),
                Type::I64,
            )),
            2 => module.functions.push(target()),
            3 => module.functions[1].debug_info = Some(Box::default()),
            _ => {
                module
                    .inline_policies
                    .insert("df.test.choose".into(), InlinePolicy::Never);
            }
        }
        assert_eq!(
            rewrite_enabled(&mut module),
            0,
            "rejected target kind {kind}"
        );
        assert_eq!(call_symbol(&module, 2), "df.test.choose");
    }
}
#[test]
fn deterministic_ids_and_literal_float_bits_are_preserved() {
    assert_ne!(
        Scalar::read(&Constant::Double(-0.0), Type::F64),
        Scalar::read(&Constant::Double(0.0), Type::F64)
    );
    let nan = f64::from_bits(0x7ff8000000000042);
    assert_eq!(
        Scalar::read(&Constant::Double(nan), Type::F64).unwrap(),
        Scalar::Double(nan.to_bits())
    );
    let mut a = fixture();
    let mut b = fixture();
    rewrite_enabled(&mut a);
    rewrite_enabled(&mut b);
    assert_eq!(
        a.functions.iter().map(|f| &f.symbol).collect::<Vec<_>>(),
        b.functions.iter().map(|f| &f.symbol).collect::<Vec<_>>()
    );
}
#[test]
fn unsupported_memory_modes_and_unprofitable_clones_do_not_expand() {
    let mut module = fixture();
    module.arc_memory = true;
    assert_eq!(rewrite_enabled(&mut module), 0);
    let mut module = fixture();
    module.functions[1].blocks = vec![BasicBlock {
        id: BlockId(0),
        instructions: vec![],
        terminator: Terminator::Return(Some(integer(9))),
    }];
    assert_eq!(rewrite_enabled(&mut module), 0);
}

#[test]
fn clone_budget_stops_distinct_context_growth() {
    let mut module = fixture();
    module.functions[0].blocks[0].instructions = (0..40)
        .map(|value| {
            (
                ValueId(value + 1),
                call("df.test.choose", i64::from(value), integer(0)),
                Type::I64,
            )
        })
        .collect();
    assert_eq!(rewrite_enabled(&mut module), MAX_PER_TARGET);
    assert_eq!(
        module
            .functions
            .iter()
            .filter(|f| is_specialized_symbol(&f.symbol))
            .count(),
        MAX_PER_TARGET
    );
    assert!(module.functions[0].blocks[0].instructions.iter().any(|(_, i, _)|
        matches!(i, Instruction::CallStatic { symbol, .. } if symbol == "df.test.choose")));
    assert_eq!(
        rewrite_enabled(&mut module),
        0,
        "the per-target bound persists across repeated invocations"
    );
}

fn with_context(
    entry: &std::path::Path,
    edit: Option<(&str, &str, &str)>,
    packages: Option<&std::path::Path>,
    action: impl FnOnce(&mut crate::context::Context),
) {
    let temporary = tempfile::tempdir().unwrap();
    let mut sdk = crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();
    if let Some((file, old, new)) = edit {
        let original = sdk.root.join(file);
        let selected = sdk.replacements.get(&original).unwrap_or(&original);
        let source = std::fs::read_to_string(selected).unwrap();
        assert!(
            source.contains(old),
            "SDK test replacement must actually apply: {file}"
        );
        let modified = temporary.path().join("sdk-edit.dart");
        std::fs::write(&modified, source.replace(old, new)).unwrap();
        sdk.replacements.insert(original, modified);
    }
    let mut interner = dartforge_intern::Interner::new();
    let (program, diagnostics) =
        dartforge_elements::load::load_lenient(entry, &sdk, packages, &mut interner);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let mut table = dartforge_types::TypeTable::new();
    let core = dartforge_types::CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
    let libraries: Vec<_> = program
        .libraries
        .iter()
        .enumerate()
        .filter(|(_, l)| !l.is_sdk)
        .map(|(i, _)| dartforge_elements::model::LibraryId(i as u32))
        .collect();
    let (bodies, _) = dartforge_types::infer_library_bodies_with_locals(
        &program,
        &interner,
        &mut table,
        &core,
        &mut outline,
        &libraries,
    );
    let mut context =
        crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies)
            .with_source_sdk();
    if packages.is_some() {
        context = context.with_source_sdk_and_ids(crate::sdk_module::sdk_class_ids(&sdk.root).unwrap());
    }
    action(&mut context);
}

#[test]
fn actual_double_source_specialization_preserves_zero_sign_and_nan_arguments() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        let temporary = tempfile::tempdir().unwrap();
        let entry = temporary.path().join("double-constants.dart");
        std::fs::write(&entry, "double select(double value, bool first) { if (first) return value; return value * 2.0; } void main() { print(select(-0.0, true)); print(select(0.0, true)); print(select(double.nan, true)); }").unwrap();
        with_context(&entry, None, None, |ctx| {
            let mut module = crate::lower::lower_program(ctx);
            module.arc_fixed_ancestry = true;
            assert!(module.errors.is_empty(), "{:?}", module.errors);
            let original = module.functions.iter().find(|f| f.name == "select").unwrap().clone();
            assert_eq!(rewrite_enabled(&mut module), 3);
            assert!(module.functions.iter().any(|f| f.symbol == original.symbol));
            let clones: Vec<_> = module.functions.iter().filter(|f| is_specialized_symbol(&f.symbol)).collect();
            assert!(clones.iter().all(|f| f.params == original.params && branches(f) == 0));
            let returns: Vec<_> = clones.iter().flat_map(|f| &f.blocks).filter_map(|b| match &b.terminator {
                Terminator::Return(Some(Operand::Constant(Constant::Double(value)))) => Some(value.to_bits()),
                _ => None,
            }).collect();
            assert!(returns.contains(&(-0.0f64).to_bits()));
            assert!(returns.contains(&0.0f64.to_bits()));
            assert!(clones.iter().any(|f| f.blocks.iter().any(|b| matches!(b.terminator,
                Terminator::Return(Some(Operand::Val(value))) if value == original.params[0].0)))
                || returns.iter().any(|bits| f64::from_bits(*bits).is_nan()),
                "NaN remains the original evaluated argument or its exact scalar literal");
        });
    }).unwrap().join().unwrap();
}

#[test]
fn actual_color_context_reports_post_prune_specialization_eligibility() {
    std::thread::Builder::new().stack_size(128 << 20).spawn(|| {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let entry = root.join("bench/comparison/color_numeric_performance.dart");
        // Mirror the unchanged application unit into a temporary package so
        // this regression does not depend on the owner's separate checkout.
        let directory = tempfile::tempdir().unwrap();
        let format = directory.path().join("dart_ui/lib/src/graphics/image/codecs/color/format.dart");
        std::fs::create_dir_all(format.parent().unwrap()).unwrap();
        std::fs::copy(root.join("crates/emit_native/tests/fixtures/color_context/format.dart"), &format).unwrap();
        let packages = directory.path().join("package_config.json");
        std::fs::write(&packages, serde_json::to_vec(&serde_json::json!({
            "configVersion": 2,
            "packages": [{"name": "dart_ui", "rootUri": "dart_ui/", "packageUri": "lib/", "languageVersion": "3.6"}]
        })).unwrap()).unwrap();
        with_context(&entry, None, Some(&packages), |ctx| {
            let mut module = crate::lower::lower_program(ctx);
            assert!(module.errors.is_empty(), "{:?}", module.errors);
            module.arc_fixed_ancestry = true;
            let original = module.functions.iter().find(|f| f.name == "convertFormatValue").unwrap();
            eprintln!("color lowered target size={} branches={} metadata={}", size(original), branches(original), module.simple_enum_constants.len());
            super::super::prepare_for_pruning(&mut module);
            crate::pruning::prune_hir(&mut module, &[]);
            for function in &mut module.functions { if types(function).is_some() { cleanup(function); } }
            let target = module.functions.iter().find(|f| f.name == "convertFormatValue").unwrap();
            let caller = module.functions.iter().find(|f| f.name == "convertPixels").unwrap();
            let inventory = super::super::enum_constants::Inventory::new(&module);
            let args = caller.blocks.iter().flat_map(|b| &b.instructions).find_map(|(_, i, _)| match i {
                Instruction::CallStatic {symbol, args, ..} if symbol == &target.symbol => Some(args), _=> None,
            }).unwrap();
            let mut formals = HashMap::new();
            for (argument, (formal, _, _)) in args.iter().zip(&target.params) {
                let token = match argument { Operand::Val(value) => inventory.token_for_call(caller, *value), _=> None };
                eprintln!("color argument={argument:?} formal={formal:?} token={token:?}");
                if let Some(token) = token { formals.insert(*formal, token); }
            }
            let mut reduced = target.clone();
            let folded = inventory.fold_identity_comparisons(&mut reduced, &formals);
            cleanup(&mut reduced);
            eprintln!("color candidate target={} branches={} size_limit={} folded={} reduced={} branches={} growth_limit={} ratio_pass={} ABI={:?} ret={:?}",size(target),branches(target),MAX_INPUT_BODY,folded,size(&reduced),branches(&reduced),MAX_GROWTH,size(&reduced)*4<=size(target)*3,target.params,target.return_ty);
            let generic_symbol = target.symbol.clone();
            drop(inventory);
            let count = rewrite_enabled(&mut module);
            eprintln!("color actual accepted clones={count}");
            assert!(count > 0, "Actual ordinary color call must produce a profitable specialization");
            let caller = module.functions.iter().find(|f| f.name == "convertPixels").unwrap();
            let specialized = caller.blocks.iter().flat_map(|b| &b.instructions).find_map(|(_, i, _)| match i {
                Instruction::CallStatic {symbol, ..} if is_specialized_symbol(symbol) => Some(symbol), _=> None,
            }).expect("actual color caller must target the reduced clone");
            let clone = module.functions.iter().find(|f| &f.symbol == specialized).unwrap();
            assert_eq!(clone.name, "convertFormatValue");
            assert!(size(clone) <= MAX_GROWTH);
            assert!(module.functions.iter().any(|f| f.symbol == generic_symbol));
        });
    }).unwrap().join().unwrap();
}

#[test]
fn independent_input_budget_accepts_profitable_large_body_and_rejects_over_limit() {
    for input_size in [MAX_INPUT_BODY, MAX_INPUT_BODY + 1] {
        let mut module = fixture();
        let target = &mut module.functions[1];
        let padding = input_size - size(target);
        for offset in 0..padding {
            target.blocks[2].instructions.insert(0, (
                ValueId(100 + offset as u32),
                Instruction::CallRuntime { name: "cold_visible_effect".into(), args: vec![], ret_ty: Type::Void },
                Type::Void,
            ));
        }
        assert_eq!(size(target), input_size);
        let original = format!("{target:?}");
        let count = rewrite_enabled(&mut module);
        if input_size == MAX_INPUT_BODY {
            assert_eq!(count, 1, "only the small proven mode-zero context fits growth");
            let clone = module.functions.iter().find(|f| is_specialized_symbol(&f.symbol)).unwrap();
            assert_eq!(branches(clone), 0);
            assert!(size(clone) <= MAX_GROWTH);
            assert!(clone.blocks.iter().flat_map(|b| &b.instructions).all(|(_, i, _)| !matches!(i,
                Instruction::CallRuntime { name, .. } if name == "cold_visible_effect")));
        } else {
            assert_eq!(count, 0, "analysis must not inspect an over-budget body");
        }
        assert_eq!(format!("{:?}", module.functions[1]), original, "generic body and cold effects stay available");
    }
}
