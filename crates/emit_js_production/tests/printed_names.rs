//! Names the production profile changes print as on the VM: named
//! parameters keep their source names in a printed function type (the keys
//! are shortened, `named_args.rs`, and the runtime prints through the reverse
//! table), generic function types name their type parameters `Y0`, `F1Y0`…,
//! and a dynamic call of a private name shows the Dart name, not the
//! production property with the library tag.

use std::path::Path;
use std::process::Command;

fn check(program: &str, table: Option<bool>) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../emit_js/tests/programs/{program}.dart"));
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
    let expected = String::from_utf8(vm.stdout).unwrap().replace("\r\n", "\n");
    let directory = std::env::temp_dir().join(format!(
        "dartforge-{program}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    for (level, minify) in [("-O1", false), ("-O2", true), ("-O4", true)] {
        let artifact = directory.join(format!("out{level}.js"));
        let result = Command::new(env!("CARGO_BIN_EXE_dartforge-jsprod"))
            .arg(&source)
            .arg("-o")
            .arg(&artifact)
            .arg(level)
            .arg("--enable-asserts")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        let js = std::fs::read_to_string(&artifact).unwrap();
        if let (Some(table), false) = (table, minify) {
            assert_eq!(
                js.contains("dart.namedSourceName = "),
                table,
                "{program} {level}: the reverse table of named keys"
            );
        }
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
            "{program} {level}"
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn function_types_print_source_names_and_vm_type_parameters() {
    check("function_type_names", Some(true));
}

#[test]
#[ignore = "requires Dart 3.6.2 and Node on PATH"]
fn private_dynamic_names_print_without_the_library_tag() {
    check("private_dynamic_names", None);
}
