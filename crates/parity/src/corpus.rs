//! The parity corpus, `corpus/diagnosticos/<group>/`.
//!
//! Each group is a Dart package (versioned `pubspec.yaml`; the
//! `.dart_tool/package_config.json` is written by [`prepare`] and stays out of
//! git) with a `grupo.json` that says which SDK is the oracle and which
//! support packages (`corpus/diagnosticos/pacotes/<name>`) it sees. The
//! expectation is **always** what the oracle recorded (`oraculo.jsonl`); the
//! annotations in the sources the cases came from (`// [analyzer] ...`,
//! `[diag.x]`) are from SDK `main` and only served as seeds.
//!
//! [`generate`] builds the groups from `references/` (local machine):
//! * `linguagem`: `references/dart-sdk/tests/language`, the files with
//!   `// [analyzer]` marks and the closure of their relative imports/parts;
//! * `analyzer`: `references/dart-sdk/pkg/analyzer/test/src/diagnostics`, one
//!   file per snippet of `assertErrorsInCode`/`assertNoErrorsInCode`/
//!   `resolveTestCodeWithDiagnostics`, except the tests that set up context
//!   (other files, options, packages);
//! * `*-3.13`: the files that declare `// @dart = 3.7` or later, whose oracle
//!   is 3.13.4.

use crate::oraculo::{FNV_START, OracleSdk, fnv};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// `grupo.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub sdk: OracleSdk,
    /// Support packages in `corpus/diagnosticos/pacotes/`.
    #[serde(default)]
    pub pacotes: Vec<String>,
    /// Where the cases came from (free text, for the report).
    #[serde(default)]
    pub origem: String,
}

/// The corpus root in the repository.
pub fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/diagnosticos")
}

/// The corpus groups, in name order.
pub fn groups(root: &Path) -> Vec<(String, Group)> {
    let mut v = Vec::new();
    let Ok(ents) = std::fs::read_dir(root) else { return v };
    for e in ents.flatten() {
        let g = e.path().join("grupo.json");
        if let Ok(t) = std::fs::read_to_string(&g) {
            if let Ok(group) = serde_json::from_str::<Group>(&t) {
                v.push((e.file_name().to_string_lossy().into_owned(), group));
            }
        }
    }
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

/// The `.dart` files of a directory, recursively, sorted, like
/// `ContextRootImpl.analyzedFiles` (INFRA §4.5): folders and files starting
/// with `.` are left out (`.dart_tool`, `.git`, `.x.dart`); `build` is in.
pub fn arquivos_dart(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(ents) = std::fs::read_dir(&d) else { continue };
        for e in ents.flatten() {
            let p = e.path();
            let name = e.file_name();
            let name = name.to_string_lossy();
            if p.is_dir() {
                if !name.starts_with('.') {
                    stack.push(p);
                }
            } else if name.ends_with(".dart") && !name.starts_with('.') {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// The `.dart` files of a directory, recursively, skipping nothing
/// (generated outputs).
pub fn all_dart_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(ents) = std::fs::read_dir(&d) else { continue };
        for e in ents.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "dart") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Hash of the group's sources (relative path + contents, in order).
pub fn sources_hash(dir: &Path) -> String {
    let mut h = FNV_START;
    for a in arquivos_dart(dir) {
        let rel = crate::oraculo::relativo(&a, dir).unwrap_or_default();
        h = fnv(rel.as_bytes(), h);
        h = fnv(&[0], h);
        h = fnv(&std::fs::read(&a).unwrap_or_default(), h);
    }
    format!("{h:016x}")
}

/// Writes the group's `.dart_tool/package_config.json` (relative URIs).
/// Returns its path.
pub fn prepare(dir: &Path, name: &str, g: &Group) -> std::io::Result<PathBuf> {
    let package = package_name(name);
    let mut pkgs = vec![serde_json::json!({
        "name": package, "rootUri": "../", "packageUri": "lib/", "languageVersion": g.sdk.language()
    })];
    for p in &g.pacotes {
        pkgs.push(serde_json::json!({
            "name": p, "rootUri": format!("../../pacotes/{p}"), "packageUri": "lib/", "languageVersion": "3.4"
        }));
    }
    let cfg = serde_json::json!({ "configVersion": 2, "packages": pkgs, "generator": "dartforge-parity" });
    let dest = dir.join(".dart_tool").join("package_config.json");
    std::fs::create_dir_all(dest.parent().expect("parent"))?;
    std::fs::write(&dest, serde_json::to_string_pretty(&cfg).expect("JSON") + "\n")?;
    Ok(dest)
}

/// A valid Dart package name for the group.
pub fn package_name(group: &str) -> String {
    format!("corpus_{}", group.replace(['-', '.'], "_"))
}

fn write_group(dir: &Path, name: &str, g: &Group) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(
        dir.join("pubspec.yaml"),
        format!("name: {}\npublish_to: none\nenvironment:\n  sdk: {}\n", package_name(name), g.sdk.constraint()),
    )?;
    std::fs::write(dir.join("grupo.json"), serde_json::to_string_pretty(g).expect("JSON") + "\n")
}

/// `// @dart = 3.7` or later at the start of the file (before code)?
pub fn declares_3_7_or_later(text: &str) -> bool {
    for l in text.lines().take(40) {
        let t = l.trim();
        if let Some(r) = t.strip_prefix("//") {
            let r: String = r.chars().filter(|c| !c.is_whitespace()).collect();
            if let Some(v) = r.strip_prefix("@dart=") {
                let mut it = v.split('.');
                let maj: u32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
                let min: u32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
                return (maj, min) >= (3, 7);
            }
        }
    }
    false
}

/// Relative `import`/`export`/`part` URIs in a file (raw text).
fn relative_uris(text: &str) -> Vec<String> {
    let mut v = Vec::new();
    for l in text.lines() {
        let t = l.trim_start();
        let rest = ["import ", "export ", "part "].iter().find_map(|k| t.strip_prefix(k));
        let Some(rest) = rest else { continue };
        let rest = rest.trim_start();
        let Some(q) = rest.chars().next().filter(|c| *c == '\'' || *c == '"') else { continue };
        let Some(end) = rest[1..].find(q) else { continue };
        let uri = &rest[1..1 + end];
        if !uri.contains(':') && uri.ends_with(".dart") {
            v.push(uri.to_string());
        }
    }
    v
}

fn normal(p: &Path) -> PathBuf {
    crate::analise::chave(p)
}

/// Builds the groups from `references` (replacing their `.dart` files; the
/// oracle must be re-recorded afterwards).
pub fn generate(references: &Path, dest: &Path, pub_cache: &Path) -> Result<String, String> {
    let mut rel = String::new();
    // Support packages.
    copy_tree(&references.join("dart-sdk/pkg/expect/lib"), &dest.join("pacotes/expect/lib"))?;
    copy_tree(&pub_cache.join("meta-1.16.0/lib"), &dest.join("pacotes/meta/lib"))?;
    let packages = vec!["expect".to_string(), "meta".to_string()];

    // --- language
    let base = references.join("dart-sdk/tests/language");
    let mut seeds = BTreeSet::new();
    for a in arquivos_dart(&base) {
        let t = std::fs::read_to_string(&a).unwrap_or_default();
        if t.contains("// [analyzer]") {
            seeds.insert(normal(&a));
        }
    }
    let n_seeds = seeds.len();
    let mut closure: BTreeSet<PathBuf> = BTreeSet::new();
    let mut stack: Vec<PathBuf> = seeds.iter().cloned().collect();
    while let Some(a) = stack.pop() {
        if !closure.insert(a.clone()) {
            continue;
        }
        let t = std::fs::read_to_string(&a).unwrap_or_default();
        for u in relative_uris(&t) {
            let target = normal(&a.parent().expect("parent").join(&u));
            if target.is_file() && target.starts_with(normal(&base)) && !closure.contains(&target) {
                stack.push(target);
            }
        }
    }
    let mut by_group: BTreeMap<&str, usize> = BTreeMap::new();
    for g in ["linguagem", "linguagem-3.13"] {
        let _ = std::fs::remove_dir_all(dest.join(g));
    }
    for a in &closure {
        let t = std::fs::read(a).map_err(|e| e.to_string())?;
        let g = if declares_3_7_or_later(&String::from_utf8_lossy(&t)) { "linguagem-3.13" } else { "linguagem" };
        let r = a.strip_prefix(normal(&base)).map_err(|_| "outside the base")?;
        let d = dest.join(g).join(r);
        std::fs::create_dir_all(d.parent().expect("parent")).map_err(|e| e.to_string())?;
        std::fs::write(&d, t).map_err(|e| e.to_string())?;
        *by_group.entry(g).or_default() += 1;
    }
    for (g, sdk) in [("linguagem", OracleSdk::V362), ("linguagem-3.13", OracleSdk::V3134)] {
        if by_group.contains_key(g) {
            write_group(
                &dest.join(g),
                g,
                &Group {
                    sdk,
                    pacotes: packages.clone(),
                    origem: "references/dart-sdk/tests/language (// [analyzer] marks + import closure)".into(),
                },
            )
            .map_err(|e| e.to_string())?;
        }
    }
    rel.push_str(&format!(
        "linguagem: {n_seeds} files with // [analyzer], {} with the closure; {:?}\n",
        closure.len(),
        by_group
    ));

    // --- analyzer
    for g in ["analyzer", "analyzer-3.13"] {
        let _ = std::fs::remove_dir_all(dest.join(g));
    }
    let base = references.join("dart-sdk/pkg/analyzer/test/src/diagnostics");
    let mut n_files = 0;
    let mut n_snippets: BTreeMap<&str, usize> = BTreeMap::new();
    let mut n_skipped = 0;
    for a in arquivos_dart(&base) {
        if a.parent() != Some(base.as_path()) {
            continue;
        }
        let t = std::fs::read_to_string(&a).map_err(|e| e.to_string())?;
        let stem = a.file_stem().expect("name").to_string_lossy().trim_end_matches("_test").to_string();
        let (snippets, skipped) = extract_snippets(&t);
        n_skipped += skipped;
        if snippets.is_empty() {
            continue;
        }
        n_files += 1;
        let mut used = BTreeSet::new();
        for (class, method, code) in snippets {
            let mut name = format!("{}__{}", short_class(&class), method.trim_start_matches("test_"));
            let mut k = 2;
            while !used.insert(name.clone()) {
                name = format!("{}__{}_{k}", short_class(&class), method.trim_start_matches("test_"));
                k += 1;
            }
            let g = if declares_3_7_or_later(&code) { "analyzer-3.13" } else { "analyzer" };
            let d = dest.join(g).join(&stem).join(format!("{}.dart", short_name(&name)));
            std::fs::create_dir_all(d.parent().expect("parent")).map_err(|e| e.to_string())?;
            std::fs::write(&d, code).map_err(|e| e.to_string())?;
            *n_snippets.entry(g).or_default() += 1;
        }
    }
    for (g, sdk) in [("analyzer", OracleSdk::V362), ("analyzer-3.13", OracleSdk::V3134)] {
        if n_snippets.contains_key(g) {
            write_group(
                &dest.join(g),
                g,
                &Group {
                    sdk,
                    pacotes: packages.clone(),
                    origem: "references/dart-sdk/pkg/analyzer/test/src/diagnostics (one file per snippet)".into(),
                },
            )
            .map_err(|e| e.to_string())?;
        }
    }
    rel.push_str(&format!(
        "analyzer: {n_files} test files, snippets {n_snippets:?}, {n_skipped} tests skipped (they set up context)\n"
    ));
    Ok(rel)
}

/// A file name short enough for Windows' 260-character limit (the CI checkout
/// and the worktrees have long prefixes): up to 48 characters; above that,
/// 39 + `_` + 8 hex digits of the name's hash.
pub fn short_name(stem: &str) -> String {
    if stem.len() <= 48 {
        return stem.to_string();
    }
    let mut cut = 39;
    while !stem.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}_{:08x}", &stem[..cut], fnv(stem.as_bytes(), FNV_START) as u32)
}

/// Renames the long-named files of an already recorded group and rewrites
/// the oracle record (the offsets do not change; only the path). Returns how
/// many.
pub fn shorten(dir: &Path) -> Result<usize, String> {
    let (mut regs, mut meta) = crate::oraculo::read(dir)?;
    let mut map = std::collections::BTreeMap::new();
    // The language tests import each other by name: nothing is renamed there
    // (the record is only rewritten with the sources' hash).
    let rename = !dir.file_name().is_some_and(|n| n.to_string_lossy().starts_with("linguagem"));
    for a in arquivos_dart(dir).into_iter().filter(|_| rename) {
        let stem = a.file_stem().expect("name").to_string_lossy().into_owned();
        let new = short_name(&stem);
        if new != stem {
            let dest = a.with_file_name(format!("{new}.dart"));
            std::fs::rename(&a, &dest).map_err(|e| e.to_string())?;
            let from = crate::oraculo::relativo(&a, dir).expect("relative");
            let to = crate::oraculo::relativo(&dest, dir).expect("relative");
            map.insert(from, to);
        }
    }
    for r in regs.iter_mut() {
        if let Some(n) = map.get(&r.arquivo) {
            r.arquivo = n.clone();
        }
    }
    meta.hash = sources_hash(dir);
    crate::oraculo::write(dir, regs, &meta).map_err(|e| e.to_string())?;
    Ok(map.len())
}

/// The test class's short name (`NonBoolConditionTest` → `NonBoolCondition`).
fn short_class(class: &str) -> String {
    class.trim_end_matches("Test").to_string()
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    let _ = std::fs::remove_dir_all(to);
    for a in arquivos_dart(from) {
        let r = a.strip_prefix(from).map_err(|_| "prefix")?;
        let d = to.join(r);
        std::fs::create_dir_all(d.parent().expect("parent")).map_err(|e| e.to_string())?;
        std::fs::copy(&a, &d).map_err(|e| format!("{}: {e}", a.display()))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Extracting the snippets of the analyzer tests.

#[derive(Debug, Clone, PartialEq)]
enum T {
    Id(String),
    /// String literal: (raw, triple, contents).
    Str(bool, bool, String),
    P(char),
}

/// Tokens with positions; comments dropped; whole strings (escapes and
/// interpolation are not interpreted).
fn tokens(src: &str) -> Vec<(usize, T)> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
        } else if b[i..].starts_with(b"//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if b[i..].starts_with(b"/*") {
            let mut depth = 0;
            while i < b.len() {
                if b[i..].starts_with(b"/*") {
                    depth += 1;
                    i += 2;
                } else if b[i..].starts_with(b"*/") {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else if c == b'\'' || c == b'"' || (c == b'r' && matches!(b.get(i + 1), Some(b'\'') | Some(b'"'))) {
            let start = i;
            let raw = c == b'r';
            if raw {
                i += 1;
            }
            let q = b[i];
            let triple = b[i..].starts_with(&[q, q, q]);
            let n = if triple { 3 } else { 1 };
            i += n;
            let contents_start = i;
            // `${...}` interpolation with strings inside: counting braces is enough.
            let mut braces = 0i32;
            loop {
                if i >= b.len() {
                    break;
                }
                if braces == 0 && ((triple && b[i..].starts_with(&[q, q, q])) || (!triple && b[i] == q)) {
                    break;
                }
                if !raw && b[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if !raw && b[i..].starts_with(b"${") {
                    braces += 1;
                    i += 2;
                    continue;
                }
                if braces > 0 && b[i] == b'}' {
                    braces -= 1;
                }
                i += 1;
            }
            let contents = src[contents_start..i.min(b.len())].to_string();
            i = (i + n).min(b.len());
            out.push((start, T::Str(raw, triple, contents)));
        } else if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
                i += 1;
            }
            out.push((start, T::Id(src[start..i].to_string())));
        } else {
            let ch = src[i..].chars().next().expect("char");
            out.push((i, T::P(ch)));
            i += ch.len_utf8();
        }
    }
    out
}

/// Calls that take the test's code as their first argument.
const CALLS: &[&str] =
    &["assertErrorsInCode", "assertNoErrorsInCode", "resolveTestCodeWithDiagnostics", "assertErrorsInCode2"];

/// Calls that set up context beyond the test file: the snippet alone does not
/// reproduce the test.
const CONTEXT: &[&str] = &[
    "newFile",
    "writeTestPackageConfig",
    "writeTestPackageConfigWithMeta",
    "createAnalysisOptionsFile",
    "writeTestPackageAnalysisOptionsFile",
    "addTestFile",
    "newPackage",
    "writePackageConfig",
    "newAnalysisOptionsYamlFile",
    "testFilePath",
    "newPubspecYamlFile",
    "createMockSdk",
    "addFile",
];

/// `(class, method, code)` of each snippet, and the number of skipped tests.
pub fn extract_snippets(src: &str) -> (Vec<(String, String, String)>, usize) {
    let t = tokens(src);
    let mut out = Vec::new();
    let mut skipped = 0;
    let mut i = 0;
    let mut depth = 0i32;
    let mut class: Option<(String, i32, bool)> = None; // name, body depth, setUp with context
    while i < t.len() {
        match &t[i].1 {
            T::P('{') => depth += 1,
            T::P('}') => {
                depth -= 1;
                if class.as_ref().is_some_and(|c| depth < c.1) {
                    class = None;
                }
            }
            T::Id(k) if k == "class" && depth == 0 => {
                if let Some((_, T::Id(name))) = t.get(i + 1) {
                    // The class body: the next `{`. A setUp with context skips the class.
                    let mut j = i + 2;
                    while j < t.len() && t[j].1 != T::P('{') {
                        j += 1;
                    }
                    let end = block_end(&t, j);
                    let ctx = has_context_setup(&t[j..end]);
                    class = Some((name.clone(), depth + 1, ctx));
                    i = j;
                    continue;
                }
            }
            T::Id(m) if m.starts_with("test_") && class.as_ref().is_some_and(|c| depth == c.1) => {
                // test_x() async { ... }
                let mut j = i + 1;
                while j < t.len() && t[j].1 != T::P('{') && t[j].1 != T::P(';') {
                    j += 1;
                }
                if j >= t.len() || t[j].1 != T::P('{') {
                    i += 1;
                    continue;
                }
                let end = block_end(&t, j);
                let body = &t[j..end];
                let (class_name, _, class_ctx) = class.clone().expect("class");
                let uses_context = class_ctx || body.iter().any(|(_, x)| matches!(x, T::Id(n) if CONTEXT.contains(&n.as_str())));
                let mut found = Vec::new();
                for w in body.windows(3) {
                    if let (T::Id(n), T::P('('), T::Str(raw, true, code)) = (&w[0].1, &w[1].1, &w[2].1) {
                        if CALLS.contains(&n.as_str()) {
                            if !*raw && (code.contains('$') || code.contains('\\')) {
                                continue;
                            }
                            // In a Dart multi-line string, spaces and the line break right
                            // after the opening quotes are not part of the value.
                            let trimmed = code.trim_start_matches([' ', '\t']);
                            let value = trimmed
                                .strip_prefix("\r\n")
                                .or_else(|| trimmed.strip_prefix('\n'))
                                .unwrap_or(code);
                            found.push(value.to_string());
                        }
                    }
                }
                if uses_context {
                    skipped += 1;
                } else {
                    for c in found {
                        out.push((class_name.clone(), m.clone(), c));
                    }
                }
                // the loop continues from the `{`, counting depth as usual
                i = j;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    (out, skipped)
}

fn block_end(t: &[(usize, T)], open: usize) -> usize {
    let mut p = 0;
    let mut j = open;
    while j < t.len() {
        match t[j].1 {
            T::P('{') => p += 1,
            T::P('}') => {
                p -= 1;
                if p == 0 {
                    return j + 1;
                }
            }
            _ => {}
        }
        j += 1;
    }
    t.len()
}

/// Does the class body have a `setUp` that sets up context?
fn has_context_setup(body: &[(usize, T)]) -> bool {
    let mut i = 0;
    while i < body.len() {
        if let T::Id(n) = &body[i].1 {
            if n == "setUp" {
                let mut j = i;
                while j < body.len() && body[j].1 != T::P('{') && body[j].1 != T::P(';') {
                    j += 1;
                }
                if j < body.len() && body[j].1 == T::P('{') {
                    let end = block_end(body, j);
                    if body[j..end].iter().any(|(_, x)| matches!(x, T::Id(n) if CONTEXT.contains(&n.as_str()))) {
                        return true;
                    }
                }
            }
        }
        i += 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_snippets_and_skips_context() {
        let src = r#"
class ATest extends PubPackageResolutionTest {
  test_um() async {
    await assertErrorsInCode(r'''
f() { return 3 ? 2 : 1; }
''', [error(X, 1, 2)]);
  }
  test_dois() async {
    newFile('$testPackageLibPath/a.dart', '');
    await assertNoErrorsInCode(r'''
import 'a.dart';
''');
  }
  test_tres() async {
    await resolveTestCodeWithDiagnostics('''
var s = '${1}';
''');
  }
}
"#;
        let (v, skipped) = extract_snippets(src);
        assert_eq!(skipped, 1);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].1, "test_um");
        assert_eq!(v[0].2, "f() { return 3 ? 2 : 1; }\n");
    }

    #[test]
    fn declared_version() {
        assert!(declares_3_7_or_later("// @dart = 3.8\nvoid main() {}"));
        assert!(!declares_3_7_or_later("// @dart=2.19\n"));
        assert!(!declares_3_7_or_later("void main() {}"));
    }
}
