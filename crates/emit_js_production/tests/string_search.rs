//! Searches of a `String` by a `String` written as the JavaScript methods
//! (`emit_js`'s `string_search_call`, `dartforge_world`'s
//! `string_search_receiver`) give the VM's results, with the switch and
//! without it.

use std::path::Path;
use std::process::Command;

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn string_search_matches_the_vm() {
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../emit_js/tests/programs/string_search.dart");
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
        "dartforge-string_search-{}-{}",
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
            .env("DARTFORGE_JSPROD_STRING_SEARCH", switch)
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
            "DARTFORGE_JSPROD_STRING_SEARCH={switch}"
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}
