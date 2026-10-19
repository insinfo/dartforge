//! Compare selected Dart files with the recorded oracle, or a fresh Dart 3.6.2
//! oracle in `--free` mode. Exit 0 means every diagnostic record matches; exit 1
//! means differences, invalid reference data or failed analysis; exit 2 is usage.
//!
//! ```text
//! cargo run -p dartforge-parity --example probe_files -- <group> <relative file>...
//! cargo run -p dartforge-parity --example probe_files -- --free <dir> [<relative file>...]
//! ```
//!
//! `--free` creates a synthetic language-3.6 package only when package metadata
//! is absent. Existing pubspec and package configuration files are preserved.
//! Automatic discovery skips dot-prefixed entries and rejects symbolic links.

use dartforge_parity::analysis::Engine;
use dartforge_parity::corpus::{self, Group};
use dartforge_parity::oracle::{self, OracleSdk, Record};
use dartforge_parity::{Execution, filters, run_ours};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

struct Setup {
    dir: PathBuf,
    config: PathBuf,
    oracle: Vec<Record>,
    files: Vec<String>,
}

enum Comparison<'a> {
    Equal(&'a Record),
    Changed(&'a Record, &'a Record),
    Missing(&'a Record),
    Extra(&'a Record),
}

type RecordKey<'a> = (usize, usize, &'a str);
type RecordSides<'a> = (Vec<&'a Record>, Vec<&'a Record>);

/// Preserve multiplicity and reserve exact matches before metadata mismatches.
fn compare_file<'a>(oracle: &'a [Record], ours: &'a [Record], file: &str) -> Vec<Comparison<'a>> {
    let mut keys: BTreeMap<RecordKey<'_>, RecordSides<'_>> = BTreeMap::new();
    for r in oracle.iter().filter(|r| r.file == file) {
        keys.entry((r.offset, r.length, &r.code))
            .or_default()
            .0
            .push(r);
    }
    for r in ours.iter().filter(|r| r.file == file) {
        keys.entry((r.offset, r.length, &r.code))
            .or_default()
            .1
            .push(r);
    }
    let mut comparisons = Vec::new();
    for (_, (mut expected, mut actual)) in keys {
        expected.sort();
        actual.sort();
        let mut unmatched = Vec::new();
        for record in expected {
            if let Some(index) = actual.iter().position(|candidate| *candidate == record) {
                comparisons.push(Comparison::Equal(record));
                actual.remove(index);
            } else {
                unmatched.push(record);
            }
        }
        let mut actual = actual.into_iter();
        for record in unmatched {
            comparisons.push(match actual.next() {
                Some(found) => Comparison::Changed(record, found),
                None => Comparison::Missing(record),
            });
        }
        comparisons.extend(actual.map(Comparison::Extra));
    }
    comparisons
}

fn run(args: &[String]) -> Result<bool, String> {
    let (first, rest) = args.split_first().ok_or("missing arguments")?;
    let setup = if first == "--free" {
        let (directory, files) = rest.split_first().ok_or("--free needs a directory")?;
        free(Path::new(directory), files)?
    } else {
        from_corpus(first, rest)?
    };
    let engine = match std::env::var_os("DARTFORGE_SDK_LIB") {
        Some(path) => Engine::new(&PathBuf::from(path)),
        None => Engine::discover(),
    }?;
    let sources: Vec<PathBuf> = setup.files.iter().map(|r| setup.dir.join(r)).collect();
    let execution = Execution {
        workers: 1,
        batch_size: sources.len(),
        progress: false,
        isolate: None,
        max_time: std::time::Duration::from_secs(600),
    };
    let result = run_ours(
        &engine,
        &setup.dir,
        &sources,
        Some(&setup.config),
        &filters::AnalysisOptions::read(&setup.dir),
        &execution,
    );
    let mut sum = [0usize; 4];
    for file in &setup.files {
        println!("== {file}");
        for comparison in compare_file(&setup.oracle, &result.records, file) {
            let (mark, index, record, actual) = match comparison {
                Comparison::Equal(r) => ("=", 0, r, None),
                Comparison::Changed(r, found) => ("~", 1, r, Some(found)),
                Comparison::Missing(r) => ("-", 2, r, None),
                Comparison::Extra(r) => ("+", 3, r, None),
            };
            sum[index] += 1;
            println!(
                "{mark} {}:{} (+{}) {}: {}",
                record.line, record.column, record.length, record.code, record.problem_message
            );
            if let Some(actual) = actual {
                println!(
                    "    oracle: {} {} correction {:?}",
                    record.severity, record.error_type, record.correction_message
                );
                println!(
                    "    ours: {}:{} (+{}) {} {} {:?} correction {:?}",
                    actual.line,
                    actual.column,
                    actual.length,
                    actual.severity,
                    actual.error_type,
                    actual.problem_message,
                    actual.correction_message
                );
            }
        }
    }
    println!(
        "total: = {} | ~ {} | - {} | + {}",
        sum[0], sum[1], sum[2], sum[3]
    );
    if !result.panics.is_empty() {
        eprintln!("panics: {:?}", result.panics);
    }
    if result.ambiguous != 0 {
        eprintln!("ambiguous diagnostics: {}", result.ambiguous);
    }
    Ok(sum[1..] == [0, 0, 0] && result.panics.is_empty() && result.ambiguous == 0)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: probe_files <group> <file>... | --free <dir> [<file>...]");
        return ExitCode::from(2);
    }
    match run(&args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

/// Unlike opportunistic corpus discovery, an unreadable subtree is an error here.
fn checked_dart_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in
            std::fs::read_dir(&directory).map_err(|e| format!("{}: {e}", directory.display()))?
        {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name();
            let name = name.to_str().ok_or("source file name is not UTF-8")?;
            if name.starts_with('.') {
                continue;
            }
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            if kind.is_symlink() {
                return Err(format!(
                    "symbolic link in source traversal: {}",
                    entry.path().display()
                ));
            }
            if kind.is_dir() {
                stack.push(entry.path());
            } else if name.ends_with(".dart") {
                if !kind.is_file() {
                    return Err(format!(
                        "not a regular Dart file: {}",
                        entry.path().display()
                    ));
                }
                files.push(entry.path());
            }
        }
    }
    files.sort();
    Ok(files)
}

fn select_files(root: &Path, files: &[String]) -> Result<Vec<String>, String> {
    if files.is_empty() {
        return Err("no Dart input files".into());
    }
    let mut selected = Vec::new();
    let mut seen = BTreeSet::new();
    for file in files {
        let path = Path::new(file);
        if path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, Component::Prefix(_) | Component::RootDir))
        {
            return Err(format!("input must be relative to the package: {file}"));
        }
        let path = std::fs::canonicalize(root.join(path)).map_err(|e| format!("{file}: {e}"))?;
        if !path.is_file() || path.extension().is_none_or(|ext| ext != "dart") {
            return Err(format!("input is not a Dart file: {file}"));
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| format!("input is outside the package: {file}"))?
            .to_str()
            .ok_or("source file name is not UTF-8")?
            .replace('\\', "/");
        if !seen.insert(relative.clone()) {
            return Err(format!("duplicate input: {file}"));
        }
        selected.push(relative);
    }
    Ok(selected)
}

fn checked_hash(dir: &Path, files: &[PathBuf]) -> Result<String, String> {
    let mut hash = oracle::FNV_START;
    for file in files {
        let relative = oracle::relative(file, dir).ok_or("source is outside the package")?;
        hash = oracle::fnv(relative.as_bytes(), hash);
        hash = oracle::fnv(&[0], hash);
        hash = oracle::fnv(
            &std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?,
            hash,
        );
    }
    Ok(format!("{hash:016x}"))
}

fn from_corpus(group: &str, files: &[String]) -> Result<Setup, String> {
    let components: Vec<_> = Path::new(group).components().collect();
    if !matches!(components.as_slice(), [Component::Normal(_)]) || group.starts_with('.') {
        return Err("group must be a corpus directory name".into());
    }
    let dir =
        std::fs::canonicalize(corpus::corpus_root().join(group)).map_err(|e| e.to_string())?;
    let config: Group = serde_json::from_str(
        &std::fs::read_to_string(dir.join("group.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("group.json: {e}"))?;
    recorded(&dir, group, &config, files)
}

fn recorded(dir: &Path, name: &str, group: &Group, files: &[String]) -> Result<Setup, String> {
    let files = select_files(dir, files)?;
    let sources = checked_dart_files(dir)?;
    let (records, meta) = oracle::read(dir)?;
    if meta.sdk != group.sdk
        || meta.files != sources.len()
        || meta.hash != checked_hash(dir, &sources)?
    {
        return Err(
            "recorded oracle metadata or source hash is out of date; re-record the group".into(),
        );
    }
    let known: BTreeSet<_> = sources
        .iter()
        .filter_map(|p| oracle::relative(p, dir))
        .collect();
    if records.iter().any(|r| !known.contains(&r.file)) {
        return Err("recorded oracle refers to a file outside the source set".into());
    }
    let config = corpus::prepare(dir, name, group).map_err(|e| e.to_string())?;
    Ok(Setup {
        dir: dir.to_path_buf(),
        config,
        oracle: records,
        files,
    })
}

/// Preserve existing metadata; synthetic metadata is created only for a fresh package.
fn free_config(dir: &Path) -> Result<PathBuf, String> {
    let pubspec = dir.join("pubspec.yaml");
    let config = dir.join(".dart_tool/package_config.json");
    let has_pubspec = pubspec.try_exists().map_err(|e| e.to_string())?;
    let has_config = config.try_exists().map_err(|e| e.to_string())?;
    if has_pubspec || has_config {
        if !pubspec.is_file() || !config.is_file() {
            return Err("existing package needs pubspec.yaml and .dart_tool/package_config.json; run dart pub get".into());
        }
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if json["configVersion"] != 2 || !json["packages"].is_array() {
            return Err(
                "unsupported package configuration; expected configVersion 2 and packages".into(),
            );
        }
        dartforge_elements::config::PackageConfig::load(&config)?;
        return Ok(config);
    }
    let name = "probe_free";
    std::fs::write(
        &pubspec,
        format!(
            "name: {}\npublish_to: none\nenvironment:\n  sdk: ^3.6.0\n",
            corpus::package_name(name)
        ),
    )
    .map_err(|e| e.to_string())?;
    corpus::prepare(
        dir,
        name,
        &Group {
            sdk: OracleSdk::V362,
            packages: vec![],
            origin: "free probe".into(),
        },
    )
    .map_err(|e| e.to_string())
}

fn free(dir: &Path, files: &[String]) -> Result<Setup, String> {
    let dir = std::fs::canonicalize(dir).map_err(|e| e.to_string())?;
    let inputs = if files.is_empty() {
        checked_dart_files(&dir)?
            .iter()
            .map(|p| oracle::relative(p, &dir).ok_or("source is outside package"))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        files.to_vec()
    };
    let files = select_files(&dir, &inputs)?;
    let config = free_config(&dir)?;
    let targets: Vec<_> = files.iter().map(|p| dir.join(p)).collect();
    let cache = tempfile::Builder::new()
        .prefix("dartforge-probe-")
        .tempdir()
        .map_err(|e| e.to_string())?;
    let diagnostics = oracle::run(OracleSdk::V362, &dir, &targets, cache.path())?;
    let records = diagnostics
        .iter()
        .filter_map(|d| Record::from_json(d, &dir))
        .collect();
    Ok(Setup {
        dir,
        config,
        oracle: records,
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(message: &str) -> Record {
        Record {
            file: "main.dart".into(),
            offset: 0,
            length: 4,
            code: "test".into(),
            severity: "ERROR".into(),
            error_type: "COMPILE_TIME_ERROR".into(),
            line: 1,
            column: 1,
            problem_message: message.into(),
            correction_message: None,
        }
    }

    #[test]
    fn duplicates_cannot_disappear_from_the_comparison() {
        let expected = [record("same")];
        let actual = [record("same"), record("same")];
        let comparisons = compare_file(&expected, &actual, "main.dart");
        assert_eq!(comparisons.len(), 2);
        assert!(matches!(comparisons[0], Comparison::Equal(_)));
        assert!(matches!(comparisons[1], Comparison::Extra(_)));
        let comparisons = compare_file(&actual, &expected, "main.dart");
        assert_eq!(comparisons.len(), 2);
        assert!(matches!(comparisons[1], Comparison::Missing(_)));
    }

    #[test]
    fn metadata_and_coordinates_are_part_of_an_exact_match() {
        let expected = [record("same")];
        for field in 0..6 {
            let mut actual = record("same");
            match field {
                0 => actual.severity = "WARNING".into(),
                1 => actual.error_type = "SYNTACTIC_ERROR".into(),
                2 => actual.line += 1,
                3 => actual.column += 1,
                4 => actual.correction_message = Some("correction".into()),
                _ => actual.problem_message = "different".into(),
            }
            let actual = [actual];
            assert!(matches!(
                compare_file(&expected, &actual, "main.dart").as_slice(),
                [Comparison::Changed(_, _)]
            ));
        }
    }

    #[test]
    fn exact_matches_are_not_consumed_by_other_duplicate_keys() {
        let expected = [record("a"), record("b")];
        let actual = [record("b")];
        let comparisons = compare_file(&expected, &actual, "main.dart");
        assert!(
            matches!(comparisons.as_slice(), [Comparison::Equal(r), Comparison::Missing(_)] if r.problem_message == "b")
        );
    }

    #[test]
    fn selection_rejects_empty_missing_duplicate_and_outside_inputs() {
        let temp = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(temp.path()).unwrap();
        let package = root.join("package");
        std::fs::create_dir(&package).unwrap();
        std::fs::write(package.join("main.dart"), "void main() {}\n").unwrap();
        std::fs::write(root.join("outside.dart"), "void main() {}\n").unwrap();
        for inputs in [
            vec![],
            vec!["missing.dart".into()],
            vec!["main.dart".into(), "./main.dart".into()],
            vec!["../outside.dart".into()],
        ] {
            assert!(select_files(&package, &inputs).is_err());
        }
        assert_eq!(
            select_files(&package, &["./main.dart".into()]).unwrap(),
            vec!["main.dart"]
        );
        assert!(checked_dart_files(&root.join("missing")).is_err());
    }

    #[test]
    fn existing_package_metadata_is_preserved_and_incomplete_metadata_is_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let pubspec = b"name: existing\nenvironment:\n  sdk: ^3.6.0\n";
        std::fs::write(root.join("pubspec.yaml"), pubspec).unwrap();
        assert!(free_config(root).is_err());
        assert_eq!(std::fs::read(root.join("pubspec.yaml")).unwrap(), pubspec);
        std::fs::create_dir(root.join(".dart_tool")).unwrap();
        let config = br#"{"configVersion":2,"packages":[{"name":"existing","rootUri":"../","languageVersion":"3.6"}]}"#;
        std::fs::write(root.join(".dart_tool/package_config.json"), config).unwrap();
        free_config(root).unwrap();
        assert_eq!(std::fs::read(root.join("pubspec.yaml")).unwrap(), pubspec);
        assert_eq!(
            std::fs::read(root.join(".dart_tool/package_config.json")).unwrap(),
            config
        );
    }

    #[test]
    fn recorded_oracle_requires_current_sources_counts_and_sdk() {
        let temp = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(temp.path()).unwrap();
        let file = root.join("main.dart");
        std::fs::write(&file, "void main() {}\n").unwrap();
        let group = Group {
            sdk: OracleSdk::V362,
            packages: vec![],
            origin: "test".into(),
        };
        let hash = checked_hash(&root, std::slice::from_ref(&file)).unwrap();
        let mut meta = oracle::Meta {
            sdk: OracleSdk::V362,
            hash,
            files: 1,
            diagnostics: 0,
            excluded: vec![],
        };
        oracle::write(&root, vec![], &meta).unwrap();
        recorded(&root, "test", &group, &["main.dart".into()]).unwrap();
        std::fs::write(&file, "void main() { print(1); }\n").unwrap();
        assert!(recorded(&root, "test", &group, &["main.dart".into()]).is_err());
        meta.hash = checked_hash(&root, &[file]).unwrap();
        meta.sdk = OracleSdk::V3134;
        oracle::write(&root, vec![], &meta).unwrap();
        assert!(recorded(&root, "test", &group, &["main.dart".into()]).is_err());
        meta.sdk = OracleSdk::V362;
        meta.files = 2;
        oracle::write(&root, vec![], &meta).unwrap();
        assert!(recorded(&root, "test", &group, &["main.dart".into()]).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn discovery_rejects_directory_symlinks_instead_of_looping() {
        let temp = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(temp.path(), temp.path().join("loop")).unwrap();
        assert!(checked_dart_files(temp.path()).is_err());
    }
}
