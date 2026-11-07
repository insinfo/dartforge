//! Actual VM comparisons for immutable native declarations, including SDK selection.
use dartforge_emit_native::{CompileOptions, DartFlags};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dartforge-native-environment-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn stdout(output: Output) -> Vec<u8> {
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .replace("\r\n", "\n")
        .into_bytes()
}
const PROGRAM: &str = r#"
String key(String value) => value;
dynamic mark(String label, dynamic value) { print(label); return value; }
void main() {
  print(const String.fromEnvironment('tag', defaultValue: 'absent'));
  print(const int.fromEnvironment('n', defaultValue: -7));
  print(const bool.fromEnvironment('dart.library.io'));
  print(const bool.hasEnvironment('dart.library.unknown'));
  print(const bool.hasEnvironment('dart.library.mirrors'));
  print(const bool.hasEnvironment('dart.developer.timeline'));
  for (final name in ['tag', 'n', 'dart.library.io', 'dart.library.unknown', 'dart.library.mirrors', 'dart.library._internal', 'dart.developer.timeline']) {
    print(name);
    print(String.fromEnvironment(key(name), defaultValue: 'absent'));
    print(bool.hasEnvironment(key(name)));
    print(int.fromEnvironment(key(name), defaultValue: -7));
  }
  final stringFactory = String.fromEnvironment;
  final integerFactory = int.fromEnvironment;
  final presenceFactory = bool.hasEnvironment;
  print(stringFactory(key('tag'), defaultValue: 'absent'));
  print(integerFactory(key('n'), defaultValue: -7));
  print(presenceFactory(key('dart.library.unknown')));
  try { print(String.fromEnvironment(mark('name first', 42), defaultValue: mark('must not run', 'default'))); }
  on TypeError { print('name type error'); }
  try { print(String.fromEnvironment(defaultValue: mark('default first', 42), mark('name later', 'tag'))); }
  on TypeError { print('default type error'); }
}
"#;
fn sdk_dll(ir: &str, sdk: &Path) -> PathBuf {
    let environment = dartforge_emit_native::ir_environment(ir).unwrap();
    dartforge_emit_native::sdk_module::compiled_sdk_for(
        sdk,
        &dartforge_emit_native::driver::NativeDriverOptions::default().clang,
        dartforge_emit_native::sdk_module::SdkProfile::Development,
        DartFlags::DEVELOPMENT,
        environment.as_ref(),
    )
    .unwrap()
    .dll
}
fn compare(product: bool) {
    let scratch = Scratch::new();
    let source = scratch.0.join("main.dart");
    std::fs::write(&source, PROGRAM).unwrap();
    let root =
        PathBuf::from(std::env::var_os("DARTFORGE_DART_SDK").expect("Dart 3.6.2 oracle SDK"));
    let dart = root.join(if cfg!(windows) {
        "bin/dart.exe"
    } else {
        "bin/dart"
    });
    let sdk = root.join("lib");
    let cases = [
        vec![
            "tag=first",
            "n=0xffffffffffffffff",
            "dart.library.io=false",
            "dart.library.unknown=false",
        ],
        vec![
            "tag=second",
            "n=+42",
            "dart.library.io=true",
            "dart.library.unknown=true",
        ],
        vec![
            "tag=first",
            "n=0xffffffffffffffff",
            "dart.library.io=false",
            "dart.library.unknown=false",
        ],
    ];
    for (index, definitions) in cases.into_iter().enumerate() {
        let mut reference = Command::new(&dart);
        let official_exe = scratch.0.join(format!("official-{index}.exe"));
        if product {
            reference.args(["compile", "exe"]);
        } else {
            reference.arg("--enable-asserts");
        }
        for value in &definitions {
            reference.arg(format!("-D{value}"));
        }
        reference.arg(&source);
        let expected = if product {
            let result = reference.arg("-o").arg(&official_exe).output().unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            stdout(Command::new(&official_exe).output().unwrap())
        } else {
            stdout(reference.output().unwrap())
        };
        let candidate_exe = scratch.0.join(format!("candidate-{index}.exe"));
        let source = source.clone();
        let sdk = sdk.clone();
        let definitions = definitions.into_iter().map(str::to_string).collect();
        let exe = candidate_exe.clone();
        let ir = std::thread::Builder::new()
            .stack_size(1 << 30)
            .spawn(move || {
                let options = CompileOptions {
                    contracts: dartforge_emit_native::contracts::ContractMode::Verify,
                    sdk: Some(&sdk),
                    packages: None,
                    timings: false,
                    optimize: product,
                    defines: definitions,
                    language_version: None,
                    experiments: Vec::new(),
                    debug: false,
                    generator: None,
                    cpu: None,
                };
                if product {
                    dartforge_emit_native::compile_with_flags(
                        &source,
                        &exe,
                        &options,
                        DartFlags::PRODUCTION,
                    )
                    .unwrap();
                    None
                } else {
                    let emitted =
                        dartforge_emit_native::emit_reloadable_ir(&source, &options, None).unwrap();
                    let dll = sdk_dll(&emitted.text, &sdk);
                    Some((emitted.text, dll))
                }
            })
            .unwrap()
            .join()
            .unwrap();
        let output = if let Some((ir, dll)) = ir {
            let file = scratch.0.join(format!("candidate-{index}.ll"));
            std::fs::write(&file, ir).unwrap();
            Command::new(env!("CARGO_BIN_EXE_dartforge-run-ir"))
                .arg(file)
                .env("DARTFORGE_SDK_DLL", dll)
                .output()
                .unwrap()
        } else {
            Command::new(&candidate_exe).output().unwrap()
        };
        assert_eq!(stdout(output), expected, "product={product}, case={index}");
    }
}
#[test]
#[ignore = "requires Dart 3.6.2, LLVM and the source SDK toolchain"]
fn jit_declarations_and_runtime_queries_match_the_vm_across_contexts() {
    compare(false);
}
#[test]
#[ignore = "requires Dart 3.6.2, LLVM and the source SDK toolchain"]
fn aot_declarations_and_runtime_queries_match_the_vm_across_contexts() {
    compare(true);
}
