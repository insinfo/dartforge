//! Source SDK and ordinary constant-pattern identity regressions.

use super::*;
use std::path::{Path, PathBuf};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/enum_pattern_identity/main.dart")
}

fn with_context(edit: Option<(&str, &str, &str)>, action: impl FnOnce(&mut Context)) {
    let temporary = tempfile::tempdir().unwrap();
    let mut sdk = crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();
    if let Some((file, old, new)) = edit {
        let original = sdk.root.join(file);
        let source = std::fs::read_to_string(&original).unwrap();
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
        dartforge_elements::load::load_lenient(&fixture(), &sdk, None, &mut interner);
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

fn selector_count(module: &Module, name: &str) -> usize {
    module
        .functions
        .iter()
        .find(|f| f.name == name)
        .unwrap()
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter(|(_, i, _)| {
            matches!(i,
                Instruction::CallSelector { selector, .. } if selector == "c:=="
            )
        })
        .count()
}

#[test]
fn direct_imported_enhanced_patterns_avoid_selector_but_custom_and_relational_keep_it() {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(|| {
            with_context(None, |context| {
                let module = crate::lower::lower_program(context);
                assert!(module.errors.is_empty(), "{:?}", module.errors);
                for name in ["direct", "imported", "enhanced"] {
                    assert_eq!(
                        selector_count(&module, name),
                        0,
                        "{name} must exercise the optimized constant pattern"
                    );
                    let f = module.functions.iter().find(|f| f.name == name).unwrap();
                    assert!(
                        f.blocks
                            .iter()
                            .flat_map(|b| &b.instructions)
                            .any(|(_, i, _)| matches!(
                                i,
                                Instruction::ICmp(ICmpOp::Eq, Operand::Val(_), Operand::Val(_))
                            )),
                        "{name} must compare the actual reference handles"
                    );
                }
                for name in [
                    "aliased",
                    "mixed",
                    "masked",
                    "shadowed",
                    "custom",
                    "relational",
                ] {
                    assert!(
                        selector_count(&module, name) > 0,
                        "uncertified {name} must keep ordinary equality"
                    );
                }
                context.enum_pattern_identity_enabled = false;
                let disabled = crate::lower::lower_program(context);
                for name in ["direct", "imported", "enhanced"] {
                    assert!(
                        selector_count(&disabled, name) > 0,
                        "opt-out must restore ordinary {name} equality"
                    );
                }
            })
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn changed_sdk_object_body_native_and_implicit_enum_member_fail_closed() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        for edit in [
            ("_internal/vm/lib/object_patch.dart", "external bool operator ==(Object other);", "bool operator ==(Object other) => false;"),
            ("_internal/vm/lib/object_patch.dart", "\"Object_equals\"", "\"Object_toString\""),
                ("_internal/vm/lib/object_patch.dart", "external bool operator ==(Object other);", "external bool operator ==(String other);"),
                ("core/enum.dart", "_Enum", "_EnumMissing"),
            ("core/enum.dart", "abstract class _Enum implements Enum {", "abstract class _Enum implements Enum { bool operator ==(Object other) => false;"),
            ("core/enum.dart", "abstract interface class Enum {", "abstract interface class Enum { bool operator ==(Object other) => false;"),
            ("core/enum.dart", "abstract class _Enum implements Enum {", "mixin EnumEquality { bool operator ==(Object other) => false; } abstract class _Enum with EnumEquality implements Enum {"),
        ] {
            with_context(Some(edit), |context| {
                let cid = context.program.classes.iter().position(|c| context.symbol_name(c.name) == "Color").unwrap();
                assert!(!enum_identity_operator(context, ClassId(cid as u32)), "SDK mutation must invalidate identity proof: {edit:?}");
                let module = crate::lower::lower_program(context);
                assert!(selector_count(&module, "direct") > 0, "altered SDK must retain the generic path");
            });
        }
    }).unwrap().join().unwrap();
}
