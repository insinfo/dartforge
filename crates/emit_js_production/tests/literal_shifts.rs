//! Shifts by a literal count are written inline in production; each value
//! is the one `JSNumber`'s operators give, compared with the same program
//! compiled without the change (the web shift semantics differ from the VM
//! for negative operands and large counts, so the reference is the
//! operators themselves).

use std::path::Path;
use std::process::Command;

fn run(source: &Path, artifact: &Path, switch: &str) -> (String, String) {
    let result = Command::new(env!("CARGO_BIN_EXE_dartforge-jsprod"))
        .arg(source)
        .arg("-o")
        .arg(artifact)
        .env("DARTFORGE_JSPROD_LITERAL_SHIFTS", switch)
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
        .arg(artifact)
        .output()
        .unwrap();
    assert!(
        run.status.success() && run.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    (
        String::from_utf8(run.stdout).unwrap(),
        std::fs::read_to_string(artifact).unwrap(),
    )
}

#[test]
#[ignore = "requires Node on PATH"]
fn inline_shifts_match_the_operators() {
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../emit_js/tests/programs/literal_shifts.dart");
    let directory = std::env::temp_dir().join(format!(
        "dartforge-literal-shifts-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let (inline, _) = run(&source, &directory.join("on.js"), "1");
    let (calls, _) = run(&source, &directory.join("off.js"), "0");
    assert_eq!(inline, calls);
    assert!(
        inline.contains("\n5\n"),
        "every side effect runs once: {inline}"
    );
    std::fs::remove_dir_all(directory).unwrap();
}
