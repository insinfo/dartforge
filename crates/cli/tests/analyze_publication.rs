//! Default publication preserves verified assignment errors and existing ignores.
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn new(source: &str) -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/analyze-publication-tests")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        std::fs::create_dir_all(root.join("sdk/core")).unwrap();
        std::fs::write(
            root.join("sdk/libraries.json"),
            r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#,
        )
        .unwrap();
        std::fs::write(root.join("sdk/core/core.dart"),"class Object {} class bool {} class num {} class int extends num {} class double extends num {} class Null {} class String {}\n").unwrap();
        std::fs::write(
            root.join("pubspec.yaml"),
            "name: publication_fixture\nenvironment:\n  sdk: ^3.6.0\n",
        )
        .unwrap();
        std::fs::write(root.join("main.dart"), source).unwrap();
        Self { root }
    }
    fn analyze(&self, all: bool) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_dartforge"));
        command
            .args(["analyze", "--format=json", "--sdk-path"])
            .arg(self.root.join("sdk"));
        if all {
            command.arg("--all");
        }
        command
            .arg(self.root.join("main.dart"))
            .env("TMPDIR", &self.root)
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn report(output: &Output) -> serde_json::Value {
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn default_reports_invalid_assignment_with_same_exit_and_json_as_all() {
    let source =
        "void main() { int value = 'wrong'; consume(value); }\nvoid consume(Object? value) {}\n";
    let f = Fixture::new(source);
    let default = f.analyze(false);
    let all = f.analyze(true);
    assert_eq!(all.status.code(), Some(3));
    assert_eq!(default.status.code(), Some(3));
    let default = report(&default);
    assert_eq!(default, report(&all));
    let diagnostics = default["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    let d = &diagnostics[0];
    assert_eq!(d["code"], "invalid_assignment");
    assert_eq!(d["severity"], "ERROR");
    assert_eq!(
        d["problemMessage"],
        "A value of type 'String' can't be assigned to a variable of type 'int'."
    );
    assert_eq!(
        d["location"]["range"]["start"]["offset"],
        source.find("'wrong'").unwrap()
    );
    assert_eq!(
        d["location"]["range"]["end"]["offset"],
        source.find("'wrong'").unwrap() + 7
    );
}
#[test]
fn valid_assignments_are_not_published_as_errors() {
    let f = Fixture::new(
        "void main() { int count = 1; double amount = 1; Object value = 'ok'; consume(count); consume(amount); consume(value); }\nvoid consume(Object? value) {}\n",
    );
    for all in [false, true] {
        let output = f.analyze(all);
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(report(&output)["diagnostics"], serde_json::json!([]));
    }
}
#[test]
fn newly_published_assignment_respects_comment_and_option_ignores() {
    for comment in [true, false] {
        let f = Fixture::new(if comment {
            "void main() {\n// ignore: invalid_assignment\nint value = 'wrong'; consume(value);\n}\nvoid consume(Object? value) {}\n"
        } else {
            "void main() { int value = 'wrong'; consume(value); }\nvoid consume(Object? value) {}\n"
        });
        if !comment {
            std::fs::write(
                f.root.join("analysis_options.yaml"),
                "analyzer:\n  errors:\n    invalid_assignment: ignore\n",
            )
            .unwrap();
        }
        for all in [false, true] {
            let output = f.analyze(all);
            assert_eq!(output.status.code(), Some(0));
            assert_eq!(report(&output)["diagnostics"], serde_json::json!([]));
        }
    }
}

#[test]
fn cannot_ignore_keeps_the_published_assignment_error() {
    let f = Fixture::new(
        "void main() {\n// ignore: invalid_assignment\nint value = 'wrong'; consume(value);\n}\nvoid consume(Object? value) {}\n",
    );
    std::fs::write(
        f.root.join("analysis_options.yaml"),
        "analyzer:\n  cannot-ignore:\n    - invalid_assignment\n",
    )
    .unwrap();
    for all in [false, true] {
        let output = f.analyze(all);
        assert_eq!(output.status.code(), Some(3));
        assert_eq!(
            report(&output)["diagnostics"][0]["code"],
            "invalid_assignment"
        );
    }
}
