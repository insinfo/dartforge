//! `workspace/willRenameFiles` (docs/LSP-ESPECIFICACAO.md §8.5): the 3.6.2
//! `MoveFileRefactoringImpl.multi`
//! (`AS:src/services/refactoring/legacy/move_file.dart`). Renaming or moving
//! files (or folders, resolved into the files inside them) only rewrites
//! directive URIs:
//!
//! * those that reach the moved file (`import`, `export`, `part` and, for a
//!   library, the relative `part of` of its parts), from any file of the
//!   analysis root;
//! * the relative ones that leave it, when the folder changes.
//!
//! The new URI keeps `package:` when the target stays in a package's `lib/`;
//! otherwise it is the relative path from where the source file will be to
//! where the target will be, with `/`. The literal's quotes stay. A fatal
//! condition (renaming the root, a file outside a root or in more than one,
//! a file that does not exist) changes nothing.
//!
//! Written without compiling or running (2026-10-05).

use crate::DocumentStore;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast;
use std::path::{Component, Path, PathBuf};

/// A reference to update: the URI literal, the file it is in, the file it
/// points to and the lexeme with its quotes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Reference {
    span: (usize, usize),
    origin: PathBuf,
    target: PathBuf,
    lexeme: String,
}

/// The normalized path (`.` and `..` resolved), without touching the disk.
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

/// The path comparison key (the loader's).
fn key(p: &Path) -> PathBuf {
    dartforge_elements::gerado::chave(&normalize(p))
}

/// `_isRelativeUri`: neither an absolute URI (with a scheme) nor an absolute
/// path.
fn relative_uri(uri: &str) -> bool {
    let scheme = uri.find(':').is_some_and(|i| i > 1 && uri[..i].chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')));
    !scheme && !Path::new(uri).is_absolute() && !uri.starts_with('/') && !uri.starts_with('\\')
}

/// The URI of each directive with a simple literal (no interpolation nor
/// adjacency): the literal, the value and whether it is `part of`.
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

/// The path of a directive URI written in `origin`: relative, or `package:`
/// through the root's packages.
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

/// `pathToUri`: `package:` when the file is in a package's `lib/`.
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

/// The relative path from `folder` to `target`, with `/` (Dart's
/// `pathContext.relative` and `posix.joinAll`).
fn relative(target: &Path, folder: &Path) -> String {
    let (normal_target, normal_folder) = (normalize(target), normalize(folder));
    let a: Vec<Component> = normal_target.components().collect();
    let b: Vec<Component> = normal_folder.components().collect();
    // The Windows `pathContext` compares case-insensitively.
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

/// The lexeme's quotes (`analyzeQuote`): the start (with the `r`) and the end.
fn quotes(lexeme: &str) -> (usize, usize) {
    let raw = usize::from(lexeme.starts_with('r'));
    let rest = &lexeme[raw..];
    let n = if rest.starts_with("'''") || rest.starts_with("\"\"\"") { 3 } else { 1 };
    (raw + n, n)
}

/// `_resolveMapping`: a folder becomes the files inside it, recursively.
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

/// The `willRenameFiles` edits for `map` (old → new), in the analysis roots
/// `roots`. `None`: fatal condition (Dart answers `null`).
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
    // The texts: the open one wins over the disk.
    let text_of = |p: &Path| -> Option<String> {
        if let Ok(u) = url::Url::from_file_path(p)
            && let Some(t) = documents.get(u.as_str())
        {
            return Some(t.to_string());
        }
        std::fs::read_to_string(p).ok()
    };
    // The files of the analysis root and the open ones under it.
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
        let p = dartforge_frontend::parser::parse_with(&text, &mut names, dartforge_frontend::LibraryFeatures::default());
        let moved_ = moved.contains(&key(file));
        let folder_changes = moved_
            && new_of(file).is_some_and(|n| n.parent().map(key) != file.parent().map(key));
        for (lit, uri, part_of) in directive_uris(&p.unit) {
            let Some(target) = uri_path(&uri, file, &packages) else { continue };
            let lexeme = text.get(lit.span.start..lit.span.end).unwrap_or_default().to_string();
            // An incoming `part of` is only found when relative (the unit's reference
            // search does not include it; the library's parts step only reads relative
            // ones).
            let entry = moved.contains(&key(&target)) && (!part_of || relative_uri(&uri));
            // Outgoing from a file whose folder changes: only the relative ones.
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
