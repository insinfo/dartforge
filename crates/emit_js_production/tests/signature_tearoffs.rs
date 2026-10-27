//! Signature entries read by tear-offs through the extension symbol
//! (`dart.bind(o, $name)`): the production profile keeps them for the SDK
//! classes and the program's classes that implement a native interface, and
//! drops the `dartx` keys nothing reads.

use std::path::Path;
use std::process::Command;

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn symbol_tearoffs_keep_their_signatures() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../emit_js/tests/programs/signature_tearoffs.dart");
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
    assert!(expected.contains("elementAt: TypeError"));
    let directory = std::env::temp_dir().join(format!(
        "dartforge-signature-tearoffs-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    // Each switch alone, and both: the output never changes.
    let cases: [&[(&str, &str)]; 3] = [
        &[],
        &[("DARTFORGE_JSPROD_SYMBOL_SIGNATURES", "0")],
        &[("DARTFORGE_JSPROD_ALIAS_SIGNATURES", "0")],
    ];
    let mut sizes = Vec::new();
    for (i, env) in cases.iter().enumerate() {
        let artifact = directory.join(format!("out{i}.js"));
        let result = Command::new(env!("CARGO_BIN_EXE_dartforge-jsprod"))
            .arg(&source)
            .arg("-o")
            .arg(&artifact)
            .envs(env.iter().copied())
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
            "{env:?}"
        );
        sizes.push(std::fs::metadata(&artifact).unwrap().len());
    }
    assert!(
        sizes[0] <= sizes[1] && sizes[0] < sizes[2],
        "the unread entries go: {sizes:?}"
    );
    std::fs::remove_dir_all(directory).unwrap();
}
