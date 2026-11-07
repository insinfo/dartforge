//! Compiler declarations must agree with dart2js and survive repeated SDK loads.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dartforge-definitions-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn successful(output: Output) -> Vec<u8> {
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[test]
#[ignore = "requires Dart 3.6.2 and Node"]
fn definitions_match_dart2js_across_repeated_api_and_cli_compilations() {
    let scratch = Scratch::new();
    let sdk = PathBuf::from(std::env::var_os("DARTFORGE_DART_SDK").expect("Dart 3.6.2 oracle SDK"));
    let dart = sdk.join(if cfg!(windows) {
        "bin/dart.exe"
    } else {
        "bin/dart"
    });
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/programs/environment_definitions/main.dart");
    // The first and last definitions deliberately recur in one process: neither
    // a prior SDK cache nor a prior analysis may supply the middle compilation.
    let cases = [
        vec![
            "feature=true",
            "flavor=yes",
            "message=first",
            "message=hello=world",
            "empty=",
            "number=9007199254740993",
            "hex=0xffffffffffffffff",
            "invalidBool=TRUE",
            "invalidInt=1_000",
            "dart.library.missing=true",
            "dart.library.js=false",
        ],
        vec![
            "feature=false",
            "flavor=no",
            "message=changed",
            "number=-0x10",
            "hex=-0xffffffffffffffff",
            "dart.vm.product=false",
        ],
        vec![
            "feature=true",
            "flavor=yes",
            "message=first",
            "message=hello=world",
            "empty=",
            "number=9007199254740993",
            "hex=0xffffffffffffffff",
            "invalidBool=TRUE",
            "invalidInt=1_000",
            "dart.library.missing=true",
            "dart.library.js=false",
        ],
    ];
    for (index, definitions) in cases.into_iter().enumerate() {
        let reference = scratch.0.join(format!("reference-{index}.js"));
        let mut command = Command::new(&dart);
        command.args(["compile", "js", "-O2"]);
        for definition in &definitions {
            command.arg(format!("-D{definition}"));
        }
        let output = command
            .arg(&source)
            .arg("-o")
            .arg(&reference)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let expected = successful(Command::new("node").arg(&reference).output().unwrap());
        let language = dartforge_elements::sdk::LanguageOptions {
            defines: definitions.iter().map(|value| (*value).into()).collect(),
            ..Default::default()
        };
        let compiled = dartforge_emit_js_production::compile_own_sdk(
            &source,
            Some(&sdk.join("lib")),
            None,
            dartforge_emit_js_production::Options {
                minify: false,
                ..Default::default()
            },
            &language,
        )
        .unwrap();
        let artifact = scratch.0.join(format!("ours-{index}.js"));
        std::fs::write(&artifact, compiled.js).unwrap();
        assert_eq!(
            successful(Command::new("node").arg(&artifact).output().unwrap()),
            expected,
            "API case {index}"
        );
        if index == 1 {
            let cli = scratch.0.join("cli.js");
            let mut command = Command::new(env!("CARGO_BIN_EXE_dartforge-jsprod"));
            command
                .arg(&source)
                .arg("--sdk")
                .arg(sdk.join("lib"))
                .arg("-o")
                .arg(&cli);
            for definition in &definitions {
                command.arg(format!("--define={definition}"));
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                successful(Command::new("node").arg(cli).output().unwrap()),
                expected,
                "CLI case"
            );
        }
    }
}

#[test]
#[ignore = "requires Dart 3.6.2 and Node"]
fn development_environment_calls_preserve_ddc_argument_order_and_type_checks() {
    let scratch = Scratch::new();
    let sdk = PathBuf::from(std::env::var_os("DARTFORGE_DART_SDK").expect("Dart 3.6.2 oracle SDK"));
    let dart = sdk.join(if cfg!(windows) {
        "bin/dart.exe"
    } else {
        "bin/dart"
    });
    let snapshot = sdk.join("bin/snapshots/dartdevc.dart.snapshot");
    let sdk_js = scratch.0.join("dart_sdk.js");
    successful(
        Command::new(&dart)
            .arg(&snapshot)
            .args([
                "--multi-root-scheme=org-dartlang-sdk",
                "--modules=es6",
                "--module-name=dart_sdk",
                "-o",
            ])
            .arg(&sdk_js)
            .arg(sdk.join("lib/_internal/ddc_platform.dill"))
            .output()
            .unwrap(),
    );
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs/environment_runtime.dart");
    let reference = scratch.0.join("reference.js");
    successful(
        Command::new(&dart)
            .arg(&snapshot)
            .args(["--modules=es6", "-o"])
            .arg(&reference)
            .arg(source.file_name().unwrap())
            .current_dir(source.parent().unwrap())
            .output()
            .unwrap(),
    );
    let code = std::fs::read_to_string(&reference)
        .unwrap()
        .replace("from 'dart_sdk.js'", "from './dart_sdk.js'");
    let export = code
        .lines()
        .find_map(|line| line.strip_prefix("export { "))
        .unwrap()
        .split([',', '}'])
        .next()
        .unwrap()
        .trim()
        .to_string();
    std::fs::write(&reference, code).unwrap();
    std::fs::write(scratch.0.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
    let runner = scratch.0.join("reference.mjs");
    std::fs::write(
        &runner,
        format!("import {{ {export} as app }} from './reference.js';\napp.main();\n"),
    )
    .unwrap();
    let expected = successful(Command::new("node").arg(&runner).output().unwrap());
    assert_eq!(expected, b"default first\nname later\ntrue\nfalse\nname first\ndefault later\ntrue\nfalse\nbool default\nbool name\ntrue\nfalse\ninvalid name first\ntrue\nfalse\npresence\ntrue\n");
    let (emitted, _) = dartforge_emit_js::compile_with_report(
        &source,
        Some(&sdk.join("lib")),
        None,
        &Default::default(),
    )
    .unwrap();
    let artifact = scratch.0.join("ours");
    dartforge_emit_js::write(&emitted, &artifact, &sdk_js).unwrap();
    assert_eq!(
        successful(
            Command::new("node")
                .arg(artifact.join("main.mjs"))
                .output()
                .unwrap()
        ),
        expected
    );
}
