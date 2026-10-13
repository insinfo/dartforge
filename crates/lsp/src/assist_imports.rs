//! Import directive assists, ported from the `analysis_server` 3.6.2
//! producers over the analyzer-shaped tree.
//!
//! | Title | Kind | Producer |
//! |---|---|---|
//! | `Add explicit 'show' combinator` | `refactor.add.showCombinator` | `ImportAddShow` |
//! | `Convert to 'package:' import` | `refactor.convert.relativeToPackageImport` | `ConvertToPackageImport` |
//! | `Convert to a relative import` | `refactor.convert.packageToRelativeImport` | `ConvertToRelativeImport` |
//! | `Convert to use a URI` | `refactor.convert.partOfToPartUri` | `ConvertPartOfToUri` |

use crate::actions::CodeAction;
use crate::refactorings::Context;
use crate::Edit;
use dartforge_diagnostics::Span;
use dartforge_elements::model::Element;
use dartforge_frontend::ast::DirectiveKind;
use dartforge_types::Resolved;

impl Context<'_> {
    /// `ImportAddShow` (import_add_show.dart): the import that encloses the
    /// selection node, without combinators, gets a `show` with the names the
    /// unit uses from it (the `_ReferenceFinder`, in text order), before the `;`.
    pub(crate) fn import_add_show(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let node = self.tree.localizar(start, end)?;
        let directive = self.with_parents(node).find(|&k| self.kind(k) == "ImportDirective")?;
        let span = self.tree.span(directive);
        let prog = self.p.program();
        let unit = prog.unit(self.unit);
        let index = unit.unit.directives.iter().position(|d| d.span.start <= span.start && span.start < d.span.end)?;
        let DirectiveKind::Import { combinators, .. } = &unit.unit.directives[index].kind else { return None };
        if !combinators.is_empty() {
            return None;
        }
        let imp = prog.library(unit.library).imports.iter().find(|i| i.unit == self.unit && i.directive == index)?;
        let mut names: Vec<String> = match imp.prefix {
            None => dartforge_analysis::importacoes::nomes_usados_do_import(prog, self.unit, index, &self.p.query.names)?
                .into_iter()
                .map(|s| self.p.name(s).to_string())
                .collect(),
            Some(prefix) => self.used_names_by_prefix(imp.library, prefix),
        };
        // `_addImplicitExtensionName`: the extension of the resolved member
        // (method, property, operator, index, `call`), if the import's namespace
        // has it under its name; a prefixed import indexes by `p.x`.
        if imp.prefix.is_none() {
            let exported_entry = &prog.library(imp.library).exported;
            for r in self.bodies.resolved.iter().flatten() {
                let Resolved::ExtensionMember { extension, .. } = r else { continue };
                let Some(name) = prog.extension(*extension).name else { continue };
                if exported_entry.get(&name).and_then(|b| b.getter) == Some(Element::Extension(*extension)) {
                    names.push(self.p.name(name).to_string());
                }
            }
        }
        // `SplayTreeSet<String>`: no repetition, in the order of Dart's `compareTo`
        // (UTF-16 units).
        names.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        names.dedup();
        if names.is_empty() {
            return None;
        }
        // `importDirective.end - 1`: before the `;`.
        let at = span.end.checked_sub(1)?;
        Some(CodeAction {
            title: "Add explicit 'show' combinator".into(),
            kind: "refactor.add.showCombinator".into(),
            edits: vec![Edit { uri: uri.to_string(), span: Span { start: at, end: at }, text: format!(" show {}", names.join(", ")) }],
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }

    /// The names the unit uses from a prefixed import: the Dart 3.6.2 server
    /// (checked against the oracle) compares each name with the import's
    /// namespace without the prefix (`namespace[name] == element`), so the
    /// `p.x` whose element is what the library exports under `x` are included.
    fn used_names_by_prefix(&self, library: dartforge_elements::model::LibraryId, prefix: dartforge_intern::SymbolId) -> Vec<String> {
        use dartforge_frontend::ast::{ExprKind, TypeKind};
        let prog = self.p.program();
        let exported_entry = &prog.library(library).exported;
        let of_import = |s: dartforge_intern::SymbolId, e: Element| exported_entry.get(&s).is_some_and(|b| b.getter == Some(e) || b.setter == Some(e));
        let mut names = Vec::new();
        for (i, x) in self.ast.exprs.iter().enumerate() {
            let n = match &x.kind {
                ExprKind::Identifier(n) | ExprKind::Property { name: n, .. } => *n,
                _ => continue,
            };
            if let Some(Resolved::Element(e)) = self.bodies.get_resolved(dartforge_frontend::ast::ExprId(i as u32))
                && of_import(n.sym, *e)
            {
                names.push(self.p.name(n.sym).to_string());
            }
        }
        let lib = prog.unit(self.unit).library;
        for ty in &self.ast.types {
            let TypeKind::Named { name, .. } = &ty.kind else { continue };
            let n = match &name[..] {
                [pr, n] | [pr, n, _] if pr.sym == prefix => *n,
                [n] | [n, _] => *n,
                _ => continue,
            };
            let binding = match &name[..] {
                [pr, _] | [pr, _, _] if pr.sym == prefix => prog.lookup_prefixed(lib, prefix, n.sym),
                _ => prog.lookup(lib, n.sym),
            };
            if let Some(g) = binding.and_then(|b| b.getter)
                && of_import(n.sym, g)
            {
                names.push(self.p.name(n.sym).to_string());
            }
        }
        // The class name in a constructor (the `returnType`, which resolves to the
        // class).
        for m in &self.ast.members {
            if let dartforge_frontend::ast::MemberKind::Constructor(k) = &m.kind
                && let Some(g) = prog.lookup(lib, k.class_name.sym).and_then(|b| b.getter)
                && of_import(k.class_name.sym, g)
            {
                names.push(self.p.name(k.class_name.sym).to_string());
            }
        }
        // The `[x]`/`[p.x]` references of the documentation (the visitor descends
        // into `CommentReference`).
        for c in self.p.unit_comments(self.unit).iter() {
            for r in &c.references {
                for i in 0..r.len() {
                    let Some(s) = self.p.query.names.lookup(&r[i].1) else { continue };
                    if let Some((crate::project::Target::Top(e), _)) = self.p.resolve_doc_reference(self.unit, c.span, &r[..=i])
                        && of_import(s, e)
                    {
                        names.push(r[i].1.clone());
                    }
                }
            }
        }
        names
    }

    /// The import directive of the selection node (the URI literal walks up to
    /// it) and its index in the unit.
    fn selection_import(&self, start: usize, end: usize) -> Option<(usize, usize)> {
        let mut node = self.tree.localizar(start, end)?;
        if matches!(self.kind(node), "SimpleStringLiteral" | "StringInterpolation" | "AdjacentStrings") {
            node = self.parent(node)?;
        }
        if self.kind(node) != "ImportDirective" {
            return None;
        }
        let span = self.tree.span(node);
        let unit = self.p.program().unit(self.unit);
        let index = unit.unit.directives.iter().position(|d| d.span.start <= span.start && span.start < d.span.end)?;
        Some((node, index))
    }

    /// The URI literal of a directive (its text and range).
    fn directive_uri(&self, index: usize) -> Option<(String, Span)> {
        let unit = self.p.program().unit(self.unit);
        let DirectiveKind::Import { uri, .. } = &unit.unit.directives.get(index)?.kind else { return None };
        Some((dartforge_elements::load::string_lit_value(uri)?, uri.span))
    }

    /// `ConvertToPackageImport` (convert_to_package_import.dart): an import
    /// written without `package:` of a library whose URI is `package:` starts
    /// citing it by that URI.
    pub(crate) fn convert_to_package_import(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let (_, index) = self.selection_import(start, end)?;
        let prog = self.p.program();
        let imp = prog.library(prog.unit(self.unit).library).imports.iter().find(|i| i.unit == self.unit && i.directive == index)?;
        let canonical = &prog.library(imp.library).uri;
        if !canonical.starts_with("package:") {
            return None;
        }
        let (write, range) = self.directive_uri(index)?;
        if write.starts_with("package:") {
            return None;
        }
        Some(CodeAction {
            title: "Convert to 'package:' import".into(),
            kind: "refactor.convert.relativeToPackageImport".into(),
            edits: vec![Edit { uri: uri.to_string(), span: range, text: format!("'{canonical}'") }],
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }

    /// `ConvertToRelativeImport` (convert_to_relative_import.dart): a `package:`
    /// import of the same package as the unit becomes a relative path.
    pub(crate) fn convert_to_relative_import(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let (_, index) = self.selection_import(start, end)?;
        let prog = self.p.program();
        // `libraryImport?.uri is DirectiveUriWithSource`: the import has a target.
        prog.library(prog.unit(self.unit).library).imports.iter().find(|i| i.unit == self.unit && i.directive == index)?;
        let source_uri = prog.unit(self.unit).uri.strip_prefix("package:")?.to_string();
        let (write, range) = self.directive_uri(index)?;
        let imported = write.strip_prefix("package:")?;
        let source_segments: Vec<&str> = source_uri.split('/').collect();
        let import_segments: Vec<&str> = imported.split('/').collect();
        if source_segments.is_empty() || import_segments.is_empty() || source_segments[0] != import_segments[0] {
            return None;
        }
        // `path.posix.relative(importUri.path, from: dirname(sourceUri.path))`.
        let from: Vec<&str> = source_segments[..source_segments.len() - 1].to_vec();
        let common = from.iter().zip(&import_segments).take_while(|(a, b)| a == b).count();
        let mut parts: Vec<&str> = vec![".."; from.len() - common];
        parts.extend(&import_segments[common..]);
        let relative = if parts.is_empty() { ".".to_string() } else { parts.join("/") };
        Some(CodeAction {
            title: "Convert to a relative import".into(),
            kind: "refactor.convert.packageToRelativeImport".into(),
            edits: vec![Edit { uri: uri.to_string(), span: Span { start: range.start + 1, end: range.end - 1 }, text: relative }],
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }

    /// `ConvertPartOfToUri` (convert_part_of_to_uri.dart): `part of name;`
    /// starts citing the library by the relative path of its file.
    pub(crate) fn convert_part_of_to_uri(&self, uri: &str, start: usize, end: usize) -> Option<CodeAction> {
        let node = self.tree.localizar(start, end)?;
        let directive = self.with_parents(node).find(|&k| self.kind(k) == "PartOfDirective")?;
        let name = self.children(directive).iter().copied().find(|&k| self.kind(k) == "LibraryIdentifier" || self.kind(k) == "SimpleIdentifier")?;
        let prog = self.p.program();
        let library = prog.library(prog.unit(self.unit).library);
        let defining = library.units.first().map(|&u| prog.unit(u))?;
        let path_lib = defining.path.as_ref()?;
        let path_part = prog.unit(self.unit).path.as_ref()?;
        let dir = path_part.parent()?;
        // `pathContext.relative` and `toUri` (forward slashes, escaped spaces).
        let a: Vec<String> = dir.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        let b: Vec<String> = path_lib.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        let common = a.iter().zip(&b).take_while(|(x, y)| x.eq_ignore_ascii_case(y)).count();
        let mut parts: Vec<String> = vec!["..".to_string(); a.len() - common];
        parts.extend(b[common..].iter().cloned());
        let relative = parts.join("/").replace(' ', "%20");
        Some(CodeAction {
            title: "Convert to use a URI".into(),
            kind: "refactor.convert.partOfToPartUri".into(),
            edits: vec![Edit { uri: uri.to_string(), span: self.tree.span(name), text: format!("'{relative}'") }],
            diagnostic: None,
            create_file: None, linked: Default::default(),
        })
    }
}
