use dartforge_emit_native::{CompileOptions, contracts::ContractMode};
use dartforge_jit::JitSession;
use std::path::PathBuf;

#[test]
#[ignore = "requires LLVM and the source-built Dart SDK"]
fn ordinary_dart_with_full_sdk_reclaims_metadata_only_generations() {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(|| {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let directory = std::env::temp_dir().join(format!(
                "dartforge-jit-plain-sdk-{}-{stamp}", std::process::id()
            ));
            std::fs::create_dir(&directory).unwrap();
            let source = directory.join("main.dart");
            let original = "int counter = 0; void main() { counter += 1; if (counter != 1) throw StateError('plain before'); print('plain:1'); }";
            std::fs::write(&source, original).unwrap();
            let sdk = std::env::var_os("DARTFORGE_TEST_SDK_LIB").map(PathBuf::from);
            let options = CompileOptions {
                contracts: ContractMode::Verify,
                sdk: sdk.as_deref(), packages: None, timings: false, optimize: false,
                language_version: None, experiments: Vec::new(), debug: false,
                generator: None, cpu: None,
            };
            let first = dartforge_emit_native::emit_reloadable_ir(&source, &options, None).unwrap();
            std::fs::write(directory.join("plain.ll"), &first.text).unwrap();
            let registrations: Vec<_> = first.text.lines()
                .filter(|line| line.contains("call void @dartforge_ffi_register_callback"))
                .collect();
            assert!(!registrations.is_empty(), "expected full SDK startup callback adapters");
            let dll = dartforge_emit_native::sdk_module::source_sdk_dll().unwrap();

            let mut simple = JitSession::new_for_ir_with(&first.text, Some(&dll)).unwrap();
            simple.add_ir_module("plain", &first.text).unwrap();
            assert_eq!(simple.run_main().unwrap().exit_code, 0);
            simple.remove_module(0).expect("SDK adapter metadata must not pin ordinary code");
            assert!(simple.module_names().is_empty());
            drop(simple);

            let mut reloadable = JitSession::new_for_ir_with(&first.text, Some(&dll)).unwrap();
            reloadable.add_reloadable_module("plain", &first.text).unwrap();
            assert_eq!(reloadable.run_reloadable_main().unwrap().exit_code, 0);
            std::fs::write(&source, original.replace("counter += 1", "counter += 2")
                .replace("counter != 1", "counter != 3")
                .replace("plain before", "plain after").replace("plain:1", "plain:3")).unwrap();
            let second = dartforge_emit_native::emit_reloadable_ir(&source, &options, Some(&first.text)).unwrap();
            // Exercise a complete replacement rather than delta reuse of
            // unchanged SDK implementations. LLVM comments change conservative
            // fingerprints without changing the actual emitted program.
            let complete_replacement = second.text.lines().map(|line| {
                if line.starts_with("define ") { format!("{line}\n; complete replacement regression") }
                else { line.to_owned() }
            }).collect::<Vec<_>>().join("\n");
            reloadable.hot_reload("plain", &complete_replacement).unwrap();
            assert_eq!(reloadable.retained_generations(), 1, "SDK adapter metadata must not retain the old generation");
            assert_eq!(reloadable.run_reloadable_main().unwrap().exit_code, 0);
            drop(reloadable);
            std::fs::remove_dir_all(directory).unwrap();
        }).unwrap().join().unwrap();
}
