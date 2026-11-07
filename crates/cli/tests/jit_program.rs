//! The AOT × JIT matrix of the program operations (J06), through
//! `dartforge run`: `Platform.script` is the `.dart`, `Isolate.spawnUri` is
//! rejected with the JIT's message and `Isolate.spawn` works.
#![cfg(feature = "jit")]

use std::process::Command;

#[test]
#[ignore = "requires Dart 3.6.2, LLVM and the source-built SDK"]
fn optimized_aot_ir_keeps_sdk_execution_flags_when_run_by_the_jit() {
    use std::path::{Path, PathBuf};
    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            if std::thread::panicking() {
                eprintln!("failed optimized IR artifacts: {}", self.0.display());
            } else {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }
    fn execute(directory: &Path, name: &str, command: &mut Command) -> Vec<u8> {
        command.env_remove("DARTFORGE_SDK_DLL");
        let result = command.output().unwrap();
        std::fs::write(directory.join(format!("{name}.stdout")), &result.stdout).unwrap();
        std::fs::write(directory.join(format!("{name}.stderr")), &result.stderr).unwrap();
        std::fs::write(
            directory.join(format!("{name}.status")),
            format!("{}\n", result.status),
        )
        .unwrap();
        assert!(
            result.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        result.stdout
    }
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let scratch = Scratch(
        target
            .join("test-artifacts")
            .join(format!("optimized-ir-{}-{stamp}", std::process::id())),
    );
    std::fs::create_dir_all(&scratch.0).unwrap();
    let source = scratch.0.join("main.dart");
    std::fs::write(
        &source,
        r#"
import 'dart:async';
import 'dart:convert';
Future<void> main(List<String> args) async {
  var assertions = false;
  assert(assertions = true);
  print(assertions ? 'asserts on' : 'asserts off');
  print(const String.fromEnvironment('tag', defaultValue: 'absent'));
  print(String.fromEnvironment('tag', defaultValue: 'absent'));
  final values = List<int>.generate(4, (i) => i + 1);
  print(jsonEncode(values.map((value) => value * 3).toList()));
  try { (values as dynamic).add('wrong'); } on TypeError { print('checked'); }
  print(await Future<int>.value(values.fold<int>(0, (sum, value) => sum + value)));
  print(args.join('|'));
}
"#,
    )
    .unwrap();
    let sdk = PathBuf::from(std::env::var_os("DARTFORGE_DART_SDK").expect("Dart 3.6.2 oracle SDK"));
    let dart = sdk.join(if cfg!(windows) {
        "bin/dart.exe"
    } else {
        "bin/dart"
    });
    for assertions in [false, true] {
        let case = scratch.0.join(if assertions {
            "asserts-on"
        } else {
            "asserts-off"
        });
        std::fs::create_dir(&case).unwrap();
        let exe = case.join("official.exe");
        let mut reference = Command::new(&dart);
        reference.args(["compile", "exe", "-Dtag=selected"]);
        if assertions {
            reference.arg("--enable-asserts");
        }
        execute(
            &case,
            "official-compile",
            reference.arg(&source).arg("-o").arg(&exe),
        );
        let expected = execute(
            &case,
            "official-run",
            Command::new(&exe).args(["-Dapplication", "value"]),
        );
        let expected_text = format!(
            "asserts {}\nselected\nabsent\n[3,6,9,12]\nchecked\n10\n-Dapplication|value\n",
            if assertions { "on" } else { "off" }
        );
        assert_eq!(
            String::from_utf8_lossy(&expected).replace("\r\n", "\n"),
            expected_text
        );
        let ir = case.join("program.ll");
        let mut emission = Command::new(env!("CARGO_BIN_EXE_dartforge"));
        emission
            .arg("compile-native")
            .arg(&source)
            .args(["--emit-ir", "--optimize", "-Dtag=selected"])
            .arg("--sdk")
            .arg(sdk.join("lib"))
            .arg("-o")
            .arg(&ir);
        if assertions {
            emission.arg("--enable-asserts");
        }
        execute(&case, "candidate-compile", &mut emission);
        let text = std::fs::read_to_string(&ir).unwrap();
        assert_eq!(
            dartforge_emit_native::ir_dart_flags(&text).unwrap(),
            Some(dartforge_emit_native::DartFlags {
                enable_asserts: assertions,
                product: true
            })
        );
        let actual = execute(
            &case,
            "candidate-run",
            Command::new(env!("CARGO_BIN_EXE_dartforge"))
                .args(["run", "--sdk"])
                .arg(sdk.join("lib"))
                .arg("--ir")
                .arg(&ir)
                .args(["--", "-Dapplication", "value"]),
        );
        assert!(
            std::fs::read(case.join("candidate-run.stderr"))
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            String::from_utf8(actual).unwrap().replace("\r\n", "\n"),
            expected_text
        );
    }
}

#[test]
#[ignore = "requires the full LLVM distribution and source-built SDK DLL"]
fn explicit_sdk_selects_the_jit_library_in_run_reload_and_restart() {
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let selected = dartforge_emit_native::dart_sdk().expect("source SDK");
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let scratch =
        Scratch(std::env::temp_dir().join(format!("dartforge-explicit-jit-sdk-{unique}")));
    std::fs::create_dir_all(&scratch.0).unwrap();
    // Discovery accepts this directory, but it cannot compile an SDK library.
    // Successful execution must use the explicit selection instead.
    std::fs::write(scratch.0.join("libraries.json"), "not a library manifest").unwrap();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/jit_main_arguments.dart");
    for mode in ["run", "reload", "restart"] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_dartforge"));
        command
            .arg(if mode == "run" { "run" } else { "reload" })
            .arg("--sdk")
            .arg(&selected)
            .arg(&fixture);
        if mode != "run" {
            command.arg("--once");
        }
        if mode == "restart" {
            command.arg("--restart");
        }
        let output = command
            .args(["--", "--sdk", "application-owned", "--"])
            .env("DARTFORGE_SDK_LIB", &scratch.0)
            .env_remove("DART_SDK")
            .env_remove("DARTFORGE_SDK_DLL")
            .output()
            .expect("CLI");
        assert!(
            output.status.success(),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
            "[\"--sdk\",\"application-owned\",\"--\"]\n",
            "{mode}"
        );
    }
}

#[test]
#[ignore = "compiles the source-built SDK DLL; run in the sdk-fonte job of the Pesado workflow"]
fn script_and_spawn_uri_in_the_jit() {
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/jit_script.dart");
    let dll = dartforge_emit_native::sdk_module::source_sdk_dll().expect("source-built SDK DLL");
    let output = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("run")
        .arg(&fixture)
        .env("DARTFORGE_SDK_DLL", dll)
        .output()
        .expect("CLI");
    let stdout = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");
    assert_eq!(
        stdout,
        "jit_script.dart\ntrue\ndo filho\n",
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success());
}

#[test]
#[ignore = "requires the full LLVM distribution and source-built SDK DLL"]
fn explicit_main_arguments_reach_run_reload_supervisor_and_restart() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/jit_main_arguments.dart");
    let dll = dartforge_emit_native::sdk_module::source_sdk_dll().expect("source-built SDK DLL");
    for mode in ["run", "reload", "restart"] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_dartforge"));
        command
            .arg(if mode == "run" { "run" } else { "reload" })
            .arg(&fixture);
        if mode != "run" {
            command.arg("--once");
        }
        if mode == "restart" {
            command.arg("--restart");
        }
        command
            .args(["--", "--timings", "--child", "", "é folder", "--"])
            .env("DARTFORGE_SDK_DLL", &dll);
        let output = command.output().expect("CLI");
        assert!(
            output.status.success(),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");
        assert_eq!(
            stdout,
            "[\"--timings\",\"--child\",\"\",\"é folder\",\"--\"]\n",
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
#[ignore = "requires the full LLVM distribution and source-built SDK DLL"]
fn selected_package_resources_resolve_from_project_cwd_in_all_jit_modes() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("dartforge-jit-package-resource-{unique}"));
    let project = root.join("project");
    let config = project.join(".dart_tool/package_config.json");
    std::fs::create_dir_all(project.join("bin")).unwrap();
    std::fs::create_dir_all(project.join(".dart_tool")).unwrap();
    std::fs::create_dir_all(root.join("dependency/lib")).unwrap();
    std::fs::write(
        root.join("dependency/lib/resource.txt"),
        "selected package resource\n",
    )
    .unwrap();
    std::fs::write(&config, r#"{"configVersion":2,"packages":[{"name":"resource_package","rootUri":"../../dependency/","packageUri":"lib/","languageVersion":"3.0"}]}"#).unwrap();
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/jit_package_resource.dart"),
        project.join("bin/main.dart"),
    )
    .unwrap();
    let dll = dartforge_emit_native::sdk_module::source_sdk_dll().expect("source-built SDK DLL");
    for explicit in [false, true] {
        for mode in ["run", "reload", "restart"] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_dartforge"));
            command
                .current_dir(&project)
                .arg(if mode == "run" { "run" } else { "reload" });
            if mode != "run" {
                command.arg("bin/main.dart").arg("--once");
            }
            if explicit {
                command.args(["--packages", ".dart_tool/package_config.json"]);
            }
            if mode == "run" {
                command.arg("bin/main.dart");
            }
            if mode == "restart" {
                command.arg("--restart");
            }
            let output = command
                .env("DARTFORGE_SDK_DLL", &dll)
                .env("DARTFORGE_PACKAGE_CONFIG", root.join("stale.json"))
                .output()
                .expect("CLI");
            assert!(
                output.status.success(),
                "{mode}, explicit={explicit}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
                "selected package resource\n",
                "{mode}, explicit={explicit}"
            );
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires the full LLVM distribution and source-built SDK DLL"]
fn absent_package_configuration_matches_vm_despite_inherited_selection() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("dartforge-jit-package-absence-{unique}"));
    std::fs::create_dir_all(&root).unwrap();
    let fixture = root.join("main.dart");
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/jit_package_absence.dart"),
        &fixture,
    )
    .unwrap();
    let stale = root.join("stale.json");
    std::fs::write(&stale, r#"{"configVersion":2,"packages":[]}"#).unwrap();
    let sdk = dartforge_emit_native::dart_sdk().expect("Dart SDK");
    let dart = sdk
        .parent()
        .unwrap()
        .join("bin")
        .join(format!("dart{}", std::env::consts::EXE_SUFFIX));
    let oracle = Command::new(dart)
        .arg(&fixture)
        .current_dir(&root)
        .output()
        .expect("Dart VM");
    assert!(
        oracle.status.success(),
        "{}",
        String::from_utf8_lossy(&oracle.stderr)
    );
    let expected = String::from_utf8_lossy(&oracle.stdout).replace("\r\n", "\n");
    let dll = dartforge_emit_native::sdk_module::source_sdk_dll().expect("source-built SDK DLL");
    for mode in ["run", "reload", "restart"] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_dartforge"));
        command
            .current_dir(&root)
            .arg(if mode == "run" { "run" } else { "reload" })
            .arg(&fixture);
        if mode != "run" {
            command.arg("--once");
        }
        if mode == "restart" {
            command.arg("--restart");
        }
        let output = command
            .env("DARTFORGE_SDK_DLL", &dll)
            .env("DARTFORGE_PACKAGE_CONFIG", &stale)
            .output()
            .expect("CLI");
        assert!(
            output.status.success(),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
            expected,
            "{mode}"
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires LLVM and the source-built SDK"]
fn environment_definitions_reach_run_reload_and_restart_without_eating_main_arguments() {
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = Scratch(std::env::temp_dir().join(format!(
        "dartforge-cli-definitions-{}-{stamp}",
        std::process::id()
    )));
    std::fs::create_dir(&directory.0).unwrap();
    let source = directory.0.join("main.dart");
    std::fs::write(&source, "void main(List<String> args) { print(const String.fromEnvironment('tag')); print(String.fromEnvironment('tag')); print(args.join('|')); }").unwrap();
    for mode in ["run", "reload", "restart"] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_dartforge"));
        command.arg(if mode == "run" { "run" } else { "reload" });
        command.args(["-Dtag=old", "--define", "tag=selected"]);
        command.arg(&source);
        if mode != "run" {
            command.arg("--once");
        }
        if mode == "restart" {
            command.arg("--restart");
        }
        if mode != "run" {
            command.arg("--");
        }
        let output = command
            .args(["-Dtag=application", "--define", "also-application"])
            .env_remove("DARTFORGE_SDK_DLL")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout)
                .unwrap()
                .replace("\r\n", "\n"),
            "selected\nselected\n-Dtag=application|--define|also-application\n",
            "{mode}"
        );
    }
}

#[test]
fn ir_execution_rejects_new_definitions_before_reading_the_ir() {
    let output = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .args(["run", "-Dtag=value", "--ir", "missing-context.ll"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("already encoded in LLVM IR"));
}
