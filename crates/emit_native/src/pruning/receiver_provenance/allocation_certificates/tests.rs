use super::*;
use crate::hir::BasicBlock;

fn function(symbol: &str, instructions: Vec<(ValueId, I, Type)>) -> Function {
    Function {
        symbol: symbol.into(),
        name: symbol.into(),
        params: vec![],
        return_ty: Type::Ref,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions,
            terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
        }],
        debug_info: None,
    }
}
fn producer() -> Module {
    let mut module = Module::new();
    module.arc_fixed_ancestry = true;
    for cid in [500, 600] {
        module.classes.push(crate::hir::ClassDef {
            id: cid,
            name: format!("SDK{cid}"),
            field_count: 0,
            vtable: vec![],
            to_string_symbol: None,
        });
        module.pruning_class_ids.insert(cid);
    }
    module.functions.push(function(
        "sdk_factory",
        vec![(
            ValueId(0),
            I::CallRuntime {
                name: "dartforge_object_new".into(),
                args: vec![
                    (Operand::Constant(Constant::Int(600)), Type::I64),
                    (Operand::Constant(Constant::Int(0)), Type::I64),
                ],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        )],
    ));
    let mut getter = module.functions[0].clone();
    getter.symbol = "sdk_getter".into();
    getter.params = vec![
        (ValueId(1), "this".into(), Type::Ref),
        (ValueId(2), "args".into(), Type::Ptr),
        (ValueId(3), "descriptor".into(), Type::Ptr),
    ];
    module.functions.push(getter);
    module
}
fn fixture() -> (
    tempfile::TempDir,
    Vec<String>,
    Module,
    Vec<crate::pruning::ModuleTable>,
) {
    let dir = tempfile::tempdir().unwrap();
    let artifacts = vec!["convert.o".into(), "convert.poda".into()];
    std::fs::write(dir.path().join(&artifacts[0]), "actual producer object").unwrap();

    let module = producer();
    let tables = vec![(
        500,
        "sdk_table".into(),
        vec![
            ("g:encoder".into(), "sdk_getter".into()),
            ("c:unknown".into(), "opaque".into()),
        ],
    )];
    let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
    let summary = crate::pruning::summarize(&ir, "dart:convert", &tables);
    std::fs::write(dir.path().join(&artifacts[1]), summary.to_text()).unwrap();
    let exports = export(&[&module], &analyze(&[&module]));
    let libraries = vec![crate::sdk_module::SdkLibrary {
        uri: "dart:convert".into(),
        ir,
        rejected: vec![],
        tables: tables.clone(),
        allocation_exports: exports.clone(),
    }];
    let exports = bind_final_ir(exports, &libraries);
    assert_eq!(exports.len(), 2);
    let marker = publish(dir.path(), "exact-key", true, &artifacts, &exports, &tables).unwrap();
    std::fs::write(
        dir.path().join("pronto"),
        serde_json::to_vec(&serde_json::json!({"cache_key":"exact-key","allocation_proof":marker}))
            .unwrap(),
    )
    .unwrap();
    (dir, artifacts, module, tables)
}

#[test]
fn exact_artifact_profile_and_cache_closure_binding() {
    let (dir, artifacts, _, _) = fixture();
    let imported = load(dir.path(), "exact-key", true, &artifacts);
    assert_eq!(
        imported.exports["sdk_factory"].classes,
        BTreeSet::from([600])
    );
    assert_eq!(imported.tables[&500].len(), 2);
    assert!(
        load(dir.path(), "wrong-key", true, &artifacts)
            .exports
            .is_empty()
    );
    assert!(
        load(dir.path(), "exact-key", false, &artifacts)
            .exports
            .is_empty()
    );
    for artifact in &artifacts {
        let path = dir.path().join(artifact);
        let original = std::fs::read(&path).unwrap();
        std::fs::write(&path, b"changed selected object or pruning summary").unwrap();
        assert!(
            load(dir.path(), "exact-key", true, &artifacts)
                .exports
                .is_empty()
        );
        std::fs::write(&path, original).unwrap();
    }
    assert_eq!(
        load(dir.path(), "exact-key", true, &artifacts)
            .exports
            .len(),
        2
    );
    assert!(
        load(dir.path(), "exact-key", true, &artifacts[..1])
            .exports
            .is_empty()
    );
}

#[test]
fn malformed_duplicate_stale_signature_and_table_records_fail_closed() {
    let (dir, artifacts, _, _) = fixture();
    let path = dir.path().join(FILE);
    let original = std::fs::read_to_string(&path).unwrap();
    for variant in 0..9 {
        let mut value: serde_json::Value = serde_json::from_str(&original).unwrap();
        match variant {
            0 => value["schema"] = serde_json::json!(0),
            1 => {
                let record = value["exports"][0].clone();
                value["exports"].as_array_mut().unwrap().push(record);
            }
            2 => value["exports"][0]["classes"] = serde_json::json!([600, 600]),
            3 => value["exports"][0]["closure"][0]["result"] = serde_json::json!("void"),
            4 => value["exports"][0]["closure"][0]["symbol"] = serde_json::json!("dfc.shared.get"),
            5 => value["exports"][0]["closure"][0]["body"] = serde_json::json!("unbound"),
            6 => {
                let row = value["tables"][0].clone();
                value["tables"].as_array_mut().unwrap().push(row);
            }
            7 => {
                let pair = value["tables"][0]["pairs"][0].clone();
                value["tables"][0]["pairs"]
                    .as_array_mut()
                    .unwrap()
                    .push(pair);
            }
            _ => value["unknown"] = serde_json::json!(true),
        }
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string(&value).unwrap()),
        )
        .unwrap();
        let text = std::fs::read(&path).unwrap();
        std::fs::write(dir.path().join("pronto"), serde_json::to_vec(&serde_json::json!({"cache_key":"exact-key","allocation_proof":blake3::hash(&text).to_hex().to_string()})).unwrap()).unwrap();
        assert!(
            load(dir.path(), "exact-key", true, &artifacts)
                .exports
                .is_empty(),
            "variant {variant}"
        );
    }
    std::fs::write(
        &path,
        original.replace("\"schema\":1", "\"schema\":1,\"schema\":1"),
    )
    .unwrap();
    assert!(
        load(dir.path(), "exact-key", true, &artifacts)
            .exports
            .is_empty()
    );
}

#[test]
fn imported_static_and_physical_getter_results_require_abi_and_symbol_reconciliation() {
    let (dir, artifacts, _, _) = fixture();
    let imports = load(dir.path(), "exact-key", true, &artifacts);
    let mut app = Module::new();
    app.arc_fixed_ancestry = true;
    app.imported_allocation_returns = Some(imports);
    app.functions.push(function(
        "caller",
        vec![(
            ValueId(0),
            I::CallStatic {
                symbol: "sdk_factory".into(),
                args: vec![],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        )],
    ));
    let getter_caller = function(
        "getter_caller",
        vec![
            (
                ValueId(1),
                I::AllocObject {
                    class_id: 500,
                    fields: vec![],
                },
                Type::Ref,
            ),
            (
                ValueId(0),
                I::CallSelector {
                    selector: "g:encoder".into(),
                    recv: Operand::Val(ValueId(1)),
                    args: vec![],
                    names: vec![],
                    tuple_types: Operand::Constant(Constant::Int(0)),
                },
                Type::Ref,
            ),
        ],
    );
    app.functions.push(getter_caller.clone());
    let proof = analyze(&[&app]);
    assert_eq!(proof.returns["caller"], BTreeSet::from([600]));
    assert_eq!(proof.returns["getter_caller"], BTreeSet::from([600]));
    if let I::CallStatic { args, .. } = &mut app.functions[0].blocks[0].instructions[0].1 {
        args.push(Operand::Constant(Constant::Null));
    }
    assert!(!analyze(&[&app]).returns.contains_key("caller"));
    if let I::CallStatic { args, .. } = &mut app.functions[0].blocks[0].instructions[0].1 {
        args.clear();
    }
    app.functions.push(function(
        "sdk_factory",
        vec![(ValueId(0), I::Const(Constant::Null), Type::Ref)],
    ));
    assert_eq!(analyze(&[&app]).returns["caller"], BTreeSet::from([1]));
    app.functions.pop();
    app.functions.push(function(
        "sdk_getter",
        vec![(ValueId(0), I::Const(Constant::Null), Type::Ref)],
    ));
    assert!(!analyze(&[&app]).returns.contains_key("getter_caller"));
    app.functions.pop();
    app.method_tables.push((500, "conflicting".into(), vec![]));
    assert!(!analyze(&[&app]).returns.contains_key("getter_caller"));
    app.method_tables.clear();
    app.pruning_ambiguous_class_ids.insert(500);
    assert!(!analyze(&[&app]).returns.contains_key("getter_caller"));
}

#[test]
fn edited_proof_and_non_strong_final_definitions_are_not_certificates() {
    let (dir, artifacts, producer, tables) = fixture();
    let path = dir.path().join(FILE);
    let text = std::fs::read_to_string(&path).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&text).unwrap();
    value["exports"][0]["classes"] = serde_json::json!([601]);
    std::fs::write(
        &path,
        format!("{}\n", serde_json::to_string(&value).unwrap()),
    )
    .unwrap();
    assert!(
        load(dir.path(), "exact-key", true, &artifacts)
            .exports
            .is_empty()
    );
    let exports = export(&[&producer], &analyze(&[&producer]));
    let original = crate::llvm::LlvmEmitter::new(&producer).emit_all();
    for linkage in [
        "linkonce_odr",
        "weak",
        "internal",
        "private",
        "available_externally",
    ] {
        let ir = original.replace(
            "define i64 @sdk_factory(",
            &format!("define {linkage} i64 @sdk_factory("),
        );
        let libraries = vec![crate::sdk_module::SdkLibrary {
            uri: "dart:convert".into(),
            ir,
            rejected: vec![],
            tables: tables.clone(),
            allocation_exports: exports.clone(),
        }];
        assert!(
            bind_final_ir(exports.clone(), &libraries)
                .iter()
                .all(|export| export.symbol != "sdk_factory"),
            "linkage {linkage}"
        );
    }
}
