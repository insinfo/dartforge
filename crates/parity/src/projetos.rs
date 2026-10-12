//! The owner's real projects (local machine): the oracle over the whole
//! project (measured: 0 diagnostics in all three), our false positives per
//! code, and the **deterministic mutations** — an error planted in a file of
//! the project, with the oracle re-recorded over the mutant.
//!
//! Nothing is written to the owner's project: each project is copied to
//! `target/scratch-paridade/projetos/<name>` (only the `.dart` files, the
//! `pubspec.yaml`, the `analysis_options.yaml` and a `package_config.json` with
//! absolute roots), and the copy is what gets mutated. The record lives in
//! `corpus/diagnosticos/projetos.json` and `corpus/diagnosticos/mutacoes.jsonl`;
//! a mutation whose original file changed (hash) is flagged as stale.

use crate::oraculo::{FNV_START, Record, OracleSdk, fnv};
use dartforge_frontend::ast::{DirectiveKind, ExprKind, TypeKind, UnaryOp};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A real project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub nome: String,
    pub caminho: PathBuf,
    pub sdk: OracleSdk,
    /// The oracle's diagnostics over the whole project (recorded).
    #[serde(default)]
    pub oraculo: Option<usize>,
}

/// The plan's three projects.
pub fn default_projects() -> Vec<Project> {
    let p = |name: &str, c: &str| Project { nome: name.into(), caminho: c.into(), sdk: OracleSdk::V362, oraculo: None };
    vec![
        p("new_sali-core", "C:/MyDartProjects/new_sali/core"),
        p("new_sali-frontend", "C:/MyDartProjects/new_sali/frontend"),
        p("limitless_ui", "E:/references/limitless_ui"),
    ]
}

/// Mutation kinds.
pub const KINDS: &[&str] = &["nome", "import", "tipo", "bang", "await"];

/// A mutation and what the oracle said about the mutated file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mutation {
    pub projeto: String,
    /// Relative to the project's root, with `/`.
    pub arquivo: String,
    pub tipo: String,
    /// Bytes replaced in the original.
    pub inicio: usize,
    pub fim: usize,
    pub inserir: String,
    /// FNV-1a of the original file.
    pub hash_original: String,
    pub oraculo: Vec<Record>,
}

impl Mutation {
    pub fn apply(&self, original: &str) -> String {
        format!("{}{}{}", &original[..self.inicio], self.inserir, &original[self.fim..])
    }
}

pub fn text_hash(t: &str) -> String {
    format!("{:016x}", fnv(t.as_bytes(), FNV_START))
}

/// The project's `.dart` files that `dart analyze` would see (without `exclude`).
pub fn arquivos(root: &Path, opcoes: &crate::filtros::Opcoes) -> Vec<PathBuf> {
    crate::corpus::arquivos_dart(root)
        .into_iter()
        .filter(|a| crate::oraculo::relativo(a, root).is_some_and(|r| !opcoes.excluido(&r)))
        .collect()
}

/// Copies the project to `dest` (sources, pubspec, options) and writes a
/// `package_config.json` with absolute roots (the package's own, `../`).
/// Returns the path of the copy's `package_config.json`.
pub fn copy(p: &Project, dest: &Path) -> Result<PathBuf, String> {
    let _ = std::fs::remove_dir_all(dest);
    for a in crate::corpus::arquivos_dart(&p.caminho) {
        let r = a.strip_prefix(&p.caminho).map_err(|_| "prefix")?;
        let d = dest.join(r);
        std::fs::create_dir_all(d.parent().expect("parent")).map_err(|e| e.to_string())?;
        std::fs::copy(&a, &d).map_err(|e| e.to_string())?;
    }
    // build_runner outputs: the analyzer sees them (ngdart's `.template.dart`).
    let generated = p.caminho.join(".dart_tool/build/generated");
    for a in crate::corpus::arquivos_dart_todos(&generated) {
        let r = a.strip_prefix(&p.caminho).map_err(|_| "prefix")?;
        let d = dest.join(r);
        std::fs::create_dir_all(d.parent().expect("parent")).map_err(|e| e.to_string())?;
        std::fs::copy(&a, &d).map_err(|e| e.to_string())?;
    }
    for f in ["pubspec.yaml", "analysis_options.yaml"] {
        if p.caminho.join(f).is_file() {
            std::fs::copy(p.caminho.join(f), dest.join(f)).map_err(|e| e.to_string())?;
        }
    }
    let orig = p.caminho.join(".dart_tool/package_config.json");
    let text = std::fs::read_to_string(&orig).map_err(|e| format!("{}: {e}", orig.display()))?;
    let mut cfg: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let base = orig.parent().expect("parent");
    if let Some(pkgs) = cfg.get_mut("packages").and_then(|v| v.as_array_mut()) {
        for pkg in pkgs {
            let Some(root) = pkg.get("rootUri").and_then(|v| v.as_str()).map(str::to_string) else { continue };
            let abs = if root.starts_with("file:") {
                root.clone()
            } else {
                let target = crate::analise::chave(&base.join(&root));
                if target == crate::analise::chave(&p.caminho) {
                    "../".to_string()
                } else {
                    format!("file:///{}", target.to_string_lossy().replace('\\', "/"))
                }
            };
            pkg["rootUri"] = serde_json::Value::String(abs);
        }
    }
    let d = dest.join(".dart_tool/package_config.json");
    std::fs::create_dir_all(d.parent().expect("parent")).map_err(|e| e.to_string())?;
    std::fs::write(&d, serde_json::to_string_pretty(&cfg).expect("JSON")).map_err(|e| e.to_string())?;
    Ok(d)
}

/// Groups `files` by owning package (the nearest directory, going up to
/// `root`, that has a `pubspec.yaml`), in path order.
pub fn por_pacote(root: &Path, files: &[PathBuf]) -> Vec<(PathBuf, Vec<PathBuf>)> {
    let mut groups: std::collections::BTreeMap<PathBuf, Vec<PathBuf>> = Default::default();
    for a in files {
        let mut d = a.parent();
        let mut owner = root.to_path_buf();
        while let Some(x) = d {
            if x.join("pubspec.yaml").is_file() {
                owner = x.to_path_buf();
                break;
            }
            if crate::analise::chave(x) == crate::analise::chave(root) {
                break;
            }
            d = x.parent();
        }
        groups.entry(owner).or_default().push(a.clone());
    }
    groups.into_iter().collect()
}

/// The sites of a mutation kind in a file: `(start, end, insert)`.
pub fn sites(text: &str, kind: &str) -> Vec<(usize, usize, String)> {
    let mut interner = dartforge_intern::Interner::new();
    let parsed = dartforge_frontend::parser::parse(text, &mut interner);
    if !parsed.diagnostics.is_empty() {
        return Vec::new();
    }
    let ast = &parsed.ast;
    let source = |s: dartforge_diagnostics::Span| text.get(s.start..s.end).unwrap_or("");
    let mut v = Vec::new();
    match kind {
        "nome" => {
            for e in &ast.exprs {
                if let ExprKind::Identifier(n) = &e.kind {
                    let t = source(n.span);
                    if t.len() > 1 && t.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
                        v.push((n.span.start, n.span.end, format!("{t}NaoExisteDf")));
                    }
                }
            }
        }
        "import" => {
            for d in &parsed.unit.directives {
                if let DirectiveKind::Import { .. } = d.kind {
                    v.push((d.span.start, d.span.end, String::new()));
                }
            }
        }
        "tipo" => {
            for t in &ast.types {
                if let TypeKind::Named { name, args } = &t.kind {
                    if name.len() == 1 && args.is_empty() && !t.nullable {
                        match source(t.span) {
                            "String" => v.push((t.span.start, t.span.end, "int".into())),
                            "int" => v.push((t.span.start, t.span.end, "String".into())),
                            _ => {}
                        }
                    }
                }
            }
        }
        "bang" => {
            for e in &ast.exprs {
                if let ExprKind::Unary { op: UnaryOp::NullAssert, .. } = e.kind {
                    if e.span.end > 0 && text.as_bytes().get(e.span.end - 1) == Some(&b'!') {
                        v.push((e.span.end - 1, e.span.end, String::new()));
                    }
                }
            }
        }
        "await" => {
            for e in &ast.exprs {
                if let ExprKind::Await(op) = e.kind {
                    let target = ast.exprs[op.0 as usize].span.start;
                    if source(e.span).starts_with("await") && target > e.span.start {
                        v.push((e.span.start, target, String::new()));
                    }
                }
            }
        }
        _ => {}
    }
    v.sort();
    v
}

/// Chooses up to `per_kind` mutations per kind, in different files,
/// deterministically (file order; the site by the path's hash).
pub fn choose(p: &Project, copy_root: &Path, files: &[PathBuf], per_kind: usize) -> Vec<Mutation> {
    let mut out = Vec::new();
    // Only `lib/`: it is the owner's code that the editor shows.
    let libs: Vec<&PathBuf> = files
        .iter()
        .filter(|a| crate::oraculo::relativo(a, copy_root).is_some_and(|r| r.starts_with("lib/") && !r.ends_with(".g.dart")))
        .collect();
    if libs.is_empty() {
        return out;
    }
    for (k, kind) in KINDS.iter().enumerate() {
        let mut n = 0;
        // Prime step over the list: files spread across the project.
        let step = 7919 % libs.len().max(1) + 1;
        let mut idx = (k * 131) % libs.len();
        for _ in 0..libs.len() {
            if n >= per_kind {
                break;
            }
            let a = libs[idx];
            idx = (idx + step) % libs.len();
            let Ok(text) = std::fs::read_to_string(a) else { continue };
            let s = sites(&text, kind);
            if s.is_empty() {
                continue;
            }
            let rel = crate::oraculo::relativo(a, copy_root).expect("relative");
            if out.iter().any(|m: &Mutation| m.arquivo == rel) {
                continue;
            }
            let h = fnv(rel.as_bytes(), FNV_START) as usize;
            let (start, end, insert) = s[h % s.len()].clone();
            out.push(Mutation {
                projeto: p.nome.clone(),
                arquivo: rel,
                tipo: kind.to_string(),
                inicio: start,
                fim: end,
                inserir: insert,
                hash_original: text_hash(&text),
                oraculo: Vec::new(),
            });
            n += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sites_of_each_kind() {
        let t = "import 'dart:async';\nFuture<int> f(String? ss) async { String xx = ss!; return await gg(xx); }\n";
        assert_eq!(sites(t, "import").len(), 1);
        assert!(!sites(t, "nome").is_empty());
        assert_eq!(sites(t, "bang").len(), 1);
        assert_eq!(sites(t, "await").len(), 1);
        let ty = sites(t, "tipo");
        assert_eq!(ty.len(), 2, "{ty:?}"); // the `int` of `Future<int>` and the local `String`
        let m = Mutation {
            projeto: "p".into(),
            arquivo: "a".into(),
            tipo: "await".into(),
            inicio: sites(t, "await")[0].0,
            fim: sites(t, "await")[0].1,
            inserir: String::new(),
            hash_original: String::new(),
            oraculo: vec![],
        };
        assert!(m.apply(t).contains("return gg(xx)"));
    }
}
