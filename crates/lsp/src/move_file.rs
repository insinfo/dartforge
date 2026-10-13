//! `workspace/willRenameFiles` (docs/LSP-ESPECIFICACAO.md §8.5): o
//! `MoveFileRefactoringImpl.multi` do 3.6.2
//! (`AS:src/services/refactoring/legacy/move_file.dart`). Renomear ou mover
//! arquivos (ou pastas, resolvidas nos arquivos de dentro) só reescreve as
//! URIs de diretivas:
//!
//! * as que chegam ao arquivo movido (`import`, `export`, `part` e, para uma
//!   biblioteca, o `part of` relativo das partes dela), de qualquer arquivo
//!   da raiz de análise;
//! * as relativas que saem dele, quando a pasta muda.
//!
//! A URI nova mantém `package:` quando o alvo continua num `lib/` de pacote;
//! senão é o caminho relativo de onde o arquivo de origem vai ficar até onde
//! o alvo vai ficar, com `/`. As aspas do literal ficam. Uma condição fatal
//! (renomear a raiz, arquivo fora de uma raiz ou de mais de uma, arquivo que
//! não existe) não muda nada.
//!
//! Escrito sem compilar nem executar (2026-10-05).

use crate::DocumentStore;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast;
use std::path::{Component, Path, PathBuf};

/// Uma referência a atualizar: o literal da URI, o arquivo em que está, o
/// arquivo a que aponta e o lexema com as aspas.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Reference {
    span: (usize, usize),
    origin: PathBuf,
    target: PathBuf,
    lexeme: String,
}

/// O caminho normalizado (`.` e `..` resolvidos), sem tocar no disco.
fn normalize(p: &Path) -> PathBuf {
    let mut output = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                output.pop();
            }
            other => output.push(other.as_os_str()),
        }
    }
    output
}

/// A chave de comparação de caminhos (a do carregamento).
fn key(p: &Path) -> PathBuf {
    dartforge_elements::gerado::chave(&normalize(p))
}

/// `_isRelativeUri`: nem URI absoluta (com esquema) nem caminho absoluto.
fn relative_uri(uri: &str) -> bool {
    let scheme = uri.find(':').is_some_and(|i| i > 1 && uri[..i].chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')));
    !scheme && !Path::new(uri).is_absolute() && !uri.starts_with('/') && !uri.starts_with('\\')
}

/// A URI de cada diretiva com literal simples (sem interpolação nem
/// adjacência): o literal, o valor e se é `part of`.
fn directive_uris(unit: &ast::CompilationUnit) -> Vec<(&ast::StringLit, String, bool)> {
    let mut v = Vec::new();
    for d in &unit.directives {
        let (lit, part_of) = match &d.kind {
            ast::DirectiveKind::Import { uri, .. } | ast::DirectiveKind::Export { uri, .. } | ast::DirectiveKind::Part { uri } => (Some(uri), false),
            ast::DirectiveKind::PartOf { uri, .. } => (uri.as_ref(), true),
            _ => (None, false),
        };
        let Some(lit) = lit else { continue };
        if lit.parts.len() != 1 {
            continue;
        }
        let Some(value) = lit.constant_value() else { continue };
        v.push((lit, String::from_utf8_lossy(value.as_bytes()).into_owned(), part_of));
    }
    v
}

/// O caminho de uma URI de diretiva escrita em `origin`: relativa, ou
/// `package:` pelos pacotes da raiz.
fn uri_path(uri: &str, origin: &Path, packages: &[(String, PathBuf)]) -> Option<PathBuf> {
    if let Some(rest) = uri.strip_prefix("package:") {
        let (name, rel) = rest.split_once('/')?;
        let (_, lib) = packages.iter().find(|(n, _)| n == name)?;
        return Some(normalize(&rel.split('/').fold(lib.clone(), |p, x| p.join(x))));
    }
    if !relative_uri(uri) {
        return None;
    }
    let folder = origin.parent()?;
    Some(normalize(&uri.split('/').fold(folder.to_path_buf(), |p, x| p.join(x))))
}

/// `pathToUri`: `package:` quando o arquivo está no `lib/` de um pacote.
fn package_uri(target: &Path, packages: &[(String, PathBuf)]) -> Option<String> {
    let target = normalize(target);
    for (name, lib) in packages {
        if let Ok(rel) = target.strip_prefix(normalize(lib)) {
            let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
            return Some(format!("package:{name}/{}", parts.join("/")));
        }
    }
    None
}

/// O caminho relativo de `folder` até `target`, com `/` (o `pathContext.relative`
/// e o `posix.joinAll` do Dart).
fn relative(target: &Path, folder: &Path) -> String {
    let (normal_target, normal_folder) = (normalize(target), normalize(folder));
    let a: Vec<Component> = normal_target.components().collect();
    let b: Vec<Component> = normal_folder.components().collect();
    // O `pathContext` do Windows compara sem caixa.
    let equal = |x: &Component, y: &Component| {
        if cfg!(windows) {
            x.as_os_str().to_string_lossy().to_lowercase() == y.as_os_str().to_string_lossy().to_lowercase()
        } else {
            x == y
        }
    };
    let common = a.iter().zip(b.iter()).take_while(|(x, y)| equal(x, y)).count();
    let mut parts: Vec<String> = vec!["..".to_string(); b.len() - common];
    parts.extend(a[common..].iter().map(|c| c.as_os_str().to_string_lossy().into_owned()));
    parts.join("/")
}

/// As aspas do lexema (`analyzeQuote`): o começo (com o `r`) e o fim.
fn quotes(lexeme: &str) -> (usize, usize) {
    let raw = usize::from(lexeme.starts_with('r'));
    let rest = &lexeme[raw..];
    let n = if rest.starts_with("'''") || rest.starts_with("\"\"\"") { 3 } else { 1 };
    (raw + n, n)
}

/// `_resolveMapping`: uma pasta vira os arquivos de dentro, recursivamente.
fn resolve(old: &Path, new: &Path, output: &mut Vec<(PathBuf, PathBuf)>) {
    if old.is_dir() {
        let Ok(r) = std::fs::read_dir(old) else { return };
        let mut children: Vec<PathBuf> = r.flatten().map(|e| e.path()).collect();
        children.sort();
        for f in children {
            let Some(name) = f.file_name() else { continue };
            resolve(&f, &new.join(name), output);
        }
    } else {
        output.push((old.to_path_buf(), new.to_path_buf()));
    }
}

/// As edições do `willRenameFiles` para `map` (antigo → novo), nas raízes
/// de análise `roots`. `None`: condição fatal (o Dart responde `null`).
pub(crate) fn edits(roots: &[PathBuf], documents: &DocumentStore, map: &[(PathBuf, PathBuf)]) -> Option<Vec<crate::Edit>> {
    // `checkFinalConditions`.
    let mut common_root: Option<PathBuf> = None;
    for (old, _) in map {
        if roots.iter().any(|r| key(r) == key(old)) {
            return None;
        }
        let contains: Vec<&PathBuf> = roots.iter().filter(|r| key(old).starts_with(key(r))).collect();
        if contains.len() != 1 || !old.exists() {
            return None;
        }
        match &common_root {
            Some(r) if key(r) != key(contains[0]) => return None,
            _ => common_root = Some(contains[0].clone()),
        }
    }
    let root = common_root?;
    let packages = crate::project::packages_with_name(&root);
    let mut resolved: Vec<(PathBuf, PathBuf)> = Vec::new();
    for (old, new) in map {
        resolve(old, new, &mut resolved);
    }
    let new_of = |p: &Path| resolved.iter().find(|(a, _)| key(a) == key(p)).map(|(_, n)| n.clone());
    // Os textos: o aberto vale mais que o disco.
    let text_of = |p: &Path| -> Option<String> {
        if let Ok(u) = url::Url::from_file_path(p)
            && let Some(t) = documents.get(u.as_str())
        {
            return Some(t.to_string());
        }
        std::fs::read_to_string(p).ok()
    };
    // Os arquivos da raiz de análise e os abertos sob ela.
    let mut files = crate::project::project_files(&root);
    for u in documents.uris() {
        if let Some(p) = url::Url::parse(u).ok().and_then(|x| x.to_file_path().ok())
            && key(&p).starts_with(key(&root))
            && p.extension().is_some_and(|e| e == "dart")
            && !files.iter().any(|a| key(a) == key(&p))
        {
            files.push(p);
        }
    }
    let moved: Vec<PathBuf> = resolved.iter().filter(|(a, _)| a.extension().is_some_and(|e| e == "dart")).map(|(a, _)| key(a)).collect();
    let mut references: std::collections::BTreeSet<Reference> = std::collections::BTreeSet::new();
    for file in &files {
        let Some(text) = text_of(file) else { continue };
        let mut names = dartforge_intern::Interner::new();
        let p = dartforge_frontend::parser::parse_com(&text, &mut names, dartforge_frontend::LibraryFeatures::default());
        let moved_ = moved.contains(&key(file));
        let folder_changes = moved_
            && new_of(file).is_some_and(|n| n.parent().map(key) != file.parent().map(key));
        for (lit, uri, part_of) in directive_uris(&p.unit) {
            let Some(target) = uri_path(&uri, file, &packages) else { continue };
            let lexeme = text.get(lit.span.start..lit.span.end).unwrap_or_default().to_string();
            // O `part of` que chega só é achado relativo (a busca de
            // referências da unidade não o inclui; o passo das partes da
            // biblioteca só lê os relativos).
            let entry = moved.contains(&key(&target)) && (!part_of || relative_uri(&uri));
            // Que sai de um arquivo cuja pasta muda: só as relativas.
            let exiting = folder_changes && relative_uri(&uri);
            if entry || exiting {
                references.insert(Reference { span: (lit.span.start, lit.span.end), origin: file.clone(), target, lexeme });
            }
        }
    }
    let mut output = Vec::new();
    for r in references {
        let new_origin = new_of(&r.origin).unwrap_or_else(|| r.origin.clone());
        let new_target = new_of(&r.target).unwrap_or_else(|| r.target.clone());
        let (start, end) = quotes(&r.lexeme);
        if r.lexeme.len() < start + end {
            continue;
        }
        let current = &r.lexeme[start..r.lexeme.len() - end];
        let new_ = match current.starts_with("package:").then(|| package_uri(&new_target, &packages)).flatten() {
            Some(u) => u,
            None => relative(&new_target, new_origin.parent().unwrap_or(Path::new(""))),
        };
        if new_ == current {
            continue;
        }
        let Ok(uri) = url::Url::from_file_path(&r.origin) else { continue };
        output.push(crate::Edit {
            uri: uri.to_string(),
            span: Span { start: r.span.0, end: r.span.1 },
            text: format!("{}{new_}{}", &r.lexeme[..start], &r.lexeme[r.lexeme.len() - end..]),
        });
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_quotes() {
        assert!(relative_uri("a/b.dart"));
        assert!(relative_uri("../b.dart"));
        assert!(!relative_uri("package:x/y.dart"));
        assert!(!relative_uri("dart:core"));
        assert_eq!(relative(Path::new("/p/lib/src/b.dart"), Path::new("/p/lib")), "src/b.dart");
        assert_eq!(relative(Path::new("/p/lib/b.dart"), Path::new("/p/lib/src")), "../b.dart");
        assert_eq!(quotes("'a.dart'"), (1, 1));
        assert_eq!(quotes("r\"a.dart\""), (2, 1));
        assert_eq!(quotes("'''a.dart'''"), (3, 3));
        let packages = vec![("x".to_string(), PathBuf::from("/p/lib"))];
        assert_eq!(package_uri(Path::new("/p/lib/src/c.dart"), &packages).as_deref(), Some("package:x/src/c.dart"));
        assert_eq!(uri_path("package:x/a.dart", Path::new("/q/z.dart"), &packages), Some(PathBuf::from("/p/lib/a.dart")));
    }
}
