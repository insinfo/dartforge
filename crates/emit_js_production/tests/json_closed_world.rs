//! `jsonEncode` in a program with no `toJson` member
//! (`docs/JS-PRODUCAO-VELOCIDADE.md` §14): the dynamic `toJson` of the
//! default `toEncodable` is the plain JavaScript call, without the
//! `noSuchMethod` path. The program gives the VM's answers
//! at each level, with the switches and without them, and the forms are in
//! the text only with the switches on.

use std::path::Path;
use std::process::Command;

const SWITCHES: &[&str] = &["DARTFORGE_JSPROD_UNANSWERED_CALLS"];

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn json_closed_world_matches_the_vm() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../emit_js/tests/programs/json_closed_world.dart");
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
        "dartforge-json_closed_world-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    for (level, switch) in [
        ("-O1", "1"),
        ("-O2", "1"),
        ("-O3", "1"),
        ("-O4", "1"),
        ("-O1", "0"),
        ("-O4", "0"),
    ] {
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
        if level == "-O1" {
            let text = std::fs::read_to_string(&artifact).unwrap();
            let on = switch == "1";
            assert_eq!(
                text.contains(".toJson()"),
                on,
                "-O1 switches={switch}: toJson"
            );
            assert_eq!(
                text.contains("InvocationImpl"),
                !on,
                "-O1 switches={switch}: dsend"
            );
        }
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
