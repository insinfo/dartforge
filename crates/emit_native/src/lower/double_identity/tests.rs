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

fn assert_canonical_certificate(ctx: &Context) {
    if certified(ctx) {
        return;
    }
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("selected_sdk_manifest.json")).unwrap();
    eprintln!(
        "selected SDK certificate rejected: core expected={} actual={:?}; trusted_sources={} canonical_hash_binding={}",
        manifest["core_fingerprint"].as_str().unwrap(),
        selected_sdk::core_fingerprint(ctx),
        selected_sdk::trusted_sources(ctx),
        selected_sdk::canonical_hash_binding(ctx)
    );
    if selected_sdk::core_fingerprint(ctx).as_deref() != manifest["core_fingerprint"].as_str()
        && let Some(core) = ctx.program.core
    {
        let library = ctx.program.library(core);
        for (ordinal, id) in library.units.iter().enumerate() {
            let unit = ctx.program.unit(*id);
            eprintln!(
                "selected core graph: ordinal={ordinal} unit={id:?} role={:?} uri={} logical_path={:?} parent={:?}",
                unit.role,
                unit.uri,
                unit.path,
                library.unit_parents.get(id)
            );
        }
    }
    let expected: std::collections::HashSet<_> = manifest["units"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry["library"].as_str().unwrap(),
                entry["role"].as_u64().unwrap(),
                entry["source_blake3"].as_str().unwrap(),
            )
        })
        .collect();
    for unit in &ctx.program.units {
        let library = ctx.program.library(unit.library);
        if !library.is_sdk {
            continue;
        }
        let role = match unit.role {
            dartforge_elements::model::UnitRole::Library => 1,
            dartforge_elements::model::UnitRole::Part => 2,
            dartforge_elements::model::UnitRole::Patch => 3,
            dartforge_elements::model::UnitRole::Augmentation => 4,
        };
        let digest = blake3::hash(unit.source.as_bytes()).to_hex().to_string();
        if !expected.contains(&(library.uri.as_str(), role, digest.as_str())) {
            eprintln!(
                "unreviewed SDK unit: library={} role={role} uri={} source_blake3={digest} bytes={} crlf={} skipped={}",
                library.uri,
                unit.uri,
                unit.source.len(),
                unit.source
                    .as_bytes()
                    .windows(2)
                    .filter(|bytes| *bytes == b"\r\n")
                    .count(),
                unit.skipped.len()
            );
        }
    }
    panic!("actual canonical SDK declarations must certify");
}

#[test]
fn canonical_selected_sdk_certifies_and_modified_identity_hash_fails_closed() {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(|| {
            with_context(None, |ctx| {
                assert_canonical_certificate(ctx);
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
fn actual_format_widened_booleans_preserve_all_eight_checked_return_rewrites() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        let temporary = tempfile::tempdir().unwrap();
        // Exact ordinary dart_ui source retained as a portable regression fixture.
        let actual = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/checked_double_roundtrip/format.dart");
        std::fs::copy(actual, temporary.path().join("format.dart")).unwrap();
        let entry = temporary.path().join("main.dart");
        std::fs::write(&entry, "import 'format.dart'; void main() { print(convertFormatValue(1, Format.uint2, Format.float32)); }").unwrap();
        with_entry_context(&entry, None, |ctx| {
            // The checked round trip alone: the primitive multiply and clamp
            // (on by default) lower the same operators into guarded paths with
            // boxes of their own.
            ctx.primitive_multiply_enabled = false;
            ctx.primitive_clamp_enabled = false;
            let mut module = crate::lower::lower_program(ctx);
            assert!(module.errors.is_empty(), "{:?}", module.errors);
            assert!(!module.sdk_library);
            assert!(module.checked_double_identity);
            module.arc_fixed_ancestry = true;
            let counts = |module: &crate::hir::Module| {
                let f = module.functions.iter().find(|f| f.name == "convertFormatValue").unwrap();
                let mut boxes = 0;
                let mut unboxes = 0;
                let mut widened_or = 0;
                for (_, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
                    if matches!(inst, crate::hir::Instruction::Box { from: crate::hir::Type::F64, .. }) { boxes += 1; }
                    if matches!(inst, crate::hir::Instruction::Unbox { to: crate::hir::Type::F64, .. }) { unboxes += 1; }
                    if matches!(inst, crate::hir::Instruction::Or(..)) && *ty == crate::hir::Type::I1
                        && crate::llvm::LlvmEmitter::result_type(inst, *ty) == crate::hir::Type::I64 { widened_or += 1; }
                }
                (boxes, unboxes, widened_or)
            };
            let before = counts(&module);
            assert_eq!(before.0, 8);
            assert!(before.2 > 0, "actual source must exercise canonical widened boolean guards");
            crate::optimize::optimize(&mut module);
            let after = counts(&module);
            assert_eq!(after.0, 0, "all eight actual checked double return paths must be optimized");
            assert_eq!(after.1, before.1, "original checks remain intact");
            assert_eq!(after.2, before.2, "unrelated predicates remain intact");
        });
    }).unwrap().join().unwrap();
}

#[test]
#[ignore = "Writes a reviewable selected SDK baseline manifest to an explicit SSD output"]
fn dump_selected_sdk_baseline_for_review() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        let temporary = tempfile::tempdir().unwrap();
        let entry = temporary.path().join("main.dart");
        let mut source = String::new();
        for name in crate::sdk_module::SOURCE_LIBRARIES { source.push_str(&format!("import 'dart:{name}';\n")); }
        source.push_str("void main() {}\n");
        std::fs::write(&entry, source).unwrap();
        with_entry_context(&entry, None, |ctx| {
            let core = selected_sdk::core_fingerprint(ctx).expect("canonical core relation proof");
            let mut units = Vec::new();
            for unit in &ctx.program.units {
                let library = ctx.program.library(unit.library);
                if !library.is_sdk { continue; }
                let role = match unit.role {
                    dartforge_elements::model::UnitRole::Library => 1,
                    dartforge_elements::model::UnitRole::Part => 2,
                    dartforge_elements::model::UnitRole::Patch => 3,
                    _ => panic!("baseline must not contain augmentation"),
                };
                assert!(unit.skipped.is_empty());
                units.push(serde_json::json!({"library":library.uri,"role":role,"source_blake3":blake3::hash(unit.source.as_bytes()).to_hex().to_string()}));
            }
            units.sort_by_cached_key(|u| u.to_string());
            let manifest = serde_json::json!({"schema":1,"baseline":"Reviewed SDK3.6.2 selected sources with DartForge native overlays; no version/path check substitutes these content hashes.","core_fingerprint":core,"units":units});
            if let Some(output) = std::env::var_os("DARTFORGE_SELECTED_SDK_REVIEW_OUTPUT") {
                let output = PathBuf::from(output);
                assert!(output.starts_with("/Volumes/ssd_dock/"));
                std::fs::write(output, serde_json::to_string_pretty(&manifest).unwrap() + "\n").unwrap();
            } else {
                let reviewed: serde_json::Value = serde_json::from_str(include_str!("selected_sdk_manifest.json")).unwrap();
                assert_eq!(manifest, reviewed, "full gate validates the reviewed baseline without writing any source");
                assert!(selected_sdk::trusted_sources(ctx));
                assert!(selected_sdk::canonical_hash_binding(ctx));
            }
        });
    }).unwrap().join().unwrap();
}

#[test]
fn inherited_and_ambient_sdk_observers_fail_closed() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        for edit in [
            ("_internal/vm/lib/object_patch.dart", "throw new NoSuchMethodError.withInvocation(this, invocation);", "return _getHash(this);"),
            ("_internal/vm/lib/object_patch.dart", "external int _getHash(obj);", "external int _getHash(obj); int _ambientObserver(Object value) => _getHash(value);"),
            ("_internal/vm/lib/internal_patch.dart", "(object is num) ||", "false ||"),
        ] {
            with_context(Some(edit), |ctx| {
                assert!(!selected_sdk::trusted_sources(ctx), "modified ambient SDK source must not certify: {edit:?}");
                assert!(!certified(ctx));
                let mut module = crate::lower::lower_program(ctx);
                assert!(!module.checked_double_identity);
                module.arc_fixed_ancestry = true;
                crate::optimize::optimize(&mut module);
                let function = module.functions.iter().find(|f| f.name == "roundTrip").unwrap();
                assert!(function.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i, crate::hir::Instruction::Box { from: crate::hir::Type::F64, .. })));
            });
        }
        let temporary = tempfile::tempdir().unwrap();
        let entry = temporary.path().join("main.dart");
        let source = std::fs::read_to_string(fixture()).unwrap();
        std::fs::write(&entry, source + "\n@pragma('vm:external-name', 'Object_getHash') external int extraHeaderIdentity(Object value);\n").unwrap();
        with_entry_context(&entry, None, |ctx| {
            assert!(selected_sdk::trusted_sources(ctx), "the SDK itself remains canonical in the alias test");
            assert!(!selected_sdk::canonical_hash_binding(ctx), "native alias outside the actual canonical helper must reject");
            assert!(!certified(ctx));
        });
    }).unwrap().join().unwrap();
}

fn with_loaded_context(
    program: &dartforge_elements::Program,
    interner: &dartforge_intern::Interner,
    action: impl FnOnce(&Context),
) {
    let mut table = dartforge_types::TypeTable::new();
    let core = dartforge_types::CoreTypes::init(&mut table, program, interner);
    let (mut outline, _) = dartforge_types::resolve_outline(program, interner, &mut table, &core);
    let libraries: Vec<_> = program
        .libraries
        .iter()
        .enumerate()
        .filter(|(_, l)| !l.is_sdk)
        .map(|(i, _)| dartforge_elements::model::LibraryId(i as u32))
        .collect();
    let (bodies, _) = dartforge_types::infer_library_bodies_with_locals(
        program,
        interner,
        &mut table,
        &core,
        &mut outline,
        &libraries,
    );
    let ctx = Context::new(program, interner, &table, &core, &outline, &bodies).with_source_sdk();
    action(&ctx);
}

fn copy_tree(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let dest = destination.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &dest);
        } else {
            std::fs::copy(entry.path(), dest).unwrap();
        }
    }
}

#[test]
fn copied_sdk_and_cold_warm_parts_have_same_reviewed_identity() {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(|| {
            let temporary = tempfile::tempdir().unwrap();
            let copy = temporary.path().join("copied-sdk-lib");
            copy_tree(crate::sdk_tests(), &copy);
            let sdk = crate::sdk_module::load_native_sdk(&copy).unwrap();
            let mut cold_interner = dartforge_intern::Interner::new();
            let (mut cold, diagnostics) =
                dartforge_elements::load::load_lenient(&fixture(), &sdk, None, &mut cold_interner);
            assert!(diagnostics.is_empty());
            let mut cold_fingerprint = None;
            with_loaded_context(&cold, &cold_interner, |ctx| {
                assert!(
                    certified(ctx),
                    "copied paths must not affect source identity"
                );
                cold_fingerprint = selected_sdk::core_fingerprint(ctx);
                assert!(
                    !ctx.program
                        .library(ctx.program.core.unwrap())
                        .unit_parents
                        .is_empty()
                );
            });
            let mut cache_sdk = sdk.clone();
            cache_sdk
                .libraries
                .retain(|name, _| crate::sdk_module::SOURCE_LIBRARIES.contains(&name.as_str()));
            let cache = dartforge_elements::SdkCache::build_and_write(
                &cache_sdk,
                &temporary.path().join("sdk-cache.bin"),
            )
            .unwrap();
            let mut warm_interner = dartforge_intern::Interner::new();
            let (warm, diagnostics) = dartforge_elements::load::load_lenient_with_cache(
                &fixture(),
                &sdk,
                None,
                &mut warm_interner,
                Some(cache),
            );
            assert!(diagnostics.is_empty());
            with_loaded_context(&warm, &warm_interner, |ctx| {
                assert!(
                    ctx.program
                        .library(ctx.program.core.unwrap())
                        .unit_parents
                        .is_empty(),
                    "actual cached loader omits the parent map"
                );
                assert_eq!(
                    selected_sdk::core_fingerprint(ctx),
                    cold_fingerprint,
                    "derive parsed-source relations without hashing SDK paths"
                );
                assert!(certified(ctx));
            });
            let core = cold.core.unwrap();
            let parent = cold.library(core).units[0];
            let patch = cold.library(core).units[1];
            cold.libraries[core.0 as usize]
                .unit_parents
                .insert(patch, parent);
            with_loaded_context(&cold, &cold_interner, |ctx| {
                assert!(
                    selected_sdk::core_fingerprint(ctx).is_none(),
                    "an explicit fabricated parent must not be ignored"
                );
                assert!(!certified(ctx));
            });
            cold.libraries[core.0 as usize].unit_parents.remove(&patch);
            cold.units[parent.0 as usize]
                .skipped
                .push(dartforge_diagnostics::Span { start: 0, end: 1 });
            with_loaded_context(&cold, &cold_interner, |ctx| {
                assert!(
                    selected_sdk::core_fingerprint(ctx).is_none(),
                    "parse-recovery inputs cannot certify"
                );
                assert!(!certified(ctx));
            });
        })
        .unwrap()
        .join()
        .unwrap();
}
