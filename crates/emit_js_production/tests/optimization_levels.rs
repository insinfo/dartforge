//! The optimization levels (`-O1`…`-O4`) drop the checks `dart2js` drops at
//! the same level, and the `dart2js:` pragmas override the level per member
//! as in `dart2js`: the program's output equals `dart compile js -O<n>`'s at
//! each level. At `-O1`/`-O2` it equals the VM's except on the lines whose
//! member trusts a check by pragma (the VM ignores `dart2js` pragmas).

use std::path::Path;
use std::process::Command;

fn run_js(artifact: &Path) -> String {
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
    String::from_utf8(run.stdout).unwrap().replace("\r\n", "\n")
}

#[test]
#[ignore = "requires Dart 3.6.2 (with dart compile js) and Node on PATH"]
fn levels_drop_the_checks_dart2js_drops() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../emit_js/tests/programs/optimization_levels.dart");
    let vm = Command::new("dart")
        .args(["run", "--enable-asserts"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        vm.status.success(),
        "{}",
        String::from_utf8_lossy(&vm.stderr)
    );
    let vm = String::from_utf8(vm.stdout).unwrap().replace("\r\n", "\n");
    let directory = std::env::temp_dir().join(format!(
        "dartforge-optimization-levels-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let pragma_lines = ["as:trust:", "index-bounds:trust:", "constant pragma:"];
    let mut sizes = Vec::new();
    for level in 1..=4 {
        let flag = format!("-O{level}");
        let reference = directory.join(format!("dart2js{level}.js"));
        let d2j = Command::new("dart")
            .args(["compile", "js", &flag, "--enable-asserts", "-o"])
            .arg(&reference)
            .arg(&source)
            .output()
            .unwrap();
        assert!(
            d2j.status.success(),
            "{}",
            String::from_utf8_lossy(&d2j.stderr)
        );
        let expected = run_js(&reference);
        let artifact = directory.join(format!("out{level}.js"));
        let result = Command::new(env!("CARGO_BIN_EXE_dartforge-jsprod"))
            .arg(&source)
            .arg("-o")
            .arg(&artifact)
            .arg(&flag)
            .arg("--enable-asserts")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        let ours = run_js(&artifact);
        assert_eq!(ours, expected, "{flag}: ours × dart2js");
        if level <= 2 {
            for (a, b) in ours.lines().zip(vm.lines()) {
                if !pragma_lines.iter().any(|p| a.starts_with(p)) {
                    assert_eq!(a, b, "{flag}: ours × VM");
                }
            }
        }
        sizes.push(std::fs::metadata(&artifact).unwrap().len());
    }
    // Each level only removes code from the one before (`-O1` is not
    // minified).
    assert!(sizes[2] < sizes[1] && sizes[3] <= sizes[2], "{sizes:?}");
    std::fs::remove_dir_all(directory).unwrap();
}
