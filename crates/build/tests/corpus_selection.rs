//! Regression checks for the VM/native builder corpus prerequisite gate.
#[path = "support/corpus_cases.rs"]
mod corpus_cases;

use corpus_cases::planned_cases;
use std::path::Path;

#[test]
fn execution_requires_no_pending_or_different_outputs() {
    use dartforge_build::{engine::Scoreboard, graph::AssetId};
    let id = AssetId::new("case", "lib/output.g.dart");
    for (pending, different, message) in [
        (
            vec![(id.clone(), "executor unavailable".into())],
            vec![],
            "pending",
        ),
        (
            vec![],
            vec![(id.clone(), "bytes differ".into())],
            "different",
        ),
    ] {
        let score = Scoreboard {
            pending,
            different,
            ..Default::default()
        };
        let error = corpus_cases::require_complete_outputs("example", &score).unwrap_err();
        assert!(
            error.contains("example")
                && error.contains(message)
                && error.contains("lib/output.g.dart"),
            "{error}"
        );
    }
    let score = Scoreboard {
        equal: vec![id.clone()],
        unrequested: vec![(id, "optional".into())],
        ..Default::default()
    };
    corpus_cases::require_complete_outputs("example", &score).unwrap();
}

fn case(root: &Path, name: &str, configured: bool) {
    let path = root.join(name);
    std::fs::create_dir_all(path.join("oraculo")).unwrap();
    std::fs::write(path.join("oraculo/plano.dart"), "// plan").unwrap();
    if configured {
        std::fs::create_dir_all(path.join(".dart_tool")).unwrap();
        std::fs::write(path.join(".dart_tool/package_config.json"), "{}").unwrap();
    }
}

#[test]
fn selects_sorted_planned_cases_and_excludes_the_no_builder_control() {
    let tmp = tempfile::tempdir().unwrap();
    case(tmp.path(), "second", true);
    case(tmp.path(), "first", true);
    std::fs::create_dir(tmp.path().join("no_builders")).unwrap();
    let names: Vec<_> = planned_cases(tmp.path(), None)
        .unwrap()
        .into_iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["first", "second"]);
}

#[test]
fn missing_configuration_fails_instead_of_skipping_a_case() {
    let tmp = tempfile::tempdir().unwrap();
    case(tmp.path(), "ready", true);
    case(tmp.path(), "unconfigured", false);
    let error = planned_cases(tmp.path(), None).unwrap_err();
    assert!(
        error.contains("unconfigured") && error.contains("pub get"),
        "{error}"
    );
    assert!(planned_cases(tmp.path(), Some(&["unconfigured".into()])).is_err());
    assert_eq!(
        planned_cases(tmp.path(), Some(&["ready".into()]))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn unknown_or_partially_unknown_filters_fail() {
    let tmp = tempfile::tempdir().unwrap();
    case(tmp.path(), "ready", true);
    for filter in [vec!["absent".into()], vec!["ready".into(), "absent".into()]] {
        assert!(
            planned_cases(tmp.path(), Some(&filter))
                .unwrap_err()
                .contains("absent")
        );
    }
}

#[test]
fn empty_filters_fail() {
    let tmp = tempfile::tempdir().unwrap();
    case(tmp.path(), "ready", true);
    for filter in [vec![], vec!["".into()], vec!["ready".into(), " ".into()]] {
        assert!(
            planned_cases(tmp.path(), Some(&filter))
                .unwrap_err()
                .contains("empty name")
        );
    }
}

#[test]
fn missing_or_empty_corpus_fails() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(
        planned_cases(&tmp.path().join("missing"), None)
            .unwrap_err()
            .contains("cannot read")
    );
    assert!(
        planned_cases(tmp.path(), None)
            .unwrap_err()
            .contains("no planned builder cases")
    );
}
