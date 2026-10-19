//! Detection agrees with the configuration reader, regardless of YAML spelling.
use dartforge_build::{config::BuildConfig, detect, instance_count};
use dartforge_elements::config::PackageConfig;
use std::path::PathBuf;

struct Project(tempfile::TempDir);
impl Project {
    fn new() -> Self {
        let project = Self(tempfile::tempdir().unwrap());
        for path in ["root/.dart_tool", "root/lib", "provider/lib", "runner/lib"] {
            std::fs::create_dir_all(project.0.path().join(path)).unwrap();
        }
        project.write_config(false);
        project
    }
    fn configuration(&self) -> PathBuf {
        self.0.path().join("provider/build.yaml")
    }
    fn write_config(&self, runner: bool) {
        let mut packages = vec![
            serde_json::json!({"name":"root","rootUri":"../","packageUri":"lib/"}),
            serde_json::json!({"name":"provider","rootUri":"../../provider/","packageUri":"lib/"}),
        ];
        if runner {
            packages.push(serde_json::json!({"name":"build_runner","rootUri":"../../runner/","packageUri":"lib/"}));
        }
        std::fs::write(
            self.0.path().join("root/.dart_tool/package_config.json"),
            serde_json::json!({"configVersion":2,"packages":packages}).to_string(),
        )
        .unwrap();
    }
    fn cfg(&self) -> PackageConfig {
        PackageConfig::load(&self.0.path().join("root/.dart_tool/package_config.json")).unwrap()
    }
}

#[test]
fn builder_definitions_in_resolved_dependencies_accept_equivalent_yaml_forms() {
    let entry = "{make: {import: 'make.dart', builder_factories: [make], build_extensions: {'.txt': ['.dart']}}}";
    for text in [
        format!("builders: {entry}\n"),
        format!("'builders': {entry}\n"),
        format!("  builders: {entry}\n"),
        format!("? builders\n: {entry}\n"),
        format!("builders: &definitions {entry}\n"),
        serde_json::json!({"builders":{"make":{"import":"make.dart", "builder_factories":["make"], "build_extensions":{".txt":[".dart"]}}}}).to_string(),
        "post_process_builders: {clean: {import: 'clean.dart', builder_factory: clean, input_extensions: ['.txt']}}\n".into(),
    ] {
        let project = Project::new();
        std::fs::write(project.configuration(), &text).unwrap();
        let parsed = BuildConfig::from_dir("provider", &[], project.configuration().parent().unwrap()).unwrap();
        assert_eq!(parsed.builders.len() + parsed.post_process_builders.len(), 1, "{text}");
        assert!(detect(&project.cfg()), "valid definitions were missed: {text}");
    }
}

#[test]
fn empty_sections_nested_options_and_other_documents_do_not_create_an_engine() {
    for text in [
        "",
        "# builders:\n",
        "builders:\npost_process_builders:\n",
        "{builders: {}, post_process_builders: {}}\n",
        "targets: {$default: {builders: {provider:make: {enabled: false}}}}\n",
        "additional_public_assets: ['builders:']\n",
        "{}\n---\nbuilders:\n  make: {import: 'make.dart', builder_factories: [make], build_extensions: {'.txt': ['.dart']}}\n",
    ] {
        let project = Project::new();
        std::fs::write(project.configuration(), text).unwrap();
        let parsed =
            BuildConfig::from_dir("provider", &[], project.configuration().parent().unwrap())
                .unwrap();
        assert!(parsed.builders.is_empty() && parsed.post_process_builders.is_empty());
        assert!(
            !detect(&project.cfg()),
            "no definitions in the selected document: {text}"
        );
    }
    assert_eq!(
        instance_count(),
        0,
        "detection must not construct an engine"
    );
}

#[test]
fn malformed_yaml_and_invalid_definition_shapes_require_validation() {
    for text in [
        "[\n",
        "'builders': [\n",
        "{builders: []}\n",
        "builders: false\n",
        "[]\n",
    ] {
        let project = Project::new();
        std::fs::write(project.configuration(), text).unwrap();
        assert!(
            BuildConfig::from_dir("provider", &[], project.configuration().parent().unwrap())
                .is_err()
        );
        assert!(
            detect(&project.cfg()),
            "invalid configuration was treated as absent: {text}"
        );
    }
}

#[test]
fn only_absence_is_ignored_while_unreadable_configuration_requires_validation() {
    let project = Project::new();
    assert!(!detect(&project.cfg()));
    std::fs::write(project.configuration(), [0xff, 0xfe]).unwrap();
    assert!(
        detect(&project.cfg()),
        "non-UTF-8 configuration must not bypass validation"
    );
    std::fs::remove_file(project.configuration()).unwrap();
    std::fs::create_dir(project.configuration()).unwrap();
    assert!(
        detect(&project.cfg()),
        "a directory named build.yaml is not an absent file"
    );
}

#[test]
fn resolved_build_runner_preserves_the_explicit_engine_trigger() {
    let project = Project::new();
    assert!(!detect(&project.cfg()));
    project.write_config(true);
    assert!(detect(&project.cfg()));
}
