use super::*;
use std::path::{Path, PathBuf};
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/checked_double_roundtrip/main.dart")
}
fn with_context(edit: Option<(&str, &str, &str)>, action: impl FnOnce(&mut Context)) {
    with_entry_context(&fixture(), edit, action);
}
fn with_entry_context(
    entry: &Path,
    edit: Option<(&str, &str, &str)>,
    action: impl FnOnce(&mut Context),
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
    action(&mut context);
}

#[test]
fn canonical_selected_sdk_certifies_and_modified_identity_hash_fails_closed() {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(|| {
            with_context(None, |ctx| {
                assert!(
                    certified(ctx),
                    "actual canonical SDK declarations must certify"
                );
                let mut module = crate::lower::lower_program(ctx);
                assert!(module.checked_double_identity);
                assert!(module.errors.is_empty(), "{:?}", module.errors);
                let boxes = |m: &crate::hir::Module| {
                    m.functions
                        .iter()
                        .filter(|f| f.name == "roundTrip")
                        .flat_map(|f| &f.blocks)
                        .flat_map(|b| &b.instructions)
                        .filter(|(_, i, _)| {
                            matches!(
                                i,
                                crate::hir::Instruction::Box {
                                    from: crate::hir::Type::F64,
                                    ..
                                }
                            )
                        })
                        .count()
                };
                assert!(
                    boxes(&module) > 0,
                    "ordinary lowering must contain the actual roundtrip"
                );
                module.arc_fixed_ancestry = true;
                crate::optimize::optimize(&mut module);
                assert_eq!(
                    boxes(&module),
                    0,
                    "canonical ordinary program must actually exercise the fast path: {:#?}",
                    module.functions.iter().find(|f| f.name == "roundTrip")
                );
                assert!(
                    module
                        .functions
                        .iter()
                        .filter(|f| f.name == "roundTrip")
                        .flat_map(|f| &f.blocks)
                        .flat_map(|b| &b.instructions)
                        .any(|(_, i, _)| matches!(
                            i,
                            crate::hir::Instruction::Unbox {
                                to: crate::hir::Type::F64,
                                ..
                            }
                        )),
                    "validation must remain"
                );
            });
            for edit in [
                (
                    "_internal/vm/lib/double.dart",
                    "int get _identityHashCode => hashCode;",
                    "int get _identityHashCode => 17;",
                ),
                (
                    "_internal/vm/lib/identical_patch.dart",
                    "Identical_comparison",
                    "Object_getHash",
                ),
                (
                    "_internal/vm/lib/identical_patch.dart",
                    "object._identityHashCode",
                    "17",
                ),
            ] {
                with_context(Some(edit), |ctx| {
                    assert!(
                        !certified(ctx),
                        "actual SDK replacement must invalidate proof: {edit:?}"
                    );
                    assert!(!crate::lower::lower_program(ctx).checked_double_identity);
                });
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn actual_format_roundtrip_diagnostic() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
  let temporary=tempfile::tempdir().unwrap();
  let actual=Path::new("/Volumes/ssd_dock/MyProjectsDart/dart_ui/lib/src/graphics/image/codecs/color/format.dart");
  std::fs::copy(actual,temporary.path().join("format.dart")).unwrap();
  let entry=temporary.path().join("main.dart");
  std::fs::write(&entry,"import 'format.dart'; void main() { print(convertFormatValue(1, Format.uint2, Format.float32)); }").unwrap();
  with_entry_context(&entry,None,|ctx| {
   let mut module=crate::lower::lower_program(ctx);
   assert!(module.errors.is_empty(),"{:?}",module.errors);
   module.arc_fixed_ancestry=true;
   eprintln!("MODE sdk_library={} fixed={} cert={} arc={} prepared={}",module.sdk_library,module.arc_fixed_ancestry,module.checked_double_identity,module.arc_memory,module.arc_prepared);
   crate::optimize::optimize(&mut module);
   let function=module.functions.iter().find(|f|f.name=="convertFormatValue").unwrap();
   for block in &function.blocks {
    for(v,i,t) in &block.instructions {
     let physical=crate::llvm::LlvmEmitter::result_type(i,*t);
     if physical!=*t { eprintln!("TYPE_MISMATCH block{:?} {:?} {:?}->{:?} {:?}",block.id,v,t,physical,i); }
    }
    if block.instructions.iter().any(|(_,i,_)|matches!(i,crate::hir::Instruction::Box{from:crate::hir::Type::F64,..})) {
     eprintln!("BOXBLOCK {block:#?}");
     for predecessor in &function.blocks {
      if matches!(predecessor.terminator,crate::hir::Terminator::CondBranch{else_block,..} if else_block==block.id) { eprintln!("PREDECESSOR {predecessor:#?}"); }
     }
    }
   }
  });
 }).unwrap().join().unwrap();
}
