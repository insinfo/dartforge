//! The production forms of `docs/JS-PRODUCAO-VELOCIDADE.md` §11: constants
//! cached per site, `for`-in through the Dart iterator, `S? as S` that
//! checks only `null`, `is C` by the prototype chain, `is num`/`is int` and
//! `identical(x, literal)` without the runtime, and the native sort of lists
//! of numbers or strings. The program gives
//! the VM's answers at each level, with the switches and without them.

use std::path::Path;
use std::process::Command;

const SWITCHES: &[&str] = &[
    "DARTFORGE_JSPROD_CONST_SLOTS",
    "DARTFORGE_JSPROD_ITERATOR_LOOPS",
    "DARTFORGE_JSPROD_NON_NULL_CASTS",
    "DARTFORGE_JSPROD_INSTANCEOF",
    "DARTFORGE_JSPROD_PRIMITIVE_SORT",
    "DARTFORGE_JSPROD_NUMBER_TESTS",
    "DARTFORGE_JSPROD_IDENTICAL_LITERALS",
];

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn production_forms_match_the_vm() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../emit_js/tests/programs/production_forms.dart");
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
        "dartforge-production_forms-{}-{}",
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
        ("-O2", "0"),
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
        // The forms are really there (-O1 is not minified).
        if level == "-O1" {
            let text = std::fs::read_to_string(&artifact).unwrap();
            for form in [
                " ??= dart.const(",
                ".moveNext();) {",
                " != null ? ",
                " instanceof L$production_forms.Sub",
            ] {
                assert!(text.contains(form), "-O1 lacks {form:?}");
            }
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
