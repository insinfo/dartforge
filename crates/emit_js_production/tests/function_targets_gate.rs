//! Function types that are only ever closure signatures (the subtype test's
//! left side) let production drop `_isFunctionSubtype`
//! (`own_sdk::targets_function_type`); a program that tests against a
//! function type keeps it. Both print as on the VM, with the switch and
//! without it.

use std::path::Path;
use std::process::Command;

fn check(program: &str, dropped_with_gate: bool) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../emit_js/tests/programs/{program}.dart"));
    let vm = Command::new("dart")
        .arg("run")
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        vm.status.success(),
        "{}",
        String::from_utf8_lossy(&vm.stderr)
    );
    let expected = String::from_utf8(vm.stdout).unwrap().replace("\r\n", "\n");
    let directory = std::env::temp_dir().join(format!(
        "dartforge-{program}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    for switch in ["1", "0"] {
        let artifact = directory.join(format!("out{switch}.js"));
        let result = Command::new(env!("CARGO_BIN_EXE_dartforge-jsprod"))
            .arg(&source)
            .arg("-o")
            .arg(&artifact)
            .env("DARTFORGE_JSPROD_FUNCTION_TARGETS_GATE", switch)
            .env("DARTFORGE_JSPROD_MANGLE", "0")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        let js = std::fs::read_to_string(&artifact).unwrap();
        assert_eq!(
            !js.contains("_isFunctionSubtype"),
            dropped_with_gate && switch == "1",
            "{program} DARTFORGE_JSPROD_FUNCTION_TARGETS_GATE={switch}"
        );
        let run = Command::new("node")
            .args(["-e", "globalThis.self=globalThis;require(process.argv[1])"])
            .arg(&artifact)
            .output()
            .unwrap();
        assert!(
            run.status.success() && run.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(
            String::from_utf8(run.stdout).unwrap().replace("\r\n", "\n"),
            expected,
            "{program} DARTFORGE_JSPROD_FUNCTION_TARGETS_GATE={switch}"
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn closure_signatures_only_drop_function_subtyping() {
    check("function_targets_closures", true);
}

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn tested_function_types_keep_function_subtyping() {
    check("function_targets_tested", false);
}
