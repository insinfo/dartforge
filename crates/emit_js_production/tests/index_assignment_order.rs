//! Indexed writes keep the VM's evaluation order, written as one call
//! (`emit_index_set_with`) and, with `DARTFORGE_JSPROD_COMPACT_INDEX_SET=0`,
//! with every operand in a temporary.
use std::path::Path;
use std::process::Command;

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn indexed_assignments_match_vm_and_o2() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../emit_js/tests/programs/index_assignment_order.dart");
    let directory =
        std::env::temp_dir().join(format!("dartforge-index-order-{}", std::process::id()));
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
    for profile in ["O2", "raw", "min", "temps"] {
        let artifact = directory.join(format!("{profile}.js"));
        let mut compile = if profile == "O2" {
            let mut c = Command::new("dart");
            c.args(["compile", "js", "-O2"]);
            c
        } else {
            let mut c = Command::new(env!("CARGO_BIN_EXE_dartforge-jsprod"));
            if profile == "raw" {
                c.arg("--no-minify");
            }
            if profile == "temps" {
                c.env("DARTFORGE_JSPROD_COMPACT_INDEX_SET", "0");
            }
            c
        };
        let result = compile
            .arg(&source)
            .arg("-o")
            .arg(&artifact)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{profile}: {}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        let result = Command::new("node")
            .args(["-e", "globalThis.self=globalThis;require(process.argv[1])"])
            .arg(&artifact)
            .output()
            .unwrap();
        assert!(
            result.status.success() && result.stderr.is_empty(),
            "{profile}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout)
                .unwrap()
                .replace("\r\n", "\n"),
            expected,
            "{profile}"
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}
