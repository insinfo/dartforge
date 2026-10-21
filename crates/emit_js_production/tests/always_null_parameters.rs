//! Parameters that every call leaves `null`: the production profile drops the
//! branches that test them (and what only those branches use), keeps the
//! counterexamples, and still prints what the VM prints.
use std::path::Path;
use std::process::Command;

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn always_null_branches_are_dropped_and_output_matches_vm() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/js/242_always_null_parameters.dart");
    let directory =
        std::env::temp_dir().join(format!("dartforge-null-params-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
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
    let artifact = directory.join("out.js");
    let result = Command::new(env!("CARGO_BIN_EXE_dartforge-jsprod"))
        .arg(&source)
        .arg("-o")
        .arg(&artifact)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let js = std::fs::read_to_string(&artifact).unwrap();
    // Dead: only reached through a parameter that is always null.
    for dead in ["only on ", "verbose on ", "Selector("] {
        assert!(!js.contains(dead), "`{dead}` should have been dropped");
    }
    // Counterexamples: a tear-off and a dynamic call reach the non-null
    // branches.
    for live in ["tagged ", " widget", " panel"] {
        assert!(js.contains(live), "`{live}` should stay");
    }
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
        expected
    );
    std::fs::remove_dir_all(directory).unwrap();
}
