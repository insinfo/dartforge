//! `dartforge test`: refusals that need no compilation, and a differential
//! run against `dart test` 3.6.2 on a package that exercises dart_test.yaml
//! and library annotations.
#![cfg(feature = "native")]

use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_dartforge"))
}

/// The differential run compiles many suites: `DARTFORGE_COMPILER` may
/// select an optimized native compiler instead of the test build.
fn compiler() -> Command {
    match std::env::var_os("DARTFORGE_COMPILER") {
        Some(path) => Command::new(path),
        None => cli(),
    }
}

#[test]
fn unsupported_runner_modes_are_refused_before_compiling() {
    for (args, message) in [
        (
            vec!["--platform", "chrome"],
            "only runs the vm platform; chrome",
        ),
        (vec!["-p", "vm,node"], "only runs the vm platform; node"),
        (vec!["--coverage=out"], "coverage collection"),
        (vec!["--pause-after-load"], "debugging"),
        (vec!["--fail-fast"], "--fail-fast"),
        (vec!["--watch"], "watch mode"),
        (
            vec!["--reporter", "github"],
            "--reporter must be expanded or json",
        ),
    ] {
        let output = cli().arg("test").args(&args).output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(64), "{args:?}: {stderr}");
        assert!(stderr.contains(message), "{args:?}: {stderr}");
    }
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// `(suite, test) -> outcomes` from a JSON event stream; load failures are
/// keyed as `loading`, and skipped tests count as `skipped`.
fn outcomes(stdout: &[u8], official: bool) -> BTreeMap<(String, String), Vec<String>> {
    let mut tests = BTreeMap::new();
    let mut suites = BTreeMap::new();
    let mut result: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for line in String::from_utf8_lossy(stdout).lines() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let kind = event["type"].as_str().unwrap_or("");
        if official {
            match kind {
                "suite" => {
                    suites.insert(
                        event["suite"]["id"].clone().to_string(),
                        event["suite"]["path"].as_str().unwrap().to_owned(),
                    );
                }
                "testStart" => {
                    tests.insert(event["test"]["id"].to_string(), event["test"].clone());
                }
                "testDone" => {
                    let test = &tests[&event["testID"].to_string()];
                    let mut name = test["name"].as_str().unwrap().to_owned();
                    if event["hidden"] == true && event["result"] == "success" {
                        continue;
                    }
                    if name.starts_with("loading ") {
                        name = "loading".into();
                    }
                    let outcome = if event["skipped"] == true {
                        "skipped".to_owned()
                    } else {
                        event["result"].as_str().unwrap().to_owned()
                    };
                    let suite = suites[&test["suiteID"].to_string()].clone();
                    result.entry((suite, name)).or_default().push(outcome);
                }
                _ => {}
            }
        } else {
            let suite = event["suite"].as_str().unwrap_or("").to_owned();
            match kind {
                "testDone" => result
                    .entry((suite, event["name"].as_str().unwrap().to_owned()))
                    .or_default()
                    .push(event["result"].as_str().unwrap().to_owned()),
                "loadError" => result
                    .entry((suite, "loading".into()))
                    .or_default()
                    .push("error".into()),
                _ => {}
            }
        }
    }
    result
}

#[test]
#[ignore = "needs DARTFORGE_DART_SDK (Dart 3.6.2), package:test 1.25.7 in PUB_CACHE, LLVM tools and a native SDK"]
fn configuration_and_annotations_match_dart_test() {
    let sdk = PathBuf::from(
        std::env::var_os("DARTFORGE_DART_SDK").expect("DARTFORGE_DART_SDK selects Dart 3.6.2"),
    );
    let dart = sdk
        .join("bin")
        .join(if cfg!(windows) { "dart.exe" } else { "dart" });
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let package = root.join(format!(
        "../../target/test-runner-cli-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&package);
    copy_tree(&root.join("tests/fixtures/test_runner/config"), &package);
    let resolved = Command::new(&dart)
        .args(["pub", "get", "--offline"])
        .current_dir(&package)
        .output()
        .unwrap();
    assert!(
        resolved.status.success(),
        "{}",
        String::from_utf8_lossy(&resolved.stderr)
    );
    for args in [
        vec![],
        vec!["-P", "ci"],
        vec!["--run-skipped", "-x", "quick"],
        vec!["-n", "library timeout$", "--timeout", "10x"],
        vec!["--total-shards", "2", "--shard-index", "1", "--no-retry"],
        vec!["test/missing_check.dart", "test/ignored_test.dart"],
        vec!["-t", "nothing-tagged"],
    ] {
        let official = Command::new(&dart)
            .args(["test", "-r", "json"])
            .args(&args)
            .current_dir(&package)
            .output()
            .unwrap();
        let native = compiler()
            .args(["test", "-r", "json", "--sdk"])
            .arg(sdk.join("lib"))
            .args(&args)
            .current_dir(&package)
            .output()
            .unwrap();
        assert_eq!(
            native.status.code(),
            official.status.code(),
            "{args:?}: {}",
            String::from_utf8_lossy(&native.stderr)
        );
        assert_eq!(
            outcomes(&native.stdout, false),
            outcomes(&official.stdout, true),
            "{args:?}: {}",
            String::from_utf8_lossy(&native.stderr)
        );
    }
    let _ = std::fs::remove_dir_all(&package);
}
