//! `async` bodies as state machines (`emit_js/src/async_machine.rs`,
//! `docs/JS-PRODUCAO-VELOCIDADE.md` §9): every control structure around an
//! `await`, the evaluation order around it, errors, `finally` and the order
//! of the event loop give the VM's output at each level, with the lowering
//! and without it (the generator).

use std::path::Path;
use std::process::Command;

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn async_state_machine_matches_the_vm() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../emit_js/tests/programs/async_state_machine.dart");
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
        "dartforge-async_state_machine-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    for (level, switch) in [
        ("-O1", "1"),
        ("-O2", "1"),
        ("-O4", "1"),
        ("-O1", "0"),
        ("-O4", "0"),
    ] {
        let artifact = directory.join(format!("out{level}{switch}.js"));
        let result = Command::new(env!("CARGO_BIN_EXE_dartforge-jsprod"))
            .arg(&source)
            .arg("-o")
            .arg(&artifact)
            .arg(level)
            .env("DARTFORGE_JSPROD_ASYNC_MACHINE", switch)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        if level == "-O1" {
            // Not minified: the state machines are visible.
            let js = std::fs::read_to_string(&artifact).unwrap();
            assert_eq!(js.contains("switch (t$goto)"), switch == "1");
        }
        let run = Command::new("node")
            .args(["-e", "globalThis.self=globalThis;require(process.argv[1])"])
            .arg(&artifact)
            .output()
            .unwrap();
        assert!(
            run.status.success() && run.stderr.is_empty(),
            "{level} switch={switch}: {}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(
            String::from_utf8(run.stdout).unwrap().replace("\r\n", "\n"),
            expected,
            "{level} switch={switch}"
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}
