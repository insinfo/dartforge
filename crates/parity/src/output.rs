//! **Project** parity harness (docs/ANALYZER-ESPECIFICACAO-INFRA.md, III.1):
//! compares the whole output of `dartforge analyze` with the recorded output
//! of `dart analyze` 3.6.2, in the three formats, plus the exit code.
//!
//! Each case is a folder of `<dir>` with:
//! * `projeto/`: the minimal project (`pubspec.yaml`, sources, optionally
//!   `analysis_options.yaml` and `.dart_tool/package_config.json`);
//! * `ARGS`: the arguments after `analyze`, one per line, relative to the
//!   project (without the file, `.`);
//! * `ESPERADO.default.txt`, `ESPERADO.json.txt`, `ESPERADO.machine.txt` and
//!   `ESPERADO.codigo` (the latter optional).
//!
//! Normalization replaces the project's absolute path with `<raiz>` (in both
//! JSON spellings, with `\\` and with `/`). `--gravar` runs `dart` (the one on
//! PATH, or `DARTFORGE_DART`) with the analyzer state in a temporary folder
//! and re-records the expectations.
//!
use std::path::{Path, PathBuf};
use std::process::Command;

const FORMATS: [&str; 3] = ["default", "json", "machine"];

/// The output of a command: the normalized text and the exit code.
struct Output {
    text: String,
    code: i32,
}

fn normalize(text: &str, root: &Path) -> String {
    let absolute = std::path::absolute(root).unwrap_or_else(|_| root.to_path_buf());
    let a = absolute.to_string_lossy().to_string();
    let mut s = text.replace("\r\n", "\n");
    // JSON: escaped backslashes.
    s = s.replace(&a.replace('\\', "\\\\"), "<raiz>");
    s = s.replace(&a, "<raiz>");
    s = s.replace(&a.replace('\\', "/"), "<raiz>");
    s
}

fn case_args(case: &Path) -> Vec<String> {
    let v: Vec<String> = std::fs::read_to_string(case.join("ARGS"))
        .map(|t| t.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect())
        .unwrap_or_default();
    if v.is_empty() { vec![".".to_string()] } else { v }
}

/// The `dartforge` executable next to this one (the same `target/<profile>`).
fn dartforge() -> PathBuf {
    let name = if cfg!(windows) { "dartforge.exe" } else { "dartforge" };
    std::env::current_exe().ok().map(|e| e.with_file_name(name)).unwrap_or_else(|| PathBuf::from(name))
}

fn dart() -> PathBuf {
    std::env::var_os("DARTFORGE_DART").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("dart"))
}

fn run(program: &Path, prefix: &[&str], format: &str, args: &[String], project: &Path, state: Option<&Path>) -> Option<Output> {
    let mut c = Command::new(program);
    c.args(prefix).arg(format!("--format={format}")).args(args).current_dir(project);
    if let Some(e) = state {
        c.env("ANALYZER_STATE_LOCATION_OVERRIDE", e);
    }
    let out = c.output().ok()?;
    Some(Output { text: normalize(&String::from_utf8_lossy(&out.stdout), project), code: out.status.code().unwrap_or(-1) })
}

/// The system path separator does not matter: the JSON's escaped backslash
/// and the plain one both become `/`, on both sides.
fn separators(s: &str) -> String {
    s.replace("\\\\", "/").replace('\\', "/")
}

/// The first differing line (1-based), with both versions.
fn first_difference(expected: &str, actual: &str) -> Option<(usize, String, String)> {
    let (e, o): (Vec<&str>, Vec<&str>) = (expected.split('\n').collect(), actual.split('\n').collect());
    for i in 0..e.len().max(o.len()) {
        let (a, b) = (e.get(i).copied().unwrap_or("<end>"), o.get(i).copied().unwrap_or("<end>"));
        if a != b {
            return Some((i + 1, a.to_string(), b.to_string()));
        }
    }
    None
}

/// The cases of `dir`, in order.
fn cases(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.join("projeto").is_dir()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// Re-records each case's expectations with `dart analyze`.
pub fn record(dir: &Path) -> bool {
    let state = std::env::temp_dir().join("dartforge-saida-analyze");
    let _ = std::fs::create_dir_all(&state);
    let mut ok = true;
    for case in cases(dir) {
        let project = case.join("projeto");
        let args = case_args(&case);
        let mut code = None;
        for f in FORMATS {
            match run(&dart(), &["analyze"], f, &args, &project, Some(&state)) {
                Some(s) => {
                    let _ = std::fs::write(case.join(format!("ESPERADO.{f}.txt")), &s.text);
                    code = Some(s.code);
                }
                None => {
                    eprintln!("{}: dart did not run", case.display());
                    ok = false;
                }
            }
        }
        if let Some(c) = code {
            let _ = std::fs::write(case.join("ESPERADO.codigo"), format!("{c}\n"));
        }
    }
    ok
}

/// Compares each case in the three formats; returns whether everything matched.
pub fn compare(dir: &Path) -> bool {
    let mut all_ok = true;
    let all = cases(dir);
    if all.is_empty() {
        eprintln!("no case in {}", dir.display());
        return false;
    }
    for case in all {
        let project = case.join("projeto");
        let args = case_args(&case);
        let name = case.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        for f in FORMATS {
            let Ok(expected) = std::fs::read_to_string(case.join(format!("ESPERADO.{f}.txt"))) else { continue };
            let expected = separators(&expected.replace("\r\n", "\n"));
            // `--todos`: the whole output, without the publication rule (which
            // only leaves the verified codes in the CLI); what is compared here
            // is the format, the order, the paths and the summary.
            let Some(actual) = run(&dartforge(), &["analyze", "--todos"], f, &args, &project, None) else {
                println!("{name} [{f}]: dartforge did not run");
                all_ok = false;
                continue;
            };
            if let Some((line, e, o)) = first_difference(&expected, &separators(&actual.text)) {
                println!("{name} [{f}] line {line}:\n  expected: {e}\n  actual:   {o}");
                all_ok = false;
            }
            if f == "default"
                && let Ok(c) = std::fs::read_to_string(case.join("ESPERADO.codigo"))
                && let Ok(c) = c.trim().parse::<i32>()
                && c != actual.code
            {
                println!("{name}: expected exit code {c}, got {}", actual.code);
                all_ok = false;
            }
        }
    }
    if all_ok {
        println!("project output: all equal");
    }
    all_ok
}
