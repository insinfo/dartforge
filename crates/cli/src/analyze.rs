//! `dartforge analyze`: the diagnostics of directories or files, with the
//! output of the SDK 3.6.2 `dart analyze` in its three formats and its exit
//! code (docs/ANALYZER-ESPECIFICACAO-INFRA.md §5.3–§5.7, step 4 of Part
//! III). Parity is with the **redirected** mode: no ANSI, a `-` marker, one
//! line per diagnostic.
//!
//! Publishes only what the rule of plan §2.3 allows: syntax always, and the
//! semantic codes in `crates/analysis/verified.txt`. `--all` also shows
//! the unverified ones (for debugging, never for the editor).
//!
//! Rewritten on 2026-10-04 without compiling or running.
use dartforge_parity::json::DiagJson;
use std::path::{Path, PathBuf};

/// The package root: the nearest directory, going up, with a `pubspec.yaml`.
fn package_root(p: &Path) -> PathBuf {
    let start = if p.is_dir() {
        p.to_path_buf()
    } else {
        p.parent().unwrap_or(Path::new(".")).to_path_buf()
    };
    let mut d = Some(start.as_path());
    while let Some(x) = d {
        if x.join("pubspec.yaml").is_file() {
            return x.to_path_buf();
        }
        d = x.parent();
    }
    start
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Default,
    Json,
    Machine,
}

/// The order of dartdev's `AnalysisError.compareTo`
/// (`dartdev/lib/src/analysis_server.dart:385-402`): severity (error,
/// warning, info), absolute path compared by UTF-16 code units, offset and
/// message. Ties beyond that keep their arrival order.
fn sort(v: &mut [DiagJson]) {
    let priority = |s: &str| match s {
        "ERROR" => 0u8,
        "WARNING" => 1,
        "INFO" => 2,
        _ => 3,
    };
    v.sort_by(|a, b| {
        priority(&a.severity)
            .cmp(&priority(&b.severity))
            .then_with(|| {
                a.location
                    .file
                    .encode_utf16()
                    .cmp(b.location.file.encode_utf16())
            })
            .then_with(|| {
                a.location
                    .range
                    .start
                    .offset
                    .cmp(&b.location.range.start.offset)
            })
            .then_with(|| {
                a.problem_message
                    .encode_utf16()
                    .cmp(b.problem_message.encode_utf16())
            })
    });
}

/// The path with the platform separator.
fn native_path(path: &str) -> String {
    path.replace('/', std::path::MAIN_SEPARATOR_STR)
}

/// `_relativePath` (`analyze.dart:481-485`): relative to `base`, used only if
/// it is not longer than the absolute one.
fn relative(file: &str, base: &Path) -> String {
    let absolute = native_path(file);
    match Path::new(&absolute).strip_prefix(base) {
        Ok(rel) => {
            let rel = rel.to_string_lossy().into_owned();
            if rel.len() <= absolute.len() && !rel.is_empty() {
                rel
            } else {
                absolute
            }
        }
        Err(_) => absolute,
    }
}

/// `_escapeForMachineMode` (`analyze.dart:463-478`).
fn escape_for_machine(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\\' => out.push_str("\\\\"),
            '|' => out.push_str("\\|"),
            other => out.push(other),
        }
    }
    out
}

pub fn run(
    args: &[std::ffi::OsString],
) -> Result<std::process::ExitCode, Box<dyn std::error::Error>> {
    let usage = "usage: dartforge analyze [--format=default|json|machine] [--fatal-infos] [--[no-]fatal-warnings] \
                 [--packages=<file>] [--sdk-path=<directory>] [--all] [<directory|file>...]";
    let mut format = Format::Default;
    let mut all = false;
    let mut fatal_infos = false;
    let mut fatal_warnings = true;
    let mut requested_packages: Option<PathBuf> = None;
    let mut requested_sdk: Option<PathBuf> = None;
    let mut targets: Vec<PathBuf> = Vec::new();
    let mut arguments = args.iter();
    while let Some(a) = arguments.next() {
        match a.to_str() {
            Some("--format=json") => format = Format::Json,
            Some("--format=default") => format = Format::Default,
            Some("--format=machine") => format = Format::Machine,
            Some("--all") => all = true,
            Some("--fatal-infos") => fatal_infos = true,
            Some("--no-fatal-infos") => fatal_infos = false,
            Some("--fatal-warnings") => fatal_warnings = true,
            Some("--no-fatal-warnings") => fatal_warnings = false,
            Some("--sdk-path") => {
                let path = arguments
                    .next()
                    .ok_or("--sdk-path requires an SDK directory")?;
                if path.is_empty() || path.to_string_lossy().starts_with('-') {
                    return Err("--sdk-path requires an SDK directory".into());
                }
                requested_sdk = Some(PathBuf::from(path));
            }
            Some(s) if s.starts_with("--sdk-path=") => {
                let path = &s["--sdk-path=".len()..];
                if path.is_empty() {
                    return Err("--sdk-path requires an SDK directory".into());
                }
                requested_sdk = Some(PathBuf::from(path));
            }
            // Accepted and without effect here: the analysis runs in process,
            // with the SDK the engine discovers.
            Some("--memory") => {}
            Some(s) if s.starts_with("--cache=") || s.starts_with("--enable-experiment=") => {}
            Some(s) if s.starts_with("--packages=") => {
                requested_packages = Some(PathBuf::from(&s["--packages=".len()..]))
            }
            Some(s) if s.starts_with('-') => return Err(usage.into()),
            _ => targets.push(PathBuf::from(a)),
        }
    }
    // No target: the current directory, whose name goes in the header.
    let typed: Vec<PathBuf> = if targets.is_empty() {
        vec![std::env::current_dir()?]
    } else {
        targets
    };
    for d in &typed {
        if !d.exists() {
            return Err(format!("Directory or file doesn't exist: {}", d.display()).into());
        }
    }
    let absolutes: Vec<PathBuf> = typed
        .iter()
        .map(std::path::absolute)
        .collect::<Result<_, _>>()?;
    // `relativeToDir`: with one target, the target folder (or the target
    // file's folder); with several, the current directory.
    let relative_base: PathBuf = if absolutes.len() == 1 {
        let single = &absolutes[0];
        if single.is_dir() {
            single.clone()
        } else {
            single.parent().unwrap_or(Path::new(".")).to_path_buf()
        }
    } else {
        std::env::current_dir()?
    };
    let engine = match requested_sdk {
        Some(path) => {
            // Accept the SDK root and its lib directory. An explicit path is
            // authoritative: invalid layouts must not fall back to discovery.
            let lib = if path.join("libraries.json").is_file() {
                path.clone()
            } else {
                path.join("lib")
            };
            dartforge_parity::analysis::Engine::new(&lib)
                .map_err(|error| format!("--sdk-path {}: {error}", path.display()))?
        }
        None => dartforge_parity::analysis::Engine::discover()?,
    };
    // Deep bodies recurse deeply: own stack, like `compile-js`.
    let mut diags: Vec<DiagJson> = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let mut v = Vec::new();
            for target in &absolutes {
                let root = package_root(target);
                let options = dartforge_parity::filters::AnalysisOptions::read(&root);
                // The package's `pubspec.yaml`, when the target is its folder.
                if target.is_dir() && *target == root {
                    v.extend(dartforge_parity::pubspec_diagnostics(&root, &options));
                    v.extend(dartforge_parity::options_diagnostics(&root));
                    v.extend(dartforge_parity::manifest_diagnostics(&root, &options));
                }
                let files: Vec<PathBuf> = if target.is_dir() {
                    dartforge_parity::corpus::dart_files(target)
                        .into_iter()
                        .filter(|a| {
                            dartforge_parity::oracle::relative(a, &root)
                                .is_some_and(|r| !options.is_excluded(&r))
                        })
                        .collect()
                } else {
                    // The file requested on the command line is not excluded.
                    vec![target.clone()]
                };
                let package_cfg = root.join(".dart_tool").join("package_config.json");
                let default = requested_packages
                    .clone()
                    .or_else(|| package_cfg.is_file().then_some(package_cfg));
                // Nested packages (`example/`): each one with its own package_config.
                for (pkg_root, fs) in dartforge_parity::projects::by_package(&root, &files) {
                    let c = pkg_root.join(".dart_tool").join("package_config.json");
                    let c = if c.is_file() {
                        Some(c)
                    } else {
                        default.clone()
                    };
                    let a = engine.analyze(&root, &fs, c.as_deref());
                    v.extend(dartforge_parity::json_diagnostics(
                        &a, &root, &options, !all,
                    ));
                }
            }
            v
        })?
        .join()
        .map_err(|_| "the analysis aborted")?;
    // A `TODO` is only shown when `errors:` promoted it (`analyze.dart:191-192`).
    diags.retain(|d| !(d.error_type == "TODO" && d.severity == "INFO"));
    sort(&mut diags);
    let (mut errors, mut warnings, mut infos) = (false, false, false);
    for d in &diags {
        match d.severity.as_str() {
            "ERROR" => errors = true,
            "WARNING" => warnings = true,
            "INFO" => infos = true,
            _ => {}
        }
    }
    match format {
        Format::Json => {
            // One line, with the newline of Dart's `print` at the end.
            println!("{}", dartforge_parity::json::write(diags));
        }
        Format::Machine => {
            // No diagnostics, no output.
            for d in &diags {
                let length = d
                    .location
                    .range
                    .end
                    .offset
                    .saturating_sub(d.location.range.start.offset);
                println!(
                    "{}|{}|{}|{}|{}|{}|{}|{}",
                    d.severity,
                    d.error_type,
                    d.code.to_uppercase(),
                    escape_for_machine(&native_path(&d.location.file)),
                    d.location.range.start.line,
                    d.location.range.start.column,
                    length,
                    escape_for_machine(&d.problem_message)
                );
            }
        }
        Format::Default => {
            // The basename of each target AS TYPED (`.` stays `.`); with no
            // target, the name of the current directory.
            let names: Vec<String> = typed
                .iter()
                .map(|p| {
                    p.file_name().map_or_else(
                        || p.to_string_lossy().into_owned(),
                        |n| n.to_string_lossy().into_owned(),
                    )
                })
                .collect();
            println!("Analyzing {}...", names.join(", "));
            if diags.is_empty() {
                // No blank line before: it belongs to the error block.
                println!("No issues found!");
            } else {
                // EXTENSION POINT (step 7): the priority block of the
                // `pubspec.yaml`/`analysis_options.yaml` errors goes here, before.
                println!();
                for d in &diags {
                    let mut msg = d.problem_message.clone();
                    if let Some(c) = &d.correction_message {
                        msg.push(' ');
                        msg.push_str(c);
                    }
                    println!(
                        "{:>7} - {}:{}:{} - {} - {}",
                        d.severity.to_lowercase(),
                        relative(&d.location.file, &relative_base),
                        d.location.range.start.line,
                        d.location.range.start.column,
                        msg,
                        d.code
                    );
                    // The context messages: ten spaces, ` - `, the message
                    // without the final period, and the path of the file OF THE
                    // ERROR with the line and column of the message
                    // (`analyze.dart:371-378`).
                    for c in &d.context_messages {
                        println!(
                            "{} - {} at {}:{}:{}.",
                            " ".repeat(10),
                            c.message.trim_end_matches('.'),
                            relative(&d.location.file, &relative_base),
                            c.location.range.start.line,
                            c.location.range.start.column
                        );
                    }
                }
                println!();
                println!(
                    "{} issue{} found.",
                    diags.len(),
                    if diags.len() == 1 { "" } else { "s" }
                );
            }
        }
    }
    // `_Result` (`analyze.dart:489-508`): 3 with an error; 2 with a warning
    // (`--fatal-warnings` is the default); 1 with an info and `--fatal-infos`.
    Ok(std::process::ExitCode::from(if errors {
        3
    } else if warnings && fatal_warnings {
        2
    } else if infos && fatal_infos {
        1
    } else {
        0
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_format_escape() {
        assert_eq!(escape_for_machine("a|b\\c\nd"), "a\\|b\\\\c\\nd");
    }

    #[test]
    fn relative_only_when_not_longer() {
        let sep = std::path::MAIN_SEPARATOR_STR;
        let base = PathBuf::from(format!("{sep}root{sep}package"));
        let file = format!("{sep}root{sep}package{sep}lib{sep}a.dart");
        assert_eq!(relative(&file, &base), format!("lib{sep}a.dart"));
        let outside = format!("{sep}other{sep}a.dart");
        assert_eq!(relative(&outside, &base), outside);
    }
}
