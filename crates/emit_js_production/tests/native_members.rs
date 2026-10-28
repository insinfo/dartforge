//! The direct JavaScript forms of the production profile's speed round
//! (`docs/JS-PRODUCAO-VELOCIDADE.md`): `length`/`isEmpty`/`isNotEmpty` of the
//! indexable primitives, `for-in` over a list by index, trusted index writes
//! (`-O4`), `toDouble`/`toInt`, literal masks, `const` constructors without
//! the leading `null`s, `async` bodies without `await`, the extension
//! prototypes installed once and the recipe cache. The program gives the
//! VM's answers at each level, with the switches and without them.

use std::path::Path;
use std::process::Command;

const SWITCHES: &[&str] = &[
    "DARTFORGE_JSPROD_NATIVE_LENGTH",
    "DARTFORGE_JSPROD_INDEXED_LOOPS",
    "DARTFORGE_JSPROD_INDEX_SET",
    "DARTFORGE_JSPROD_NUMBER_IDENTITY",
    "DARTFORGE_JSPROD_MASK",
    "DARTFORGE_JSPROD_CONST_ORDER",
    "DARTFORGE_JSPROD_SYNC_ASYNC_BODIES",
    "DARTFORGE_JSPROD_FAST_PROTOTYPES",
    "DARTFORGE_JSPROD_RECIPE_CACHE",
];

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn native_members_match_the_vm() {
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../emit_js/tests/programs/native_members.dart");
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
        "dartforge-native_members-{}-{}",
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
