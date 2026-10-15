//! The AOT × JIT matrix of the program operations (J06), through
//! `dartforge run`: `Platform.script` is the `.dart`, `Isolate.spawnUri` is
//! rejected with the JIT's message and `Isolate.spawn` works.
#![cfg(feature = "jit")]

use std::process::Command;

#[test]
#[ignore = "compiles the source-built SDK DLL; run in the sdk-fonte job of the Pesado workflow"]
fn script_and_spawn_uri_in_the_jit() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/jit_script.dart");
    let dll = dartforge_emit_native::sdk_module::source_sdk_dll().expect("source-built SDK DLL");
    let output = Command::new(env!("CARGO_BIN_EXE_dartforge"))
        .arg("run")
        .arg(&fixture)
        .env("DARTFORGE_SDK_DLL", dll)
        .output()
        .expect("CLI");
    let stdout = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");
    assert_eq!(stdout, "jit_script.dart\ntrue\ndo filho\n", "stderr: {}", String::from_utf8_lossy(&output.stderr));
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
        command.arg(if mode == "run" { "run" } else { "reload" }).arg(&fixture);
        if mode != "run" { command.arg("--once"); }
        if mode == "restart" { command.arg("--restart"); }
        command.args(["--", "--timings", "--child", "", "é folder", "--"])
            .env("DARTFORGE_SDK_DLL", &dll);
        let output = command.output().expect("CLI");
        assert!(output.status.success(), "{mode}: {}", String::from_utf8_lossy(&output.stderr));
        let stdout = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");
        assert_eq!(stdout, "[\"--timings\",\"--child\",\"\",\"é folder\",\"--\"]\n", "{mode}: {}", String::from_utf8_lossy(&output.stderr));
    }
}

#[test]
#[ignore = "requires the full LLVM distribution and source-built SDK DLL"]
fn selected_package_resources_resolve_from_project_cwd_in_all_jit_modes() {
    let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let root = std::env::temp_dir().join(format!("dartforge-jit-package-resource-{unique}"));
    let project = root.join("project");
    let config = project.join(".dart_tool/package_config.json");
    std::fs::create_dir_all(project.join("bin")).unwrap();
    std::fs::create_dir_all(project.join(".dart_tool")).unwrap();
    std::fs::create_dir_all(root.join("dependency/lib")).unwrap();
    std::fs::write(root.join("dependency/lib/resource.txt"), "selected package resource\n").unwrap();
    std::fs::write(&config, r#"{"configVersion":2,"packages":[{"name":"resource_package","rootUri":"../../dependency/","packageUri":"lib/","languageVersion":"3.0"}]}"#).unwrap();
    std::fs::copy(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/jit_package_resource.dart"), project.join("bin/main.dart")).unwrap();
    let dll = dartforge_emit_native::sdk_module::source_sdk_dll().expect("source-built SDK DLL");
    for explicit in [false, true] {
        for mode in ["run", "reload", "restart"] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_dartforge"));
            command.current_dir(&project).arg(if mode == "run" { "run" } else { "reload" });
            if mode != "run" { command.arg("bin/main.dart").arg("--once"); }
            if explicit { command.args(["--packages", ".dart_tool/package_config.json"]); }
            if mode == "run" { command.arg("bin/main.dart"); }
            if mode == "restart" { command.arg("--restart"); }
            let output = command.env("DARTFORGE_SDK_DLL", &dll)
                .env("DARTFORGE_PACKAGE_CONFIG", root.join("stale.json"))
                .output().expect("CLI");
            assert!(output.status.success(), "{mode}, explicit={explicit}: {}", String::from_utf8_lossy(&output.stderr));
            assert_eq!(String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"), "selected package resource\n", "{mode}, explicit={explicit}");
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires the full LLVM distribution and source-built SDK DLL"]
fn absent_package_configuration_matches_vm_despite_inherited_selection() {
    let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let root = std::env::temp_dir().join(format!("dartforge-jit-package-absence-{unique}"));
    std::fs::create_dir_all(&root).unwrap();
    let fixture = root.join("main.dart");
    std::fs::copy(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/jit_package_absence.dart"), &fixture).unwrap();
    let stale = root.join("stale.json");
    std::fs::write(&stale, r#"{"configVersion":2,"packages":[]}"#).unwrap();
    let sdk = dartforge_emit_native::dart_sdk().expect("Dart SDK");
    let dart = sdk.parent().unwrap().join("bin").join(format!("dart{}", std::env::consts::EXE_SUFFIX));
    let oracle = Command::new(dart).arg(&fixture).current_dir(&root).output().expect("Dart VM");
    assert!(oracle.status.success(), "{}", String::from_utf8_lossy(&oracle.stderr));
    let expected = String::from_utf8_lossy(&oracle.stdout).replace("\r\n", "\n");
    let dll = dartforge_emit_native::sdk_module::source_sdk_dll().expect("source-built SDK DLL");
    for mode in ["run", "reload", "restart"] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_dartforge"));
        command.current_dir(&root).arg(if mode == "run" { "run" } else { "reload" }).arg(&fixture);
        if mode != "run" { command.arg("--once"); }
        if mode == "restart" { command.arg("--restart"); }
        let output = command.env("DARTFORGE_SDK_DLL", &dll)
            .env("DARTFORGE_PACKAGE_CONFIG", &stale).output().expect("CLI");
        assert!(output.status.success(), "{mode}: {}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"), expected, "{mode}");
    }
    std::fs::remove_dir_all(root).unwrap();
}
