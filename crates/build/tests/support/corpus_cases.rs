//! Select planned builder cases without silently dropping missing prerequisites.
use std::path::{Path, PathBuf};

pub fn require_complete_outputs(
    case: &str,
    scoreboard: &dartforge_build::engine::Scoreboard,
) -> Result<(), String> {
    let failures: Vec<_> = [
        ("pending", &scoreboard.pending),
        ("different", &scoreboard.different),
    ]
    .into_iter()
    .flat_map(|(kind, outputs)| {
        outputs
            .iter()
            .map(move |(id, reason)| format!("{case}: {kind} {} ({reason})", id.text()))
    })
    .collect();
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

pub fn planned_cases(root: &Path, filter: Option<&[String]>) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(root)
        .map_err(|error| format!("cannot read corpus {}: {error}", root.display()))?;
    let mut cases = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|error| format!("cannot read corpus entry: {error}"))?
            .path();
        if path.join("oraculo/plano.dart").is_file() {
            cases.push(path);
        }
    }
    cases.sort();
    if cases.is_empty() {
        return Err(format!("no planned builder cases in {}", root.display()));
    }
    if let Some(names) = filter {
        if names.is_empty() || names.iter().any(|name| name.trim().is_empty()) {
            return Err("builder case filter contains an empty name".into());
        }
        for name in names {
            if !cases
                .iter()
                .any(|path| path.file_name().is_some_and(|n| n == name.as_str()))
            {
                return Err(format!("unknown builder case: {name}"));
            }
        }
        cases.retain(|path| {
            names
                .iter()
                .any(|name| path.file_name().is_some_and(|n| n == name.as_str()))
        });
    }
    for path in &cases {
        if !path.join(".dart_tool/package_config.json").is_file() {
            return Err(format!(
                "{}: missing .dart_tool/package_config.json; run dart pub get --enforce-lockfile",
                path.display()
            ));
        }
    }
    Ok(cases)
}
