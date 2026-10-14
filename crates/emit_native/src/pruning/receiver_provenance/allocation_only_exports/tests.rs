use super::*;
use crate::hir::{BasicBlock, ICmpOp};

fn block(id: u32, instructions: Vec<(ValueId, I, Type)>, terminator: Terminator) -> BasicBlock {
    BasicBlock {
        id: BlockId(id),
        instructions,
        terminator,
    }
}
fn function(symbol: &str, instructions: Vec<(ValueId, I, Type)>) -> Function {
    Function {
        symbol: symbol.into(),
        name: symbol.into(),
        params: vec![],
        return_ty: Type::Ref,
        blocks: vec![block(
            0,
            instructions,
            Terminator::Return(Some(Operand::Val(ValueId(0)))),
        )],
        debug_info: None,
    }
}
fn fresh() -> Module {
    let mut module = Module::new();
    module.arc_fixed_ancestry = true;
    module.functions.push(function(
        "fresh",
        vec![(
            ValueId(0),
            I::AllocObject {
                class_id: 300,
                fields: vec![],
            },
            Type::Ref,
        )],
    ));
    module.functions.push(function(
        "factory",
        vec![(
            ValueId(0),
            I::CallStatic {
                symbol: "fresh".into(),
                args: vec![],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        )],
    ));
    module
}
fn exported(module: &Module, symbol: &str) -> bool {
    candidates(&[module], &analyze(&[module]))
        .iter()
        .any(|e| e.symbol == symbol)
}

#[test]
fn complete_immutable_factory_closure_and_nullable_union() {
    let mut module = fresh();
    let mut nullable = function("nullable", vec![]);
    nullable.params = vec![(ValueId(10), "condition".into(), Type::I1)];
    nullable.blocks = vec![
        block(
            0,
            vec![],
            Terminator::CondBranch {
                cond: Operand::Val(ValueId(10)),
                then_block: BlockId(1),
                else_block: BlockId(2),
            },
        ),
        block(
            1,
            vec![(
                ValueId(1),
                I::AllocObject {
                    class_id: 301,
                    fields: vec![],
                },
                Type::Ref,
            )],
            Terminator::Branch(BlockId(3)),
        ),
        block(2, vec![], Terminator::Branch(BlockId(3))),
        block(
            3,
            vec![(
                ValueId(0),
                I::Phi {
                    incoming: vec![
                        (BlockId(1), Operand::Val(ValueId(1))),
                        (BlockId(2), Operand::Constant(Constant::Null)),
                    ],
                    ty: Type::Ref,
                },
                Type::Ref,
            )],
            Terminator::Return(Some(Operand::Val(ValueId(0)))),
        ),
    ];
    module.functions.push(nullable);
    let proof = analyze(&[&module]);
    let exports = candidates(&[&module], &proof);
    let factory = exports.iter().find(|e| e.symbol == "factory").unwrap();
    assert_eq!(factory.classes, BTreeSet::from([300]));
    assert_eq!(factory.parameters, Vec::<Type>::new());
    assert_eq!(
        factory
            .closure
            .iter()
            .map(|f| f.symbol.as_str())
            .collect::<Vec<_>>(),
        ["fresh", "factory"]
    );
    assert!(std::ptr::eq(factory.closure[0], &module.functions[0]));
    let nullable = exports.iter().find(|e| e.symbol == "nullable").unwrap();
    assert_eq!(nullable.classes, BTreeSet::from([1, 301]));
    assert_eq!(nullable.parameters, [Type::I1]);
}

#[test]
fn complete_dependency_census_rejects_cache_global_address_and_opaque_operations() {
    for variant in 0..10 {
        let mut module = fresh();
        let forbidden = match variant {
            0 => I::LoadGlobal {
                symbol: "cache".into(),
                ty: Type::Ref,
            },
            1 => I::StoreGlobal {
                symbol: "cache".into(),
                val: Operand::Constant(Constant::Null),
                ty: Type::Ref,
                root: Some(0),
            },
            2 => I::Const(Constant::Function("hidden".into())),
            3 => I::Const(Constant::String("dartforge_globals_area".into())),
            4 => I::CallRuntime {
                name: "dartforge_object_new".into(),
                args: vec![],
                ret_ty: Type::Ref,
            },
            5 => I::CallStatic {
                symbol: "unknown".into(),
                args: vec![],
                ret_ty: Type::Ref,
            },
            6 => I::CallSelector {
                selector: "g:encoder".into(),
                recv: Operand::Val(ValueId(0)),
                args: vec![],
                names: vec![],
                tuple_types: Operand::Constant(Constant::Int(0)),
            },
            7 => I::ArcCopy {
                value: Operand::Val(ValueId(0)),
            },
            8 => I::CallStatic {
                symbol: "factory".into(),
                args: vec![],
                ret_ty: Type::Ref,
            },
            _ => I::Phi {
                incoming: vec![],
                ty: Type::Ref,
            },
        };
        module.functions[0].blocks[0]
            .instructions
            .push((ValueId(1), forbidden, Type::Ref));
        assert!(!exported(&module, "factory"), "variant {variant}");
    }
    let mut module = fresh();
    module.functions[0].symbol = "dfc.hidden.get".into();
    if let I::CallStatic { symbol, .. } = &mut module.functions[1].blocks[0].instructions[0].1 {
        *symbol = "dfc.hidden.get".into();
    }
    assert!(!exported(&module, "factory"));
}

#[test]
fn policy_abi_duplicates_and_stale_dependency_bodies_are_unknown() {
    for variant in 0..7 {
        let mut module = fresh();
        match variant {
            0 => module.arc_fixed_ancestry = false,
            1 => module.arc_memory = true,
            2 => module.arc_prepared = true,
            3 => module.functions.push(module.functions[0].clone()),
            4 => module.functions[0]
                .params
                .push((ValueId(9), "extra".into(), Type::Ref)),
            5 => module.functions[0].return_ty = Type::I64,
            _ => module.functions[0].blocks[0].instructions.push((
                ValueId(0),
                I::Const(Constant::Null),
                Type::Ref,
            )),
        }
        assert!(!exported(&module, "factory"), "variant {variant}");
    }
    // A root body may retain its address while an independently supplied callee
    // is replaced by another same-name body. All closure members need witnesses.
    let mut root = fresh();
    let dependency = root.functions.remove(0);
    let mut original = Module::new();
    original.arc_fixed_ancestry = true;
    original.functions.push(dependency.clone());
    let proof = analyze(&[&root, &original]);
    let mut replacement = Module::new();
    replacement.arc_fixed_ancestry = true;
    replacement.functions.push(dependency);
    assert!(
        candidates(&[&root, &replacement], &proof)
            .iter()
            .all(|e| e.symbol != "factory")
    );
    assert!(
        candidates(&[&root, &original], &proof)
            .iter()
            .any(|e| e.symbol == "factory")
    );
}

#[test]
fn rejected_operations_in_unreachable_blocks_are_not_exportable() {
    let mut module = fresh();
    module.functions[0].blocks.push(block(
        1,
        vec![(
            ValueId(4),
            I::StoreGlobal {
                symbol: "cache".into(),
                val: Operand::Constant(Constant::Null),
                ty: Type::Ref,
                root: Some(0),
            },
            Type::Void,
        )],
        Terminator::Unreachable,
    ));
    assert!(!exported(&module, "factory"));
    // Harmless scalar comparisons do not erase the body return proof.
    module.functions[0].blocks.pop();
    module.functions[0].blocks[0].instructions.push((
        ValueId(4),
        I::ICmp(
            ICmpOp::Eq,
            Operand::Constant(Constant::Int(1)),
            Operand::Constant(Constant::Int(1)),
        ),
        Type::I1,
    ));
    assert!(exported(&module, "factory"));
}

#[test]
fn actual_lowered_sdk_candidate_census_keeps_codec_caches_unknown() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        use dartforge_types::table::{CoreTypes, TypeTable};
        let directory = tempfile::tempdir().unwrap();
        let entry = directory.path().join("main.dart");
        std::fs::write(&entry, "import 'dart:convert';\nclass Fresh {}\nFresh factory() => Fresh();\nList<Object?> listFactory() => [];\nvoid main() { print(factory()); print(listFactory()); print(const AsciiCodec().encoder); print(const Utf8Codec().encoder); print(const Base64Codec().encoder); }\n").unwrap();
        let sdk = crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();
        let mut interner = dartforge_intern::Interner::new();
        let (program, diagnostics) = dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
        assert!(diagnostics.is_empty());
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &program, &interner);
        let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
        let libraries: Vec<_> = program.libraries.iter().enumerate()
            .filter(|(_, library)| !library.is_sdk || library.uri.strip_prefix("dart:").is_some_and(|name| crate::sdk_module::SOURCE_LIBRARIES.contains(&name)))
            .map(|(index, _)| dartforge_elements::model::LibraryId(index as u32)).collect();
        let (bodies, _) = dartforge_types::infer_library_bodies(&program, &interner, &mut table, &core, &mut outline, &libraries);
        let mut context = crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies).with_source_sdk();
        for library in &libraries { context.in_module[library.0 as usize] = true; }
        let mut module = crate::lower::lower_program(&context);
        assert!(module.errors.is_empty(), "{:?}", module.errors);
        module.arc_fixed_ancestry = true;
        crate::optimize::prepare_for_emission(&mut module, false, false);
        let proof = analyze(&[&module]);
        let exports = candidates(&[&module], &proof);
        let source_factory = module.functions.iter().find(|f| f.name == "factory").unwrap();
        assert!(exports.iter().any(|export| export.symbol == source_factory.symbol), "ordinary fresh factory must be eligible");
        for function in module.functions.iter().filter(|f| f.symbol.contains("AsciiEncoder") || f.symbol.contains("Utf8Encoder")) {
            let calls: Vec<_> = function.blocks.iter().flat_map(|b| &b.instructions).filter_map(|(_, instruction, _)| match instruction {
                I::CallStatic { symbol, .. } | I::CallRuntime { name: symbol, .. } => Some(symbol.as_str()), _ => None,
            }).collect();
            eprintln!("codec body diagnostic: {} name={} calls={calls:?}", function.symbol, function.name);
        }
        eprintln!("allocation-only source census: {} candidates / {} functions", exports.len(), module.functions.len());
        for export in &exports { eprintln!("allocation-only candidate: {} {:?}", export.symbol, export.classes); }
        for name in ["AsciiCodec", "Utf8Codec", "Base64Codec"] {
            let cid = module.classes.iter().find(|class| class.name == name).unwrap().id;
            let getter = module.method_tables.iter().find(|(class, _, _)| *class == cid)
                .and_then(|(_, _, pairs)| pairs.iter().find(|(selector, _)| selector == "g:encoder")).unwrap();
            assert!(!exports.iter().any(|export| export.symbol == getter.1), "cache/field getter incorrectly exported: {name}");
        }
        // This census supplies SDK bodies once. It does not claim that existing
        // cached SDK objects or app/SDK COMDAT boundaries transport these facts.
    }).unwrap().join().unwrap();
}

#[test]
fn ordinary_native_registry_alias_disables_getter_tables_without_erasing_allocations() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        use dartforge_types::table::{CoreTypes, TypeTable};
        let directory = tempfile::tempdir().unwrap();
        for native in [false, true] {
            let entry = directory.path().join("main.dart");
            let binding = if native { "@Native<Void Function(Int64, Pointer<Int64>, Int64)>(symbol: 'dartforge_register_methods')\nexternal void overwrite(int cid, Pointer<Int64> pairs, int count);\n" } else { "" };
            let call = if native { "overwrite(300, nullptr, 0);" } else { "" };
            let source = format!("import 'dart:ffi';\n{binding}class Fresh {{}}\nclass Owner {{ Fresh get child => Fresh(); }}\nFresh factory() => Fresh();\nvoid main() {{ dynamic owner = Owner(); print(owner.child); print(factory()); {call} }}\n");
            std::fs::write(&entry, source).unwrap();
            let sdk = crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();
            let mut interner = dartforge_intern::Interner::new();
            let (program, diagnostics) = dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
            assert!(diagnostics.is_empty());
            let mut table = TypeTable::new(); let core = CoreTypes::init(&mut table, &program, &interner);
            let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
            let libraries: Vec<_> = program.libraries.iter().enumerate().filter(|(_, library)| !library.is_sdk)
                .map(|(index, _)| dartforge_elements::model::LibraryId(index as u32)).collect();
            let (bodies, _) = dartforge_types::infer_library_bodies(&program, &interner, &mut table, &core, &mut outline, &libraries);
            let context = crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies).with_source_sdk();
            let mut module = crate::lower::lower_program(&context);
            assert!(module.errors.is_empty(), "{:?}", module.errors);
            module.arc_fixed_ancestry = true;
            crate::optimize::prepare_for_emission(&mut module, false, false);
            assert_eq!(registry::closed(&[&module]), !native);
            let proof = analyze(&[&module]);
            let main = module.functions.iter().find(|f| f.symbol == "dart_main").unwrap();
            if native {
                let child = main.blocks.iter().flat_map(|b| &b.instructions).find_map(|(value, instruction, _)| match instruction {
                    I::CallSelector { selector, .. } if selector == "g:child" => Some(*value), _ => None,
                }).unwrap_or_else(|| panic!("native mutator must retain dynamic getter: {main:#?}"));
                assert!(!proof.values_for(main).unwrap().contains_key(&child));
                let summary = crate::pruning::hir_summary(&module);
                assert!(summary.exact_receiver_calls.is_empty());
                assert!(summary.receiver_calls.is_empty());
                let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
                assert!(!ir.contains("; df.selector-exact "));
                assert!(!ir.contains("; df.selector-cone "));
            }
            let factory = module.functions.iter().find(|f| f.name == "factory").unwrap();
            assert!(proof.returns.contains_key(&factory.symbol));
        }
    }).unwrap().join().unwrap();
}
