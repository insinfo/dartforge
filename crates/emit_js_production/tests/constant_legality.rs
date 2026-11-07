//! Constant legality is checked before production reachability pruning.
use std::process::Command;
#[test]
#[ignore = "requires the Dart 3.6.2 SDK and Node"]
fn annotation_arguments_are_constant_with_or_without_an_import_prefix() {
    let directory = std::env::temp_dir().join(format!(
        "dartforge-js-annotation-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(
        directory.join("annotation.dart"),
        "class Marker { const Marker(int value); }\n",
    )
    .unwrap();
    let compiler = std::env::var_os("DARTFORGE_JSPROD")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_dartforge-jsprod").into());
    for (case, prefixed, constant) in [(0, true, false), (1, false, false), (2, true, true)] {
        let import = if prefixed {
            "import 'annotation.dart' as annotation;"
        } else {
            "import 'annotation.dart';"
        };
        let binding = if constant {
            "const int runtimeValue = 1;"
        } else {
            "int runtimeValue = 1;"
        };
        let annotation = if prefixed {
            "@annotation.Marker(runtimeValue)"
        } else {
            "@Marker(runtimeValue)"
        };
        let source = directory.join("main.dart");
        std::fs::write(&source,format!("{import}\n{binding}\n{annotation}\nvoid decorated() {{}}\nvoid main() {{ print('constant annotation ok'); }}\n")).unwrap();
        for level in ["-O1", "-O2"] {
            let reference = directory.join(format!("reference-{case}{level}.js"));
            let oracle = Command::new("dart")
                .args(["compile", "js", level])
                .arg(&source)
                .arg("-o")
                .arg(&reference)
                .output()
                .unwrap();
            assert_eq!(
                oracle.status.success(),
                constant,
                "{}{}",
                String::from_utf8_lossy(&oracle.stdout),
                String::from_utf8_lossy(&oracle.stderr)
            );
            let artifact = directory.join(format!("ours-{case}{level}.js"));
            let mut command = Command::new(&compiler);
            command.arg(&source).arg("-o").arg(&artifact).arg(level);
            if let Some(sdk) = std::env::var_os("DARTFORGE_SDK_LIB") {
                command.arg("--sdk").arg(sdk);
            }
            let output = command.output().unwrap();
            if !constant {
                assert_eq!(
                    output.status.code(),
                    Some(1),
                    "{level}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                assert!(
                    String::from_utf8_lossy(&output.stderr)
                        .contains("const_with_non_constant_argument")
                );
                assert!(!artifact.exists());
            } else {
                assert!(
                    output.status.success(),
                    "{level}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                for path in [&reference, &artifact] {
                    let output = Command::new("node")
                        .args(["-e", "globalThis.self=globalThis;require(process.argv[1])"])
                        .arg(path)
                        .output()
                        .unwrap();
                    assert!(
                        output.status.success() && output.stderr.is_empty(),
                        "{}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    assert_eq!(
                        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
                        "constant annotation ok\n"
                    );
                }
            }
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
}
