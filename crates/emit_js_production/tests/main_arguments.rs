//! `main(List<String> args)` receives an empty `List<String>` (and `null`
//! for a second positional parameter), as on the VM and in `dart2js`: the
//! entry used to call `main()` with no arguments, and `args` was `undefined`.

use std::path::Path;
use std::process::Command;

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn main_receives_the_vm_arguments() {
    let directory = std::env::temp_dir().join(format!(
        "dartforge-main_arguments-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    for program in ["main_arguments", "main_arguments_message"] {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../emit_js/tests/programs/{program}.dart"));
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
        for level in ["-O1", "-O2", "-O4"] {
            let artifact = directory.join(format!("{program}{level}.js"));
            let result = Command::new(env!("CARGO_BIN_EXE_dartforge-jsprod"))
                .arg(&source)
                .arg("-o")
                .arg(&artifact)
                .arg(level)
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
                "{program} {level}: {}",
                String::from_utf8_lossy(&run.stderr)
            );
            assert_eq!(
                String::from_utf8(run.stdout).unwrap().replace("\r\n", "\n"),
                expected,
                "{program} {level}"
            );
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
}
