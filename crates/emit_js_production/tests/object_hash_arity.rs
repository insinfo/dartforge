//! `Object.hash` calls go to a patched function per arity (`_hashN`); each
//! one gives the same hash as the original body reached through a
//! tear-off, with the patch and without it.

use std::path::Path;
use std::process::Command;

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn each_arity_matches_the_original_body() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../emit_js/tests/programs/object_hash_arity.dart");
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
    assert!(expected.contains("20 true"));
    let directory = std::env::temp_dir().join(format!(
        "dartforge-object-hash-arity-{}-{}",
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
            .env("DARTFORGE_JSPROD_HASH_ARITY", switch)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
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
            "DARTFORGE_JSPROD_HASH_ARITY={switch}"
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}
