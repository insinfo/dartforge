//! An explicit SDK supplies both frontend declarations and the linked SDK bodies.

use dartforge_emit_native::{CompileOptions, compile_with};
use std::path::Path;

fn copy_directory(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = target.join(entry.file_name());
        if entry.path().is_dir() {
            copy_directory(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
#[ignore = "requires native SDK, runtime archive and Clang"]
fn selected_sdk_wrapper_survives_conflicting_discovery_and_warm_cache() {
    let discovered = dartforge_emit_native::dart_sdk().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let selected = directory.path().join("selected-sdk/lib");
    copy_directory(&discovered, &selected);
    let original_file = discovered.join("io/platform_impl.dart");
    let original = std::fs::read_to_string(&original_file).unwrap();
    let marker = "static String get operatingSystem => _operatingSystem();";
    assert_eq!(original.matches(marker).count(), 1);
    let changed = original.replace(marker,
        "static String get operatingSystem { print('selected sdk wrapper'); return _operatingSystem(); }");
    std::fs::write(selected.join("io/platform_impl.dart"), changed).unwrap();
    assert_ne!(selected, dartforge_emit_native::dart_sdk().unwrap());
    let entry = directory.path().join("main.dart");
    std::fs::write(&entry, "import 'dart:io'; void main() { print(Platform.operatingSystem); print(Platform.isWindows); print(Platform.isMacOS); }").unwrap();
    for warm in [false, true] {
        let entry = entry.clone();
        let selected = selected.clone();
        let binary = directory.path().join(if warm { "warm" } else { "cold" });
        let result = binary.clone();
        std::thread::Builder::new().stack_size(64 << 20).spawn(move || {
            let options = CompileOptions {
                contracts: dartforge_emit_native::contracts::ContractMode::Verify,
                sdk: Some(&selected), packages: None, timings: false, optimize: true,
                language_version: None, experiments: Vec::new(), debug: false, generator: None, cpu: None,
            };
            compile_with(&entry, &result, &options).unwrap();
        }).unwrap().join().unwrap();
        let output = std::process::Command::new(&binary).output().unwrap();
        assert!(output.status.success() && output.stderr.is_empty(), "{}", String::from_utf8_lossy(&output.stderr));
        let expected = format!("selected sdk wrapper\n{}\n{}\n{}\n", std::env::consts::OS,
            cfg!(target_os = "windows"), cfg!(target_os = "macos"));
        assert_eq!(String::from_utf8(output.stdout).unwrap().replace("\r\n", "\n"), expected);
    }
    assert_eq!(std::fs::read_to_string(original_file).unwrap(), original);
}
