//! Actual selected-source and native-binding certificate regressions.
use super::*;
use crate::hir::*;
use std::path::Path;

fn fixture() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/primitive_clamp.dart")
}
fn with_context(entry: &Path, edit: Option<(&str, &str, &str)>, action: impl FnOnce(&mut Context)) {
    let temporary = tempfile::tempdir().unwrap();
    let mut sdk = crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();
    if let Some((file, old, new)) = edit {
        let original = sdk.root.join(file);
        let selected = sdk.replacements.get(&original).unwrap_or(&original);
        let source = std::fs::read_to_string(selected).unwrap();
        assert!(source.contains(old));
        let modified = temporary.path().join("sdk-edit.dart");
        std::fs::write(&modified, source.replace(old, new)).unwrap();
        sdk.replacements.insert(original, modified);
    }
    let mut interner = dartforge_intern::Interner::new();
    let (program, diagnostics) =
        dartforge_elements::load::load_lenient(entry, &sdk, None, &mut interner);
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
        Context::new(&program, &interner, &table, &core, &outline, &bodies).with_source_sdk();
    context.primitive_clamp_enabled = true;
    action(&mut context);
}

#[test]
fn actual_literal_clamp_uses_selected_sources_and_preserves_fallback_checks() {
    std::thread::Builder::new().stack_size(64<<20).spawn(||with_context(&fixture(),None,|context| {
        assert!(context.program.functions.iter().any(|function|crate::natives::pragmas(context.program,context.interner,function).0.as_deref()==Some("Integer_fromEnvironment")),"the real source fixture retains an unrelated native binding outside the clamp closure");
        assert!(certify(context),"selected SDK must certify the actual clamp/compareTo closures");
        let mut module=crate::lower::lower_program(context);assert!(module.errors.is_empty(),"{:?}",module.errors);
        assert!(module.primitive_clamp_certificate,"normal opt-in lowering publishes the premise after every internal join");assert_eq!(module.primitive_clamp_targets,targets(context));module.arc_fixed_ancestry=true;
        assert_eq!(crate::optimize::primitive_clamp::rewrite(&mut module),0,"unpruned FFI exposure is never exempted by name");
        crate::pruning::prune_hir(&mut module,&[]);
        let original=module.functions.iter().find(|f|f.name=="unitClamp").unwrap().clone();
        let pending=|function:&Function|function.blocks.iter().flat_map(|b|&b.instructions).filter(|(_,i,_)|matches!(i,Instruction::CallRuntime{name,..}if name=="dartforge_exception_pending")).count();
        assert!(pending(&original)>0);
        assert!(crate::optimize::primitive_clamp::rewrite(&mut module)>0);
        let changed=module.functions.iter().find(|f|f.name=="unitClamp").unwrap();
        assert_eq!(pending(changed),pending(&original),"all original pending checks remain after result join");
        assert!(changed.blocks.iter().flat_map(|b|&b.instructions).any(|(_,i,_)|matches!(i,Instruction::Bitcast{to:Type::I64,..})));
        assert!(changed.blocks.iter().flat_map(|b|&b.instructions).any(|(_,i,_)|matches!(i,Instruction::CallSelector{selector,..}if selector=="c:clamp")));
        let generic=module.functions.iter().find(|f|f.name=="genericClamp").unwrap();
        assert!(!generic.blocks.iter().flat_map(|b|&b.instructions).any(|(_,i,_)|matches!(i,Instruction::Bitcast{..})),"unknown bounds stay generic");
        let typed=module.functions.iter().find(|f|f.name=="typedUnitClamp").unwrap();
        assert!(typed.blocks.iter().flat_map(|b|&b.instructions).any(|(_,i,_)|matches!(i,Instruction::Bitcast{..})),"actual typed SDK direct calls must also exercise the optimization");
    })).unwrap().join().unwrap();
}

#[test]
fn changed_sdk_clamp_comparison_and_program_native_alias_deny_the_premise() {
    std::thread::Builder::new().stack_size(64<<20).spawn(|| {
        for edit in [
            ("_internal/vm/lib/double.dart","if (lowerLimit.isNaN) return lowerLimit;","if (lowerLimit.isNaN) return this;"),
            ("_internal/vm/lib/integers.dart","if (this < lowerLimit) return lowerLimit;","if (this < lowerLimit) return this;"),
            ("_internal/vm/lib/double.dart","Double_getIsNegative","Double_getIsNaN"),
            ("_internal/vm/lib/double.dart","return thisIsNegative ? LESS : GREATER;","return thisIsNegative ? GREATER : LESS;"),
        ] { with_context(&fixture(),Some(edit),|context|assert!(!certify(context),"SDK edit must deny the premise: {edit:?}")); }
        let temporary=tempfile::tempdir().unwrap();let entry=temporary.path().join("alias.dart");
        let mut source=std::fs::read_to_string(fixture()).unwrap();source.push_str("\n@pragma('vm:external-name', 'Double_getIsNegative') external bool spoof(double value);\n");
        std::fs::write(&entry,source).unwrap();with_context(&entry,None,|context|assert!(!certify(context),"a program alias cannot share the certified native binding"));
        let ffi_entry=temporary.path().join("ffi-alias.dart");
        let mut foreign_source=std::fs::read_to_string(fixture()).unwrap();foreign_source.push_str("\n@pragma('vm:external-name', 'Ffi_dl_processLibrary') external dynamic foreignLibrary();\n");
        std::fs::write(&ffi_entry,foreign_source).unwrap();with_context(&ffi_entry,None,|context| {
            assert!(!certify(context),"an ordinary source native alias exposes FFI ABI outside the numeric call closure");
            assert!(!crate::lower::lower_program(context).primitive_clamp_certificate,"normal lowering cannot publish a premise for a foreign-native program");
        });
        with_context(&fixture(),None,|context| {
            context.primitive_clamp_enabled=false;
            assert!(!enabled_certificate(context),"source proof alone cannot enable the experiment");
            assert!(!crate::lower::lower_program(context).primitive_clamp_certificate);
        });
    }).unwrap().join().unwrap();
}
