//! Ordinary source integration for complete-table getter return facts.

use super::*;

#[test]
fn actual_sdk_dynamic_codec_getter_results_have_body_proved_classes() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
        use dartforge_types::table::{CoreTypes, TypeTable};
        let directory = tempfile::tempdir().unwrap();
        let entry = directory.path().join("main.dart");
        std::fs::write(&entry, r#"
import 'dart:convert';
void main() {
  dynamic ascii = const AsciiCodec();
  dynamic utf8 = const Utf8Codec();
  dynamic base64Codec = const Base64Codec();
  print(ascii.encoder.convert('ASCII'));
  print(utf8.encoder.convert('UTF8'));
  print(base64Codec.encoder.convert([65]));
}
"#).unwrap();
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
        let (bodies, _) = dartforge_types::infer_library_bodies(&program, &interner, &mut table,
            &core, &mut outline, &libraries);
        let mut context = crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies)
            .with_source_sdk();
        // Unified body-availability witness: app HIR normally links SDK modules
        // separately. This test supplies each inferred body once without COMDAT duplicates.
        for library in &libraries { context.in_module[library.0 as usize] = true; }
        let mut module = crate::lower::lower_program(&context);
        assert!(module.errors.is_empty(), "{:?}", module.errors);
        module.arc_fixed_ancestry = true;
        crate::optimize::prepare_for_emission(&mut module, false, false);
        let modules = [module];
        let proof = analyze(&modules.iter().collect::<Vec<_>>());
        let physical_class = |name: &str| modules.iter().flat_map(|module| &module.classes)
            .find(|class| class.name == name).unwrap().id;
        let ascii_encoder = physical_class("AsciiEncoder");
        let utf8_encoder = physical_class("Utf8Encoder");
        let main = modules.iter().flat_map(|module| &module.functions)
            .find(|function| function.symbol == "dart_main" || function.name == "main").unwrap();
        let values = proof.values_for(main).expect("main body witness");
        let getters: Vec<_> = main.blocks.iter().flat_map(|block| &block.instructions)
            .filter_map(|(value, instruction, _)| getter(instruction)
                .filter(|(selector, _)| *selector == "g:encoder").map(|(_, receiver)| (*value, receiver)))
            .collect();
        assert_eq!(getters.len(), 3, "{:?}", main.blocks);
        for ((value, _), expected) in getters.iter().take(2).zip([ascii_encoder, utf8_encoder]) {
            let classes = values.get(value).unwrap_or_else(|| panic!("unproved getter result {value:?}: {values:?}"));
            assert!(classes.contains(&expected), "{classes:?} lacks physical encoder {expected}");
            assert!(classes.iter().all(|cid| *cid == expected || *cid == 1), "unexpected getter classes {classes:?}");
        }
        // A field-read getter needs the independent closed-writer/context proof.
        assert!(!values.contains_key(&getters[2].0));
        let convert_receivers: Vec<_> = main.blocks.iter().flat_map(|block| &block.instructions)
            .filter_map(|(_, instruction, _)| match instruction {
                I::CallSelector { selector, recv: Operand::Val(receiver), .. }
                    if selector.ends_with("convert") => Some(*receiver),
                _ => None,
            }).collect();
        assert!(convert_receivers.iter().any(|receiver| values.get(receiver)
            .is_some_and(|classes| classes.contains(&ascii_encoder))));
        assert!(convert_receivers.iter().any(|receiver| values.get(receiver)
            .is_some_and(|classes| classes.contains(&utf8_encoder))));
    }).unwrap().join().unwrap();
}
