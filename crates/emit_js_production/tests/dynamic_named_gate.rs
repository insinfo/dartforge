//! Without a dynamic call that passes named arguments, the named paths of
//! the runtime's dynamic calls fold away (`Constants::set_without_dynamic_named`);
//! a program with such calls (or `Function.apply` with a map) restarts with
//! them. Both match the VM, with the gate and without it.

use std::path::Path;
use std::process::Command;

fn run(name: &str, directory: &Path) -> Vec<u64> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../emit_js/tests/programs/{name}.dart"));
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
    let mut sizes = Vec::new();
    for switch in ["1", "0"] {
        let artifact = directory.join(format!("{name}{switch}.js"));
        let result = Command::new(env!("CARGO_BIN_EXE_dartforge-jsprod"))
            .arg(&source)
            .arg("-o")
            .arg(&artifact)
            .env("DARTFORGE_JSPROD_DYNAMIC_NAMED_GATE", switch)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        sizes.push(std::fs::metadata(&artifact).unwrap().len());
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
            "{name} DARTFORGE_JSPROD_DYNAMIC_NAMED_GATE={switch}"
        );
    }
    sizes
}

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn dynamic_named_paths_match_the_vm() {
    let directory = std::env::temp_dir().join(format!(
        "dartforge-dynamic-named-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let gated = run("dynamic_named_gate", &directory);
    assert!(
        gated[0] < gated[1],
        "the named paths should fold: {gated:?}"
    );
    let named = run("dynamic_named_calls", &directory);
    assert_eq!(named[0], named[1], "named calls keep every path: {named:?}");
    std::fs::remove_dir_all(directory).unwrap();
}
