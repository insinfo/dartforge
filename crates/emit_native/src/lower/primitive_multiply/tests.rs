use super::*;
use std::path::Path;
fn fixture() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/primitive_multiply.dart")
}
fn with_context(entry: &Path, edit: Option<(&str, &str, &str)>, action: impl FnOnce(&mut Context)) {
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
    context.primitive_multiply_enabled = true;
    action(&mut context);
}

#[test]
fn selected_sdk_multiply_proof_and_runtime_guards_are_real() {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(|| {
            with_context(&fixture(), None, |ctx| {
                assert!(
                    certify(ctx),
                    "canonical selected SDK must prove multiplication"
                );
                let module = crate::lower::lower_program(ctx);
                assert!(module.errors.is_empty(), "{:?}", module.errors);
                let function = module
                    .functions
                    .iter()
                    .find(|f| f.name == "multiply")
                    .unwrap();
                let instructions: Vec<_> = function
                    .blocks
                    .iter()
                    .flat_map(|b| &b.instructions)
                    .collect();
                assert!(
                    instructions
                        .iter()
                        .any(|(_, instruction, _)| matches!(instruction,
                Instruction::FMul(..)))
                );
                assert!(!instructions.iter().any(|(_, instruction, _)| matches!(instruction,
                    Instruction::CallRuntime { name, .. } if name == "dartforge_dyn_op")));
                assert!(
                    instructions
                        .iter()
                        .filter(|(_, instruction, _)| matches!(instruction,
                Instruction::CallRuntime { name, .. } if name == "dartforge_value_class"))
                        .count()
                        >= 2
                );
                assert!(
                    instructions.iter().any(|(_, instruction, _)| matches!(
                        instruction,
                        Instruction::CallSelector { .. }
                    )),
                    "unknown operands retain original selector fallback: {function:#?}"
                );
            });
            with_context(
                &fixture(),
                Some((
                    "_internal/vm/lib/integers.dart",
                    "num operator *(num other) => other._mulFromInteger(this);",
                    "num operator *(num other) => 123;",
                )),
                |ctx| {
                    assert!(!certify(ctx));
                    let module = crate::lower::lower_program(ctx);
                    let function = module
                        .functions
                        .iter()
                        .find(|f| f.name == "multiply")
                        .unwrap();
                    assert!(
                        !function
                            .blocks
                            .iter()
                            .flat_map(|b| &b.instructions)
                            .any(|(_, instruction, _)| matches!(instruction,
                Instruction::FMul(..)))
                    );
                },
            );
            with_context(
                &fixture(),
                Some(("_internal/vm/lib/double.dart", "Double_mul", "Double_add")),
                |ctx| assert!(!certify(ctx)),
            );
            let temporary = tempfile::tempdir().unwrap();
            let alias_entry = temporary.path().join("alias.dart");
            let mut source = std::fs::read_to_string(fixture()).unwrap();
            source.push_str("\n@pragma('vm:external-name', 'Double_mul')\nexternal double spoofMultiply(double other);\n");
            std::fs::write(&alias_entry, source).unwrap();
            with_context(&alias_entry, None, |ctx| assert!(!certify(ctx),
                "a program native alias cannot share the certified SDK binding"));
            with_context(&fixture(), None, |ctx| {
                assert!(certified(ctx));
                ctx.primitive_multiply_enabled = false;
                assert!(!certified(ctx), "source proof alone cannot enable the experiment");
                let module = crate::lower::lower_program(ctx);
                let function = module.functions.iter().find(|f| f.name == "multiply").unwrap();
                assert!(!function.blocks.iter().flat_map(|b| &b.instructions).any(|(_, instruction, _)|
                    matches!(instruction, Instruction::FMul(..))));
                ctx.primitive_multiply_enabled = true;
                ctx.arc_memory = true;
                assert!(!certified(ctx), "ARC is excluded even when source proof is cached");
            });
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn experimental_color_arithmetic_keeps_checked_return_box_reuse() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        let temporary = tempfile::tempdir().unwrap();
        let actual = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/checked_double_roundtrip/format.dart");
        std::fs::copy(actual, temporary.path().join("format.dart")).unwrap();
        let entry = temporary.path().join("main.dart");
        std::fs::write(&entry,
            "import 'format.dart'; void main() { print(convertFormatValue(1, Format.uint2, Format.float32)); }").unwrap();
        with_context(&entry, None, |ctx| {
            let mut module = crate::lower::lower_program(ctx);
            assert!(module.errors.is_empty(), "{:?}", module.errors);
            assert!(module.checked_double_identity);
            module.arc_fixed_ancestry = true;
            let checked_return_boxes = |module: &Module| {
                let function = module.functions.iter().find(|f| f.name == "convertFormatValue").unwrap();
                function.blocks.iter().filter(|block| {
                    let Terminator::Return(Some(Operand::Val(returned))) = &block.terminator else { return false; };
                    block.instructions.iter().any(|(value, instruction, _)| value == returned
                        && matches!(instruction, Instruction::Box { from: Type::F64, .. }))
                }).count()
            };
            assert_eq!(checked_return_boxes(&module), 8,
                "all original checked return paths remain before optimization");
            let function = module.functions.iter().find(|f| f.name == "convertFormatValue").unwrap();
            assert!(function.blocks.iter().flat_map(|b| &b.instructions).any(|(_, instruction, _)| matches!(instruction, Instruction::FMul(..))));
            crate::optimize::optimize(&mut module);
            assert_eq!(checked_return_boxes(&module), 0,
                "experimental multiplication preserves all eight original return rewrites");
        });
    }).unwrap().join().unwrap();
}
