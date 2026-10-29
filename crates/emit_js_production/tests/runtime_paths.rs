//! The runtime paths the production profile shortens
//! (`docs/JS-PRODUCAO-VELOCIDADE.md` §11): lazy statics that become data
//! properties, folded `const` ints, the `Future` completion and listener
//! paths and the closure recipes of the runtime declared once. The program
//! gives the VM's answers at each level, with the switches and without them.

use std::path::Path;
use std::process::Command;

const SWITCHES: &[&str] = &[
    "DARTFORGE_JSPROD_FUTURE_PATHS",
    "DARTFORGE_JSPROD_LAZY_DATA_PROPERTIES",
    "DARTFORGE_JSPROD_FOLD_CONST_INTS",
    "DARTFORGE_JSPROD_SDK_CLOSURE_RECIPES",
];

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn runtime_paths_match_the_vm() {
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../emit_js/tests/programs/runtime_paths.dart");
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
        "dartforge-runtime_paths-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    for (level, switch) in [("-O1", "1"), ("-O2", "1"), ("-O4", "1"), ("-O4", "0")] {
        let artifact = directory.join(format!("out{level}{switch}.js"));
        let mut compile = Command::new(env!("CARGO_BIN_EXE_dartforge-jsprod"));
        compile.arg(&source).arg("-o").arg(&artifact).arg(level);
        for s in SWITCHES {
            compile.env(s, switch);
        }
        let result = compile.output().unwrap();
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
            "{level} switches={switch}: {}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(
            String::from_utf8(run.stdout).unwrap().replace("\r\n", "\n"),
            expected,
            "{level} switches={switch}"
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}
