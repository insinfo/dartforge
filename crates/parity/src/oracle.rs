//! The oracle: `dart analyze --format=json` from the official SDK, and the
//! record of what it said, written to disk next to the corpus.
//!
//! Two SDKs, chosen by the library's language version (plan §2.4): 3.6.2 for
//! libraries up to 3.6, 3.13.4 for those declaring 3.7 or later
//! (`// @dart = 3.7`). The record (`oraculo.jsonl`) holds one diagnostic per
//! line, with the path relative to the group; CI compares against it without
//! needing `dart`. `oraculo.json` holds the SDK and the hash of the sources: a
//! source changed without re-recording is flagged by the scoreboard.

use crate::json::{DiagJson, Report};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

/// An official SDK used as the oracle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum OracleSdk {
    /// Dart 3.6.2 (`E:/DartSDKs/3.6.2`).
    #[serde(rename = "3.6.2")]
    V362,
    /// Dart 3.13.4 (`E:/DartSDKs/3.13.4/dart-sdk`).
    #[serde(rename = "3.13.4")]
    V3134,
}

impl OracleSdk {
    pub fn version(self) -> &'static str {
        match self {
            OracleSdk::V362 => "3.6.2",
            OracleSdk::V3134 => "3.13.4",
        }
    }

    /// The `environment: sdk:` constraint of the group's pubspec.
    pub fn constraint(self) -> &'static str {
        match self {
            OracleSdk::V362 => "^3.6.0",
            OracleSdk::V3134 => "^3.13.0",
        }
    }

    /// The group's default language version.
    pub fn language(self) -> &'static str {
        match self {
            OracleSdk::V362 => "3.6",
            OracleSdk::V3134 => "3.13",
        }
    }

    /// Does this SDK's `dart` executable exist on this machine?
    pub fn available(self) -> bool {
        self.root().join("bin").join(if cfg!(windows) { "dart.exe" } else { "dart" }).is_file()
    }

    /// Where the SDK is on this machine (`DARTFORGE_ORACLE_3_6`/`_3_13` override it).
    pub fn root(self) -> PathBuf {
        let (var, default) = match self {
            OracleSdk::V362 => ("DARTFORGE_ORACLE_3_6", "E:/DartSDKs/3.6.2"),
            OracleSdk::V3134 => ("DARTFORGE_ORACLE_3_13", "E:/DartSDKs/3.13.4/dart-sdk"),
        };
        std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(default))
    }
}

/// One line of the oracle record.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    /// Path relative to the group's root, with `/`.
    #[serde(rename = "arquivo")]
    pub file: String,
    pub offset: usize,
    pub length: usize,
    pub code: String,
    pub severity: String,
    #[serde(rename = "type")]
    pub error_type: String,
    pub line: usize,
    pub column: usize,
    pub problem_message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correction_message: Option<String>,
}

impl Record {
    /// From a JSON v1 diagnostic, relative to `root` (`None` outside it or
    /// outside `.dart` files).
    pub fn from_json(d: &DiagJson, root: &Path) -> Option<Record> {
        let file = relative(Path::new(&d.location.file), root)?;
        if !file.ends_with(".dart") {
            return None;
        }
        let r = &d.location.range;
        Some(Record {
            file,
            offset: r.start.offset,
            length: r.end.offset.saturating_sub(r.start.offset),
            code: d.code.clone(),
            severity: d.severity.clone(),
            error_type: d.error_type.clone(),
            line: r.start.line,
            column: r.start.column,
            problem_message: without_root(&d.problem_message, root),
            correction_message: d.correction_message.as_deref().map(|m| without_root(m, root)),
        })
    }
}

/// Replaces, in a message, the root's absolute path (as a native path or as
/// a `file:` URI) with `<raiz>`, with `/` as the separator in the rest of the
/// path. Messages like "Target of URI doesn't exist: 'file:///…/a.dart'" or
/// "'C (where C is defined in …)'" depend on the machine where the oracle
/// ran; normalized on both sides, the record holds on any machine.
///
/// ```
/// use dartforge_parity::oracle::without_root;
/// use std::path::Path;
/// let m = without_root("Target of URI doesn't exist: 'file:///g/x/a.dart'.", Path::new("/g/x"));
/// assert_eq!(m, "Target of URI doesn't exist: '<raiz>/a.dart'.");
/// ```
pub fn without_root(msg: &str, root: &Path) -> String {
    let slash = crate::analysis::key(root).to_string_lossy().replace('\\', "/");
    let slash = slash.trim_end_matches('/');
    if slash.is_empty() {
        return msg.to_string();
    }
    let backslash = slash.replace('/', "\\");
    let uri = format!("file://{}{slash}", if slash.starts_with('/') { "" } else { "/" });
    let mut out = msg.to_string();
    for form in [uri.as_str(), slash, backslash.as_str()] {
        let mut s = String::with_capacity(out.len());
        let mut rest = out.as_str();
        while let Some(i) = rest.find(form) {
            s.push_str(&rest[..i]);
            s.push_str("<raiz>");
            rest = &rest[i + form.len()..];
            // The rest of the path runs up to a quote, parenthesis or space.
            let end = rest.find(['\'', ')', ' ', '"']).unwrap_or(rest.len());
            s.push_str(&rest[..end].replace('\\', "/"));
            rest = &rest[end..];
        }
        s.push_str(rest);
        out = s;
    }
    out
}

/// Relative path with `/`, ignoring case in the root (Windows).
pub fn relative(p: &Path, root: &Path) -> Option<String> {
    let p = crate::analysis::key(p).to_string_lossy().replace('\\', "/");
    let r = crate::analysis::key(root).to_string_lossy().replace('\\', "/");
    let r = r.trim_end_matches('/');
    if p.len() > r.len() && p[..r.len()].eq_ignore_ascii_case(r) && p.as_bytes()[r.len()] == b'/' {
        Some(p[r.len() + 1..].to_string())
    } else {
        None
    }
}

/// The record's metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    pub sdk: OracleSdk,
    /// Hash (FNV-1a 64, hexadecimal) of the group's `.dart` sources.
    pub hash: String,
    #[serde(rename = "arquivos")]
    pub files: usize,
    #[serde(rename = "diagnosticos")]
    pub diagnostics: usize,
    /// Files removed from the group because they crash the oracle's analysis
    /// server (there is nothing to compare).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[serde(rename = "excluidos")]
    pub excluded: Vec<String>,
}

/// FNV-1a 64: stable across Rust versions (`DefaultHasher` is not).
pub fn fnv(bytes: &[u8], mut h: u64) -> u64 {
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub const FNV_START: u64 = 0xcbf29ce484222325;

/// Runs `dart analyze --format=json <targets>...` with the given SDK, in the
/// package's directory. `cache` goes to `--cache` (off the C: drive). Returns
/// the diagnostics.
pub fn run(sdk: OracleSdk, package: &Path, targets: &[PathBuf], cache: &Path) -> Result<Vec<DiagJson>, String> {
    let dart = sdk.root().join("bin").join(if cfg!(windows) { "dart.exe" } else { "dart" });
    if !dart.exists() {
        return Err(format!("SDK {} not found at {}", sdk.version(), dart.display()));
    }
    let output = Command::new(&dart)
        .arg("analyze")
        .arg("--format=json")
        .arg("--cache")
        .arg(cache)
        .args(targets.iter().map(|a| std::path::absolute(a).unwrap_or_else(|_| a.clone())))
        .current_dir(package)
        .env("DART_DISABLE_ANALYTICS", "1")
        .output()
        .map_err(|e| format!("{}: {e}", dart.display()))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let code = output.status.code().unwrap_or(-1);
    if !(0..=3).contains(&code) {
        return Err(format!(
            "dart analyze exited with {code}: {}{}",
            text.chars().take(2000).collect::<String>(),
            String::from_utf8_lossy(&output.stderr).chars().take(2000).collect::<String>()
        ));
    }
    // The output may be preceded by warnings; the JSON is the line starting with `{`.
    let json = text.lines().find(|l| l.starts_with('{')).unwrap_or("");
    if json.is_empty() {
        // No JSON: the analysis server crashed (e.g. 3.6.2's `FormatException`
        // with a digit separator in `1.234_456e0`).
        return Err(format!("no JSON (code {code}): {}", text.lines().take(6).collect::<Vec<_>>().join(" | ")));
    }
    let r: Report = serde_json::from_str(json).map_err(|e| format!("oracle JSON: {e}"))?;
    Ok(r.diagnostics)
}

/// Writes the record (sorted) and the metadata.
pub fn write(dir: &Path, mut regs: Vec<Record>, meta: &Meta) -> std::io::Result<()> {
    regs.sort();
    let mut s = String::new();
    for r in &regs {
        s.push_str(&serde_json::to_string(r).expect("JSON"));
        s.push('\n');
    }
    std::fs::write(dir.join("oraculo.jsonl"), s)?;
    std::fs::write(dir.join("oraculo.json"), serde_json::to_string_pretty(meta).expect("JSON") + "\n")
}

/// Reads a group's record and metadata.
pub fn read(dir: &Path) -> Result<(Vec<Record>, Meta), String> {
    let meta: Meta = serde_json::from_str(
        &std::fs::read_to_string(dir.join("oraculo.json")).map_err(|e| format!("{}: {e}", dir.display()))?,
    )
    .map_err(|e| e.to_string())?;
    let text = std::fs::read_to_string(dir.join("oraculo.jsonl")).map_err(|e| e.to_string())?;
    let mut regs = Vec::new();
    for l in text.lines().filter(|l| !l.is_empty()) {
        regs.push(serde_json::from_str(l).map_err(|e| format!("{l}: {e}"))?);
    }
    Ok((regs, meta))
}

/// Features the 3.6.2 SDK does not know: in a 3.6 library it answers with a
/// syntax cascade, and analyzer 3.13.4 with `experiment_not_enabled` (and
/// keeps analyzing). Each one's `enabledIn` is later than 3.6.2
/// (`experimental_features.yaml`); 3.6.2 already flags the other disabled
/// features (for example `null-aware-elements`) the same way. The list is the
/// parser's (`Feature::UNKNOWN_IN_3_6`), which uses it to decide each
/// unit's reference.
pub fn features_unknown_to_3_6_2() -> impl Iterator<Item = &'static str> {
    dartforge_frontend::features::Feature::UNKNOWN_IN_3_6.into_iter().map(|f| f.name())
}

/// The tool's current version is 3.13 (VERSOES-LINGUAGEM.md, D2): in a 3.6
/// library, the reference behaviour is that of analyzer 3.13.4 on the same
/// library, not that of an SDK that does not know the feature. A file with
/// syntax of these features has its oracle recorded by 3.13.4, with the
/// group's package unchanged (language 3.6).
pub fn uses_new_syntax(regs_313: &[&Record]) -> bool {
    regs_313.iter().any(|r| {
        r.code == "experiment_not_enabled"
            && features_unknown_to_3_6_2().any(|f| r.problem_message.contains(&format!("'{f}'")))
    })
}

/// The files of a 3.6 group that **our** parser marks with the 3.13.4
/// reference (`Parsed::referencia`), as paths relative to the group: the list
/// that must match [`read_new_syntax`]'s, produced by the 3.13.4 parser
/// (docs/ANALYZER-ESPECIFICACAO.md, T2, step 9 and the done criterion). The
/// version is the group package's, 3.6: files with a `// @dart` marker above
/// it live in the `*-3.13` groups, and a marker below it enables none of the
/// features that decide the reference.
pub fn marker_3_13(dir: &Path, sources: &[std::path::PathBuf]) -> std::collections::BTreeSet<String> {
    let features = dartforge_frontend::features::LibraryFeatures::floor();
    let mut marked = std::collections::BTreeSet::new();
    for f in sources {
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        let mut names = dartforge_intern::Interner::new();
        let parsed = dartforge_frontend::parser::parse_with(&text, &mut names, features);
        if parsed.reference == dartforge_diagnostics::AnalyzerReference::V3_13
            && let Some(r) = relative(f, dir)
        {
            marked.insert(r);
        }
    }
    marked
}

/// `corpus/diagnosticos/sintaxe-nova.json`: per group, the files whose oracle
/// is 3.13.4 (see [`uses_new_syntax`]).
pub fn read_new_syntax(root: &Path) -> std::collections::BTreeMap<String, Vec<String>> {
    std::fs::read_to_string(root.join("sintaxe-nova.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Writes the list of [`read_new_syntax`].
pub fn write_new_syntax(root: &Path, list: &std::collections::BTreeMap<String, Vec<String>>) -> std::io::Result<()> {
    std::fs::write(root.join("sintaxe-nova.json"), serde_json::to_string_pretty(list).expect("JSON") + "\n")
}
