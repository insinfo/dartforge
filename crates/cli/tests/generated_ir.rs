//! Native IR and digest modes must use the same in-memory generation as AOT.
#![cfg(feature = "native")]

use std::path::PathBuf;
use std::process::{Command, Output};

struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "dartforge-generated-ir-{}-{unique}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        let fixture = Self(root);
        for directory in ["lib", "bin", ".dart_tool", "vendor/i18n/lib"] {
            std::fs::create_dir_all(fixture.0.join(directory)).unwrap();
        }
        fixture.write(
            "pubspec.yaml",
            "name: generated_app\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  i18n: 4.2.1\n",
        );
        fixture.write("pubspec.lock", "packages:\n  i18n:\n    version: '4.2.1'\n    description:\n      name: i18n\n      url: https://pub.dev\n    source: hosted\n    dependency: direct main\n");
        fixture.write(
            "vendor/i18n/pubspec.yaml",
            "name: i18n\nversion: 4.2.1\nenvironment:\n  sdk: ^3.6.0\n",
        );
        // The native port uses this descriptor and lock version. No external
        // builder, package cache, Pub invocation or pre-generated output is used.
        fixture.write(
            "vendor/i18n/build.yaml",
            r#"builders:
  yamlBasedBuilder:
    import: 'package:i18n/builder.dart'
    builder_factories: ['yamlBasedBuilder']
    build_extensions: {'.i18n.yaml': ['.i18n.dart']}
    build_to: source
    auto_apply: root_package
"#,
        );
        // Only declarations required by the generated, unused plural helpers;
        // this fixture tests engine routing, not i18n package compatibility.
        let library = ["plural", "ordinal", "cardinal"].map(|name| format!(
            "String {name}(int count, String locale, {{String? zero, String? one, String? two, String? few, String? many, String? other}}) => other ?? '';\n"
        )).concat();
        fixture.write("vendor/i18n/lib/i18n.dart", &library);
        fixture.write("bin/main.dart", "import 'package:generated_app/messages.i18n.dart';\nvoid main() { print(Messages().value); }\n");
        fixture.write("lib/messages.i18n.yaml", "value: engine first\n");
        fixture.write(".dart_tool/package_config.json", &serde_json::json!({
            "configVersion": 2,
            "packages": [
                {"name":"generated_app", "rootUri":"../", "packageUri":"lib/", "languageVersion":"3.6"},
                {"name":"i18n", "rootUri":"../vendor/i18n", "packageUri":"lib/", "languageVersion":"3.6"}
            ]
        }).to_string());
        fixture
    }
    fn write(&self, path: &str, value: &str) {
        std::fs::write(self.0.join(path), value).unwrap();
    }
    fn command(&self) -> Command {
        // An explicit path permits the same regression to be replayed against
        // a recorded previous compiler. Normal Cargo tests use the built CLI.
        let compiler = std::env::var_os("DARTFORGE_TEST_COMPILER")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_dartforge").into());
        eprintln!(
            "generated IR test compiler: {}",
            PathBuf::from(&compiler).display()
        );
        let mut command = Command::new(compiler);
        command
            .arg("compile-native")
            .arg(self.0.join("bin/main.dart"))
            .arg("--packages")
            .arg(self.0.join(".dart_tool/package_config.json"))
            .args([
                "--memory=tracing",
                "--dartforge-contracts=verify",
                "--language-version=3.6",
            ])
            .env("DARTFORGE_BUILD_NATIVE", "0")
            .env_remove("DARTFORGE_BUILD_DART")
            .env_remove("DARTFORGE_BUILD_COMPILING_EXECUTOR")
            .env_remove("DARTFORGE_BUILD_STATE");
        if let Some(sdk) = std::env::var_os("DARTFORGE_TEST_SDK_LIB") {
            command.arg("--sdk").arg(sdk);
        }
        command
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn digest(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    let words: Vec<_> = text.split_whitespace().collect();
    assert_eq!(words.len(), 2, "digest stdout: {text}");
    assert_eq!(words[0].len(), 32);
    words[1].parse::<usize>().unwrap();
    text
}

#[test]
fn ir_and_digest_generate_fresh_sources_without_materializing_them() {
    let project = Project::new();
    let ir = project.0.join("program.ll");
    let manifest = project.0.join("program.ll.contracts.json");
    let options = [
        "--digest",
        "--optimize",
        "--enable-asserts",
        "--debug",
        "--timings",
    ];
    let first = digest(
        project
            .command()
            .args(options)
            .arg("--emit-ir")
            .arg("-o")
            .arg(&ir)
            .output()
            .unwrap(),
    );
    let original_ir = std::fs::read(&ir).unwrap();
    let original_manifest = std::fs::read(&manifest).unwrap();
    assert!(String::from_utf8_lossy(&original_ir).contains("engine first"));
    project.write("lib/messages.i18n.yaml", "value: engine second\n");
    let second = digest(project.command().args(options).output().unwrap());
    assert_ne!(
        first, second,
        "an input edit must reach generation in digest-only mode"
    );
    assert_eq!(std::fs::read(&ir).unwrap(), original_ir);
    assert_eq!(std::fs::read(&manifest).unwrap(), original_manifest);
    let third = digest(
        project
            .command()
            .args(options)
            .arg("--emit-ir")
            .arg("-o")
            .arg(&ir)
            .output()
            .unwrap(),
    );
    assert_eq!(
        second, third,
        "IR and digest-only modes must see the same generation"
    );
    let text = std::fs::read_to_string(&ir).unwrap();
    assert!(text.contains("engine second") && !text.contains("engine first"));
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
    assert_eq!(
        report["artifact"]["ir_digest"],
        second.split_whitespace().next().unwrap()
    );
    assert!(!project.0.join("lib/messages.i18n.dart").exists());
    assert!(!project.0.join(".dart_tool/build").exists());
}

#[test]
fn invalid_builder_configuration_cannot_use_stale_sources_or_replace_artifacts() {
    let project = Project::new();
    project.write("vendor/i18n/build.yaml", "builders:\n  broken: [\n");
    project.write(
        "lib/messages.i18n.dart",
        "class Messages { String get value => 'stale fallback'; }\n",
    );
    let ir = project.0.join("program.ll");
    let manifest = project.0.join("program.ll.contracts.json");
    std::fs::write(&ir, b"previous IR").unwrap();
    std::fs::write(&manifest, b"previous manifest").unwrap();
    for emit in [false, true] {
        let mut command = project.command();
        command.arg("--digest");
        if emit {
            command.arg("--emit-ir").arg("-o").arg(&ir);
        }
        let output = command.output().unwrap();
        assert!(
            !output.status.success(),
            "stale disk source bypassed the invalid builder configuration"
        );
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(
            diagnostic.contains("invalid YAML") && diagnostic.contains("build.yaml"),
            "{diagnostic}"
        );
        assert!(output.stdout.is_empty());
        assert_eq!(std::fs::read(&ir).unwrap(), b"previous IR");
        assert_eq!(std::fs::read(&manifest).unwrap(), b"previous manifest");
    }
}

#[test]
fn quoted_and_flow_builder_definitions_generate_sources() {
    for flow in [false, true] {
        let project = Project::new();
        let path = project.0.join("vendor/i18n/build.yaml");
        let configuration = if flow {
            serde_json::json!({"builders":{"yamlBasedBuilder":{
                "import":"package:i18n/builder.dart", "builder_factories":["yamlBasedBuilder"],
                "build_extensions":{".i18n.yaml":[".i18n.dart"]},
                "build_to":"source", "auto_apply":"root_package"
            }}})
            .to_string()
        } else {
            std::fs::read_to_string(&path)
                .unwrap()
                .replacen("builders:", "'builders':", 1)
        };
        std::fs::write(path, configuration).unwrap();
        let ir = project.0.join("program.ll");
        digest(
            project
                .command()
                .args(["--digest", "--emit-ir", "-o"])
                .arg(&ir)
                .output()
                .unwrap(),
        );
        assert!(
            std::fs::read_to_string(ir)
                .unwrap()
                .contains("engine first")
        );
        assert!(!project.0.join("lib/messages.i18n.dart").exists());
    }
}

#[test]
fn invalid_non_block_configuration_cannot_be_bypassed_with_stale_sources() {
    for (configuration, diagnostic) in [
        (&b"'builders': [\n"[..], "invalid YAML"),
        (&b"{builders: []}\n"[..], "expected a map"),
        (&b"\xff\xfe"[..], "could not read"),
    ] {
        let project = Project::new();
        std::fs::write(project.0.join("vendor/i18n/build.yaml"), configuration).unwrap();
        project.write(
            "lib/messages.i18n.dart",
            "class Messages { String get value => 'stale fallback'; }\n",
        );
        let ir = project.0.join("program.ll");
        let manifest = project.0.join("program.ll.contracts.json");
        std::fs::write(&ir, b"previous IR").unwrap();
        std::fs::write(&manifest, b"previous manifest").unwrap();
        let output = project
            .command()
            .args(["--digest", "--emit-ir", "-o"])
            .arg(&ir)
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "invalid configuration bypassed generation"
        );
        let text = String::from_utf8_lossy(&output.stderr);
        assert!(
            text.contains("build.yaml") && text.contains(diagnostic),
            "{text}"
        );
        assert!(output.stdout.is_empty());
        assert_eq!(std::fs::read(&ir).unwrap(), b"previous IR");
        assert_eq!(std::fs::read(&manifest).unwrap(), b"previous manifest");
    }
}
