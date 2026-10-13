//! `textDocument/documentLink` (docs/LSP-ESPECIFICACAO.md §8.4): the 3.6.2
//! `DartDocumentLinkVisitor`
//! (`analyzer_plugin/lib/utilities/navigation/document_links.dart`). In each
//! documentation comment of the tree (the `Comment` of a declaration, a
//! member, an enum constant, a directive, a parameter), each `{@tool …}` …
//! `{@end-tool}` block whose content cites `** See code in
//! examples/api/…dart` links the path snippet to the file under the ancestor
//! folder that has `examples/api` (the Flutter convention). Without that
//! folder, no link. 3.6.2 does not link directive URIs.
//!
//! Written without compiling or running (2026-10-05).

use dartforge_diagnostics::Span;
use dartforge_frontend::ast;
use dartforge_frontend::comentarios::Comentarios;
use std::path::{Path, PathBuf};

/// The links of `text` (the file `path`): the snippet and the target file.
pub(crate) fn links(text: &str, path: &Path) -> Vec<(Span, PathBuf)> {
    let mut names = dartforge_intern::Interner::new();
    let p = dartforge_frontend::parser::parse_com(text, &mut names, dartforge_frontend::LibraryFeatures::default());
    let comments = Comentarios::de(text);
    let mut docs: Vec<Span> = Vec::new();
    let mut doc_of = |start: usize| {
        if let Some(s) = documentation_comment(&comments, text, start)
            && !docs.contains(&s)
        {
            docs.push(s);
        }
    };
    let start_of = |span: Span, metadata: &[ast::Annotation]| metadata.first().map_or(span.start, |m| m.span.start.min(span.start));
    for d in &p.unit.directives {
        doc_of(start_of(d.span, &d.metadata));
    }
    let a = &p.ast;
    for d in &a.decls {
        doc_of(start_of(d.span, &d.metadata));
        if let ast::DeclKind::Enum(e) = &d.kind {
            for k in &e.constants {
                doc_of(start_of(k.span, &k.metadata));
            }
        }
    }
    for m in &a.members {
        doc_of(start_of(m.span, &m.metadata));
        if let ast::MemberKind::Constructor(k) = &m.kind {
            for x in k.parameters.iter() {
                doc_of(start_of(x.span, &x.metadata));
            }
        }
    }
    for f in &a.functions {
        for x in f.parameters.iter().flatten() {
            doc_of(start_of(x.span, &x.metadata));
        }
    }
    docs.sort_by_key(|s| s.start);
    let mut folder: Option<Option<PathBuf>> = None;
    let mut output = Vec::new();
    for d in docs {
        let snippet = &text[d.start..d.end];
        let mut search = 0;
        while let Some(k) = snippet[search..].find("{@tool") {
            let opens = search + k;
            // `{@tool}` or `{@tool arguments}` (not `{@toolbar}`).
            if !matches!(snippet.as_bytes().get(opens + 6), Some(b' ' | b'}' | b'\t' | b'\r' | b'\n')) {
                search = opens + 6;
                continue;
            }
            let Some(f) = snippet[opens..].find('}') else { break };
            let content = opens + f + 1;
            // Without `{@end-tool}`, the block does not close and is left out.
            let Some(c) = snippet[content..].find("{@end-tool}") else {
                search = content;
                continue;
            };
            let end_ = content + c;
            search = end_ + "{@end-tool}".len();
            let value = &snippet[content..end_];
            if value.is_empty() {
                continue;
            }
            const SEE: &str = "** See code in ";
            let Some(i) = value.find("** See code in examples/api/") else { continue };
            let root = folder.get_or_insert_with(|| examples_folder(path));
            // Without the folder, the visitor's `return`: nothing more from this comment.
            let Some(root) = root.clone() else { break };
            let start_ = i + SEE.len();
            // The first `.dart` of the content (the visitor's `indexOf`). Before the
            // start, Dart's `substring` would throw: the link is left out.
            let Some(end) = value.find(".dart").map(|e| e + 5) else { continue };
            if end < start_ {
                continue;
            }
            let target = value[start_..end].split('/').fold(root, |p, part| p.join(part));
            let start = d.start + content + start_;
            output.push((Span { start: start, end: start + (end - start_) }, target));
        }
    }
    output
}

/// The documentation `Comment` of the node that starts at `start`
/// (`findDartDoc`): the last `/**`, or from the first to the last line of the
/// last `///` sequence.
fn documentation_comment(c: &Comentarios, text: &str, start: usize) -> Option<Span> {
    let doc = c.dart_doc(text, start)?;
    if !text[doc.start..doc.end].starts_with("///") {
        return Some(doc);
    }
    let end = c
        .antes_de(text, start)
        .into_iter()
        .filter(|s| s.start >= doc.start && text[s.start..s.end].starts_with("///"))
        .map(|s| s.end)
        .max()
        .unwrap_or(doc.end);
    Some(Span { start: doc.start, end: end })
}

/// The folder, among the file's own and its ancestors, that has `examples/api`.
fn examples_folder(path: &Path) -> Option<PathBuf> {
    let mut current = path.parent();
    while let Some(p) = current {
        if p.join("examples").join("api").is_dir() {
            return Some(p.to_path_buf());
        }
        current = p.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_example_code_of_tool_block() {
        let root = std::env::temp_dir().join(format!("dartforge-links-{}", std::process::id()));
        let _ = std::fs::create_dir_all(root.join("examples/api"));
        let _ = std::fs::create_dir_all(root.join("lib"));
        let text = "/// Um widget.\n///\n/// {@tool dartpad}\n/// ** See code in examples/api/lib/a/b.0.dart **\n/// {@end-tool}\nclass A {}\n\n/// {@tool snippet}\n/// ** See code in examples/api/lib/c.dart **\nclass B {}\n";
        let ls = links(text, &root.join("lib/a.dart"));
        assert_eq!(ls.len(), 1, "o bloco sem {{@end-tool}} fica de fora: {ls:?}");
        let (s, target) = &ls[0];
        assert_eq!(&text[s.start..s.end], "examples/api/lib/a/b.0.dart");
        assert_eq!(target, &root.join("examples").join("api").join("lib").join("a").join("b.0.dart"));
        // Without the `examples/api` folder, no link.
        let other = std::env::temp_dir().join(format!("dartforge-links-sem-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&other);
        assert!(links(text, &other.join("a.dart")).is_empty());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&other);
    }
}
