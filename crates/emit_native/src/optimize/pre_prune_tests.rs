//! Early CFG cleanup must change roots, not reachable effects or ownership.

use super::*;

fn call(symbol: &str) -> Instruction {
    Instruction::CallStatic { symbol: symbol.into(), args: vec![], ret_ty: Type::Void }
}

fn function(symbol: &str) -> Function {
    Function { symbol: symbol.into(), name: symbol.into(), params: vec![],
        return_ty: Type::Void, debug_info: None,
        blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
            terminator: Terminator::Return(None) }] }
}

fn fixture() -> Module {
    let mut module = Module::new();
    let mut entry = function("dart_main");
    entry.return_ty = Type::I64;
    entry.blocks = vec![
        BasicBlock { id: BlockId(0), instructions: vec![
            (ValueId(0), call("condition_effect"), Type::Void),
            (ValueId(1), Instruction::Const(Constant::Bool(false)), Type::I1),
        ], terminator: Terminator::CondBranch { cond: Operand::Val(ValueId(1)),
            then_block: BlockId(1), else_block: BlockId(2) } },
        BasicBlock { id: BlockId(1), instructions: vec![
            (ValueId(2), call("Dead.constructor"), Type::Void),
            (ValueId(3), Instruction::CallSelector { selector: "c:dead".into(),
                recv: Operand::Constant(Constant::Null), args: vec![], names: vec![],
                tuple_types: Operand::Constant(Constant::Int(0)) }, Type::Ref),
            (ValueId(4), Instruction::Const(Constant::String("F<0;C703;0;;>".into())), Type::Ref),
        ], terminator: Terminator::Branch(BlockId(3)) },
        BasicBlock { id: BlockId(2), instructions: vec![
            (ValueId(5), call("live_effect"), Type::Void),
        ], terminator: Terminator::Branch(BlockId(3)) },
        BasicBlock { id: BlockId(3), instructions: vec![
            (ValueId(6), Instruction::Phi { incoming: vec![
                (BlockId(1), Operand::Constant(Constant::Int(9))),
                (BlockId(2), Operand::Constant(Constant::Int(7)))], ty: Type::I64 }, Type::I64),
        ], terminator: Terminator::Return(Some(Operand::Val(ValueId(6)))) },
    ];
    module.functions = vec![entry, function("condition_effect"), function("live_effect"),
        function("Dead.constructor"), function("Dead.dead$c"), function("df.ffi.call.i_")];
    module.entry_symbol = Some("dart_main".into());
    module.sdk_records = vec!["df.register.core".into()];
    module.method_tables = vec![(315, "df.mt.Dead".into(), vec![("c:dead".into(), "Dead.dead$c".into())])];
    module.ffi_types = vec![(703, 'i')];
    module.ffi_trampolines = vec![("i_".into(), "df.ffi.call.i_".into())];
    module
}

#[test]
fn early_cfg_removes_dead_roots_preserves_effects_and_phi() {
    let mut baseline = fixture();
    crate::pruning::prune_hir(&mut baseline, &[]);
    assert!(baseline.functions.iter().any(|f| f.symbol == "Dead.constructor"));
    assert!(!baseline.ffi_trampolines.is_empty());
    assert!(!baseline.method_tables[0].2.is_empty());

    let mut module = fixture();
    prune_constant_cfg(&mut module);
    let entry = &module.functions[0];
    assert_eq!(entry.blocks.iter().map(|b| b.id.0).collect::<Vec<_>>(), vec![0, 2, 3]);
    assert!(matches!(entry.blocks[0].terminator, Terminator::Branch(BlockId(2))));
    assert!(matches!(&entry.blocks[2].instructions[0].1, Instruction::Phi { incoming, .. }
        if incoming.len() == 1 && incoming[0].0 == BlockId(2)));
    let symbols: Vec<_> = entry.blocks.iter().flat_map(|b| &b.instructions).filter_map(|(_, i, _)|
        if let Instruction::CallStatic { symbol, .. } = i { Some(symbol.as_str()) } else { None }).collect();
    assert_eq!(symbols, vec!["condition_effect", "live_effect"]);
    assert!(!module.arc_memory);
    assert!(module.functions.iter().flat_map(|f| &f.blocks).flat_map(|b| &b.instructions).all(|(_, i, _)|
        !matches!(i, Instruction::ArcCopy { .. } | Instruction::ArcDrop { .. } | Instruction::ArcMove { .. })));
    crate::pruning::prune_hir(&mut module, &[]);
    let retained: Vec<_> = module.functions.iter().map(|f| f.symbol.as_str()).collect();
    assert_eq!(retained, vec!["dart_main", "condition_effect", "live_effect"]);
    assert!(module.method_tables[0].2.is_empty());
    assert!(module.ffi_trampolines.is_empty());
}

#[test]
fn early_cfg_keeps_unknown_condition_and_throwing_producer() {
    let mut module = fixture();
    let entry = &mut module.functions[0];
    entry.params.push((ValueId(7), "condition".into(), Type::I1));
    entry.blocks[0].terminator = Terminator::CondBranch { cond: Operand::Val(ValueId(7)),
        then_block: BlockId(1), else_block: BlockId(2) };
    entry.blocks[0].instructions.pop();
    let before = format!("{module:?}");
    prune_constant_cfg(&mut module);
    // An unknown branch condition cannot discard either path or its producers.
    assert_eq!(format!("{module:?}"), before);
}

#[test]
fn ordinary_dart_dead_constructor_and_selector_leave_hir_summary() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        use dartforge_types::{CoreTypes, TypeTable};
        let entry = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/pre_prune_cfg/main.dart");
        let sdk = crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();
        let mut interner = dartforge_intern::Interner::new();
        let program = dartforge_elements::load::load(&entry, &sdk, None, &mut interner).unwrap();
        let mut table = TypeTable::new();
        let core = CoreTypes::init(&mut table, &program, &interner);
        let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
        let libraries: Vec<_> = program.libraries.iter().enumerate().filter(|(_, l)| !l.is_sdk)
            .map(|(i, _)| dartforge_elements::model::LibraryId(i as u32)).collect();
        let (bodies, errors) = dartforge_types::infer_library_bodies_with_locals(
            &program, &interner, &mut table, &core, &mut outline, &libraries);
        assert!(errors.is_empty(), "{errors:?}");
        let context = crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies).with_source_sdk();
        let mut module = crate::lower::lower_program(&context);
        assert!(module.errors.is_empty(), "{:?}", module.errors);
        let selector_present = |module: &Module| module.functions.iter().flat_map(|f| &f.blocks)
            .flat_map(|b| &b.instructions).any(|(_, i, _)|
                matches!(i, Instruction::CallSelector { selector, .. } if selector == "c:invoke"));
        assert!(selector_present(&module), "lowering must retain the unoptimized dead branch");
        let mut baseline = module.clone();
        crate::pruning::prune_hir(&mut baseline, &[]);
        assert!(baseline.functions.iter().any(|f| f.symbol.ends_with(".Dead.invoke$c")));
        prune_constant_cfg(&mut module);
        assert!(!selector_present(&module));
        crate::pruning::prune_hir(&mut module, &[]);
        assert!(!module.functions.iter().any(|f| f.symbol.ends_with(".Dead.invoke$c")));
        let strings: Vec<_> = module.functions.iter().flat_map(|f| &f.blocks)
            .flat_map(|b| &b.instructions).filter_map(|(_, i, _)| match i {
                Instruction::Const(Constant::StringWtf8(bytes)) => Some(bytes.as_slice()),
                _ => None,
            }).collect();
        assert!(strings.contains(&b"before branch".as_slice()));
        assert!(strings.contains(&b"after branch".as_slice()));
        assert!(!strings.contains(&b"dead constructor".as_slice()));
        assert!(!strings.contains(&b"dead selector".as_slice()));
    }).unwrap().join().unwrap();
}
