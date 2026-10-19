//! Move top-level to file
//! (`AS:src/services/refactoring/move_top_level_to_file.dart`,
//! docs/LSP-ESPECIFICACAO.md §13.11.9 e): `compute`, with the
//! `ImportAnalyzer` (`AS:src/utilities/import_analyzer.dart`), the file
//! header, the group ranges and the `importLibrary`/`_addLibraryImports` of
//! `DartFileEditBuilderImpl` (`analyzer_plugin`, `change_builder_dart.dart`).
//! Written without compiling or running (2026-10-05).

use crate::refactorings::{Context, RefactoringResult};
use crate::refactorings_exec::{Change, Text, enabled_rule};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{Element, FunctionKind, LibraryId, UnitId};
use dartforge_frontend::ast::{self, DeclKind, ExprId, ExprKind};
use dartforge_frontend::token::Kind;
use dartforge_types::{MemberRef, Resolved};
use std::path::{Path, PathBuf};

type R = RefactoringResult;

/// Import combinators and URI spelling policy for one requested library.
#[derive(Default)]
struct ImportOptions<'a> {
    prefix: Option<&'a str>,
    show: Option<&'a str>,
    use_show: bool,
    absolute: bool,
    relative: bool,
}

/// The lines of a text (`LineInfo`): `\r\n`, `\r` and `\n` end a line.
struct Lines {
    starts: Vec<usize>,
}

impl Lines {
    fn from(t: &str) -> Lines {
        let b = t.as_bytes();
        let mut starts = vec![0];
        let mut i = 0;
        while i < b.len() {
            match b[i] {
                b'\r' => {
                    if i + 1 < b.len() && b[i + 1] == b'\n' {
                        i += 1;
                    }
                    starts.push(i + 1);
                }
                b'\n' => starts.push(i + 1),
                _ => {}
            }
            i += 1;
        }
        Lines { starts }
    }

    /// `getLocation(offset).lineNumber` (1-based).
    fn number(&self, offset: usize) -> usize {
        self.starts.partition_point(|&s| s <= offset)
    }

    /// `getOffsetOfLine(index)` (0-based).
    fn start(&self, index: usize) -> usize {
        self.starts.get(index).copied().unwrap_or_else(|| *self.starts.last().unwrap())
    }
}

/// A directive of the file (read from the text).
#[derive(Debug, Clone)]
struct Directive {
    kind: &'static str,
    /// The URI text (without the quotes).
    uri: String,
    prefix: String,
    /// With the documentation comment and the annotations.
    start: usize,
    start_without_meta: usize,
    end: usize,
    shown: Vec<(Span, Vec<String>)>,
    hidden: Vec<(Span, Vec<String>)>,
}

/// A unit prepared for the imports (`DartFileEditBuilderImpl`).
struct Constructor {
    uri: String,
    path: Option<PathBuf>,
    /// The text (empty for a new file).
    source: String,
    /// The unit, when it exists in the program (the lint options).
    unit: Option<UnitId>,
    directives: Vec<Directive>,
    /// The start of the first declaration.
    first_declaration: Option<usize>,
    /// `librariesToImport`, in insertion order.
    import: Vec<PendingImport>,
    header: Option<String>,
}

/// `_LibraryImport`.
#[derive(Debug, Clone)]
struct PendingImport {
    key: String,
    text: String,
    prefixes: Vec<String>,
    shown: Vec<Vec<String>>,
    hidden: Vec<Vec<String>>,
}

impl PendingImport {
    fn all_shown(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for l in &self.shown {
            for n in l {
                if !v.contains(n) {
                    v.push(n.clone());
                }
            }
        }
        v
    }

    fn all_hidden(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for l in &self.hidden {
            for n in l {
                if !v.contains(n) {
                    v.push(n.clone());
                }
            }
        }
        v
    }

    /// `ensureShown`.
    fn ensure_shown(&mut self, name: &str, use_show: bool) {
        if self.shown.is_empty() && use_show {
            self.shown.push(vec![name.to_string()]);
        } else if let Some(l) = self.shown.last_mut() {
            l.push(name.to_string());
        }
        for l in self.hidden.iter_mut() {
            l.retain(|n| n != name);
        }
        self.hidden.retain(|l| !l.is_empty());
    }
}

/// `DirectiveSortPriority` of an import.
fn priority(uri: &str) -> u8 {
    if uri.starts_with("dart:") {
        0
    } else if uri.starts_with("package:") {
        1
    } else if uri.contains("://") {
        2
    } else {
        3
    }
}

/// `compareDirectiveUri`.
fn compare_uris(a: &str, b: &str) -> std::cmp::Ordering {
    if (!a.starts_with("package:") || !b.starts_with("package:")) && !a.starts_with('/') && !b.starts_with('/') {
        return a.cmp(b);
    }
    let (Some(ia), Some(ib)) = (a.find('/'), b.find('/')) else { return a.cmp(b) };
    a[..ia].cmp(&b[..ib]).then_with(|| a[ia + 1..].cmp(&b[ib + 1..]))
}

/// The `pathToUri` of the session's converter: `package:` for a file under a
/// package's `lib/`, otherwise `file:`.
fn path_uri(path: &Path) -> String {
    let configuration = dartforge_elements::config::PackageConfig::discover(path).and_then(|p| dartforge_elements::config::PackageConfig::load(&p).ok());
    let canonical = match (path.parent().and_then(|p| p.canonicalize().ok()), path.file_name()) {
        (Some(folder), Some(name)) => dartforge_elements::config::without_verbatim(folder.join(name)),
        _ => path.to_path_buf(),
    };
    if let Some(c) = &configuration {
        for (name, folder) in &c.package_dirs {
            if let Ok(rel) = canonical.strip_prefix(folder) {
                return format!("package:{name}/{}", rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    url::Url::from_file_path(path).map(|u| u.to_string()).unwrap_or_default()
}

impl Constructor {
    fn new(uri: String, path: Option<PathBuf>, source: String, unit: Option<UnitId>, header: Option<String>) -> Constructor {
        let mut names = dartforge_intern::Interner::new();
        let analyzed = dartforge_frontend::parser::parse(&source, &mut names);
        let mut directives = Vec::new();
        for d in analyzed.unit.directives.iter() {
            let start = start_with_meta(&source, d.span.start, &d.metadata);
            let mut x = Directive {
                kind: "",
                uri: String::new(),
                prefix: String::new(),
                start,
                start_without_meta: d.span.start,
                end: d.span.end,
                shown: Vec::new(),
                hidden: Vec::new(),
            };
            match &d.kind {
                ast::DirectiveKind::Library { .. } => x.kind = "library",
                ast::DirectiveKind::Import { uri, prefix, combinators, .. } => {
                    x.kind = "import";
                    x.uri = source[uri.span.start..uri.span.end].trim_matches(|c| c == '\'' || c == '"').to_string();
                    x.prefix = prefix.map(|p| source[p.span.start..p.span.end].to_string()).unwrap_or_default();
                    for c in combinators.iter() {
                        match c {
                            ast::Combinator::Show(ns) => {
                                x.shown.push((combinator_span(&source, ns, "show"), ns.iter().map(|n| source[n.span.start..n.span.end].to_string()).collect()))
                            }
                            ast::Combinator::Hide(ns) => {
                                x.hidden.push((combinator_span(&source, ns, "hide"), ns.iter().map(|n| source[n.span.start..n.span.end].to_string()).collect()))
                            }
                        }
                    }
                }
                ast::DirectiveKind::Export { .. } => x.kind = "export",
                ast::DirectiveKind::Part { .. } => x.kind = "part",
                _ => x.kind = "outra",
            }
            directives.push(x);
        }
        let first_declaration = analyzed.unit.declarations.first().map(|&d| {
            let decl = analyzed.ast.decl(d);
            start_with_meta(&source, decl.span.start, &decl.metadata)
        });
        Constructor { uri, path, source, unit, directives, first_declaration, import: Vec::new(), header }
    }

    /// The canonical URI of a URI written in an import of this file.
    fn canonical_uri(&self, write: &str) -> String {
        if write.contains(':') {
            return write.to_string();
        }
        match self.path.as_deref().and_then(|c| c.parent()) {
            Some(folder) => path_uri(&folder.join(write)),
            None => write.to_string(),
        }
    }

    /// `_getLibraryUriText(uri, forceAbsolute, forceRelative)`.
    fn forced_uri_text(&self, cx: &Context<'_>, uri: &str, absolute: bool, relative_forced: bool) -> String {
        let folder = self.path.as_deref().and_then(|c| c.parent()).map(Path::to_path_buf);
        if let Ok(u) = url::Url::parse(uri)
            && u.scheme() == "file"
            && let (Ok(target), Some(folder)) = (u.to_file_path(), &folder)
        {
            return crate::refactorings_method::relative_path(&target, folder);
        }
        let prefer = self.unit.is_some_and(|u| enabled_rule(cx.p, u, "prefer_relative_imports"));
        if relative_forced || (prefer && !absolute) {
            let own = path_uri(self.path.as_deref().unwrap_or(Path::new("")));
            let package = |s: &str| s.strip_prefix("package:").and_then(|r| r.split('/').next()).map(str::to_string);
            if let (Some(a), Some(b)) = (package(uri), package(&own))
                && a == b
                && let Some(folder) = &folder
            {
                // `uriToPath`: through the `package_config`.
                let configuration = self
                    .path
                    .as_deref()
                    .and_then(dartforge_elements::config::PackageConfig::discover)
                    .and_then(|p| dartforge_elements::config::PackageConfig::load(&p).ok());
                if let Some(c) = configuration
                    && let Ok(target) = c.resolve_package_uri(uri)
                {
                    return crate::refactorings_method::relative_path(&target, folder);
                }
            }
        }
        uri.to_string()
    }

    /// `_importLibrary(uri, prefix, shownName, useShow)`.
    fn import_library(&mut self, cx: &Context<'_>, uri: &str, prefix: Option<&str>, show: Option<&str>, use_show: bool) {
        self.forced_import(cx, uri, ImportOptions { prefix, show, use_show, ..ImportOptions::default() });
    }

    /// `_importLibrary` with `forceAbsolute`/`forceRelative`: the `uriText`.
    fn forced_import(&mut self, cx: &Context<'_>, uri: &str, options: ImportOptions<'_>) -> String {
        let ImportOptions { prefix, show, use_show, absolute, relative } = options;
        if let Some(i) = self.import.iter_mut().find(|i| i.key == uri) {
            if let Some(p) = prefix
                && !i.prefixes.iter().any(|x| x == p)
            {
                i.prefixes.push(p.to_string());
            }
            if let Some(n) = show {
                i.ensure_shown(n, use_show);
            }
            return i.text.clone();
        }
        let text = self.forced_uri_text(cx, uri, absolute, relative);
        // The `show`/`hide` of the existing imports with the same URI and prefix.
        let (mut shown, mut hidden): (Vec<Vec<String>>, Vec<Vec<String>>) = (Vec::new(), Vec::new());
        for d in self.directives.iter().filter(|d| d.kind == "import") {
            if d.prefix != prefix.unwrap_or("") {
                continue;
            }
            if self.forced_uri_text(cx, &self.canonical_uri(&d.uri), absolute, relative) != text {
                continue;
            }
            shown.extend(d.shown.iter().map(|(_, n)| n.clone()));
            hidden.extend(d.hidden.iter().map(|(_, n)| n.clone()));
        }
        let mut i = PendingImport {
            key: uri.to_string(),
            text,
            prefixes: vec![prefix.unwrap_or("").to_string()],
            shown,
            hidden,
        };
        if let Some(n) = show {
            i.ensure_shown(n, use_show);
        }
        let text = i.text.clone();
        self.import.push(i);
        text
    }

    /// `finalize`: the imports (`_addLibraryImports`) and the header.
    fn finish(&self, cx: &Context<'_>, m: &mut Change, eol: &str) {
        if !self.import.is_empty() {
            self.add_imports(cx, m, eol);
        }
        if let Some(c) = &self.header {
            // `addInsertion(0, insertBeforeExisting: true)`.
            insert_before(m, &self.uri, 0, format!("{c}{eol}"));
        }
    }

    /// `_addLibraryImports`.
    fn add_imports(&self, cx: &Context<'_>, m: &mut Change, eol: &str) {
        let source = self.source.as_str();
        let library: Option<usize> = self.directives.iter().find(|d| d.kind == "library").map(|d| d.end);
        let imports: Vec<&Directive> = self.directives.iter().filter(|d| d.kind == "import").collect();
        let first_export: Option<usize> = self.directives.iter().find(|d| d.kind == "export").map(|d| d.start);
        let first_part: Option<usize> = self.directives.iter().find(|d| d.kind == "part").map(|d| d.start);
        let first_directive: Option<usize> = self.directives.first().map(|d| d.start);
        let first_declaration = self.first_declaration;
        let unit_end = source.len();
        let mut list = self.import.clone();
        list.sort_by(|a, b| {
            let (pa, pb) = (priority(&a.text), priority(&b.text));
            if pa == pb { compare_uris(&a.text, &b.text) } else { pa.cmp(&pb) }
        });
        let sort = self.unit.is_some_and(|u| enabled_rule(cx.p, u, "combinators_ordering"));
        // The preferred quotes, from the imports.
        let quotes = match self.unit {
            Some(u) if enabled_rule(cx.p, u, "prefer_single_quotes") => '\'',
            Some(u) if enabled_rule(cx.p, u, "prefer_double_quotes") => '"',
            _ => {
                let double = imports.iter().filter(|d| source[d.start_without_meta..d.end].contains("\"")).count();
                let simple = imports.len() - double;
                if double > simple { '"' } else { '\'' }
            }
        };
        let write = |i: &PendingImport| -> String {
            let mut s = String::new();
            let mut prefixes = i.prefixes.clone();
            prefixes.sort();
            for (k, p) in prefixes.iter().enumerate() {
                if k > 0 {
                    s.push_str(eol);
                }
                s.push_str(&format!("import {quotes}{}{quotes}", i.text));
                if !p.is_empty() {
                    s.push_str(" as ");
                    s.push_str(p);
                }
                let shown = i.all_shown();
                if !shown.is_empty() {
                    let mut v = shown.clone();
                    if sort {
                        v.sort();
                    }
                    s.push_str(&format!(" show {}", v.join(", ")));
                }
                let hidden = i.all_hidden();
                if !hidden.is_empty() {
                    let mut v = hidden.clone();
                    if sort {
                        v.sort();
                    }
                    s.push_str(&format!(" hide {}", v.join(", ")));
                }
                s.push(';');
            }
            s
        };
        if !imports.is_empty() {
            for i in &list {
                let dart = i.text.starts_with("dart:");
                let package = i.text.starts_with("package:");
                let mut inserted = false;
                let mut last: Option<usize> = None;
                let mut last_dart: Option<usize> = None;
                let mut last_package: Option<usize> = None;
                let (mut last_is_dart, mut last_is_package) = (false, false);
                for (k, e) in imports.iter().enumerate() {
                    let is_dart = e.uri.starts_with("dart:");
                    let is_package = e.uri.starts_with("package:");
                    let is_relative = !e.uri.contains(':');
                    let replaces = i.text == e.uri && i.prefixes.first().map(String::as_str).unwrap_or("") == e.prefix;
                    let before = i.text.as_str() < e.uri.as_str();
                    // `insert(prev, replace, next, trailingNewLine)`.
                    let insert = |previous: Option<usize>, next: usize, line_after: bool, m: &mut Change| {
                        if let Some(a) = previous {
                            let d = &imports[a];
                            let mut offset = d.end;
                            // Comments on the same line after the previous one.
                            let lines = Lines::from(source);
                            let line = lines.number(offset);
                            for c in dartforge_frontend::comments::Comments::scan(source).all() {
                                if c.start >= d.end && lines.number(c.start) == line && source[d.end..c.start].trim().is_empty() {
                                    offset = c.end;
                                }
                            }
                            m.add(&self.uri, Span { start: offset, end: offset }, format!("{eol}{}", write(i)));
                        } else {
                            let d = &imports[next];
                            let offset = if Some(d.start) == first_directive { d.start_without_meta } else { d.start };
                            let mut t = format!("{}{eol}", write(i));
                            if line_after {
                                t.push_str(eol);
                            }
                            m.add(&self.uri, Span { start: offset, end: offset }, t);
                        }
                    };
                    if replaces {
                        self.update_combinators(m, i, &e.shown, &e.hidden, sort);
                        inserted = true;
                        break;
                    } else if dart {
                        if !is_dart || before {
                            insert(last_dart, k, !is_dart, m);
                            inserted = true;
                            break;
                        }
                    } else if package {
                        if is_relative || before {
                            insert(last_package, k, is_relative, m);
                            inserted = true;
                            break;
                        }
                    } else if !is_dart && !is_package && before {
                        insert(None, k, false, m);
                        inserted = true;
                        break;
                    }
                    last = Some(k);
                    if is_dart {
                        last_dart = Some(k);
                    } else if is_package {
                        last_package = Some(k);
                    }
                    last_is_dart = is_dart;
                    last_is_package = is_package;
                }
                if !inserted && let Some(u) = last {
                    let mut t = String::new();
                    if package {
                        if last_is_dart {
                            t.push_str(eol);
                        }
                    } else if !dart && (last_is_dart || last_is_package) {
                        t.push_str(eol);
                    }
                    t.push_str(eol);
                    t.push_str(&write(i));
                    let end = imports[u].end;
                    m.add(&self.uri, Span { start: end, end }, t);
                }
            }
            return;
        }
        if let Some(end) = library {
            let mut t = format!("{eol}{eol}");
            for (k, i) in list.iter().enumerate() {
                t.push_str(&write(i));
                if k != list.len() - 1 {
                    t.push_str(eol);
                }
            }
            m.add(&self.uri, Span { start: end, end }, t);
            return;
        }
        if let Some(o) = first_export.or(first_part) {
            let mut t = String::new();
            for i in &list {
                t.push_str(&write(i));
                t.push_str(eol);
            }
            t.push_str(eol);
            m.add(&self.uri, Span { start: o, end: o }, t);
            return;
        }
        // Before the first declaration, or together with the other edits.
        let file_edits: Vec<usize> = m.files.iter().find(|(u, _)| *u == self.uri).map(|(_, l)| l.iter().map(|e| e.span.start).collect()).unwrap_or_default();
        let (offset, line_after) = if let Some(o) = first_declaration {
            (o, true)
        } else if let Some(&o) = file_edits.iter().min() {
            (o, true)
        } else {
            (unit_end, false)
        };
        let mut t = String::new();
        for (k, i) in list.iter().enumerate() {
            t.push_str(&write(i));
            t.push_str(eol);
            if k == list.len() - 1 && line_after {
                t.push_str(eol);
            }
        }
        insert_before(m, &self.uri, offset, t);
    }

    /// `updateHideCombinators` and `updateShowCombinators` of an existing import.
    fn update_combinators(&self, m: &mut Change, i: &PendingImport, shown: &[(Span, Vec<String>)], hidden: &[(Span, Vec<String>)], sort: bool) {
        let source = self.source.as_str();
        let all_hidden = i.all_hidden();
        let all_shown = i.all_shown();
        for (s, names) in hidden {
            let new: Vec<String> = names.iter().filter(|n| all_hidden.contains(n) && !all_shown.contains(n)).cloned().collect();
            if new.is_empty() {
                // From the end of the previous token to the end of the `hide`.
                let previous = source[..s.start].trim_end().len();
                m.add(&self.uri, Span { start: previous, end: s.end }, "");
            } else if new.len() != names.len() {
                let mut v = new.clone();
                if sort {
                    v.sort();
                }
                m.add(&self.uri, *s, format!("hide {}", v.join(", ")));
            }
        }
        if i.shown.is_empty() || shown.is_empty() {
            return;
        }
        let existing: Vec<String> = shown.iter().flat_map(|(_, n)| n.iter().cloned()).collect();
        let new: Vec<String> = all_shown.iter().filter(|n| !existing.contains(n)).cloned().collect();
        if new.is_empty() {
            return;
        }
        let sorted = existing.windows(2).all(|w| w[0] <= w[1]);
        let (last_span, last) = shown.last().unwrap();
        if sort || sorted {
            let mut v: Vec<String> = last.clone();
            for n in &new {
                if !v.contains(n) {
                    v.push(n.clone());
                }
            }
            v.sort();
            m.add(&self.uri, *last_span, format!("show {}", v.join(", ")));
        } else {
            let mut v = new.clone();
            v.sort();
            m.add(&self.uri, Span { start: last_span.end, end: last_span.end }, format!(", {}", v.join(", ")));
        }
    }
}

/// `addInsertion(offset, insertBeforeExisting: true)`: the insertion goes
/// before the others at the same offset in the text.
fn insert_before(m: &mut Change, uri: &str, offset: usize, text: String) {
    // In `SourceFileEdit` order (descending), the insertion goes after all
    // those with a greater or equal offset: in the text, before them.
    let list = match m.files.iter().position(|(u, _)| u == uri) {
        Some(i) => &mut m.files[i].1,
        None => {
            m.files.push((uri.to_string(), Vec::new()));
            &mut m.files.last_mut().unwrap().1
        }
    };
    let mut i = 0;
    while i < list.len() && list[i].span.start >= offset {
        i += 1;
    }
    list.insert(i, crate::Edit { uri: uri.to_string(), span: Span { start: offset, end: offset }, text });
}

/// The start of a directive or declaration with the documentation comment
/// and the annotations.
fn start_with_meta(source: &str, start_: usize, meta: &[ast::Annotation]) -> usize {
    let mut start = meta.iter().map(|a| a.span.start).min().unwrap_or(start_).min(start_);
    if let Some(d) = dartforge_frontend::comments::Comments::scan(source).dart_doc(source, start) {
        start = start.min(d.start);
    }
    start
}

/// The range of a `show`/`hide` combinator (from the keyword to the last name).
fn combinator_span(source: &str, names: &[ast::Name], word: &str) -> Span {
    let first = names.first().map_or(0, |n| n.span.start);
    let last = names.last().map_or(first, |n| n.span.end);
    let before = source[..first].trim_end();
    let start = if before.ends_with(word) { before.len() - word.len() } else { first };
    Span { start, end: last }
}

/// `fileHeader`: the first comment before the first token (after the `#!`):
/// a non-doc `/* */`, or the consecutive `//` lines.
fn file_header(source: &str) -> Option<(usize, usize)> {
    let tokens = dartforge_frontend::lexer::lex(source).ok()?;
    let mut first = tokens.first()?;
    if first.kind == Kind::ScriptTag {
        first = tokens.get(1)?;
    }
    let search_start = if tokens[0].kind == Kind::ScriptTag { tokens[0].span.end } else { 0 };
    let comments: Vec<Span> = dartforge_frontend::comments::Comments::scan(source)
        .all()
        .iter()
        .copied()
        .filter(|c| c.start >= search_start && c.end <= first.span.start)
        .collect();
    let c0 = *comments.first()?;
    let lexeme = &source[c0.start..c0.end];
    if lexeme.starts_with("/**") || lexeme.starts_with("///") {
        return None;
    }
    if lexeme.starts_with("/*") {
        return Some((c0.start, c0.end));
    }
    if !lexeme.starts_with("//") {
        return None;
    }
    let lines = Lines::from(source);
    let mut end = c0.end;
    let mut previous_line = lines.number(c0.start);
    for c in comments.iter().skip(1) {
        let l = &source[c.start..c.end];
        if !l.starts_with("//") || l.starts_with("///") {
            break;
        }
        let line = lines.number(c.start);
        if line != previous_line + 1 {
            break;
        }
        end = c.end;
        previous_line = line;
    }
    Some((c0.start, end))
}

/// The `ImportAnalyzer`.
#[derive(Default)]
struct ImportsAnalyzer {
    moving: Vec<Element>,
    staying: Vec<Element>,
    /// Element → the indexes (in the library's `imports`) used.
    refs_moving: Vec<(Element, Vec<usize>)>,
    refs_staying: Vec<(Element, Vec<usize>)>,
}

impl ImportsAnalyzer {
    fn register(list: &mut Vec<(Element, Vec<usize>)>, el: Element, import: Option<usize>) {
        let i = match list.iter().position(|(e, _)| *e == el) {
            Some(i) => i,
            None => {
                list.push((el, Vec::new()));
                list.len() - 1
            }
        };
        if let Some(k) = import
            && !list[i].1.contains(&k)
        {
            list[i].1.push(k);
        }
    }

    fn has_moving_to_staying(&self) -> bool {
        self.staying.iter().any(|d| self.refs_moving.iter().any(|(e, _)| e == d))
    }

    fn has_staying_to_moving(&self) -> bool {
        self.moving.iter().any(|d| self.refs_staying.iter().any(|(e, _)| e == d))
    }
}

/// `dart.refactor.move_top_level_to_file` (`compute`).
/// The imports that a fixes `DartFileEditBuilder` scheduled
/// (`_writeLibraryReference` → `_importLibrary(uri)`), inserted by
/// `_addLibraryImports` in the defining unit of `cx`'s library.
pub(crate) fn builder_imports(cx: &Context<'_>, m: &mut Change, libraries: &std::collections::BTreeSet<dartforge_elements::model::LibraryId>) {
    uris_imports(cx, m, &libraries.iter().map(|&b| cx.p.program().library(b).uri.clone()).collect::<Vec<_>>());
}

/// `importLibraryWithAbsoluteUri`/`importLibraryWithRelativeUri` of a URI in
/// `cx`'s library: the `uriText` and the edits (in ascending order).
pub(crate) fn import_one(cx: &Context<'_>, uri: &str, relative: bool) -> Option<(String, Vec<crate::Edit>)> {
    let prog = cx.p.program();
    let lib = prog.unit(cx.unit).library;
    let &defining = prog.library(lib).units.first()?;
    let uri_def = cx.p.unit_uri(defining)?;
    let u = prog.unit(defining);
    let mut b = Constructor::new(uri_def, u.path.clone(), u.source.clone(), Some(defining), None);
    let text = b.forced_import(cx, uri, ImportOptions { absolute: !relative, relative, ..ImportOptions::default() });
    let mut m = Change::default();
    let eol = Text::new(&u.source).eol();
    b.finish(cx, &mut m, eol);
    if m.conflict.is_some() {
        return None;
    }
    Some((text, m.files.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect()))
}

/// `importLibrary(uri)` of each URI and the `finalize` of the builder of the
/// defining unit of `cx`'s library.
pub(crate) fn uris_imports(cx: &Context<'_>, m: &mut Change, uris: &[String]) {
    if uris.is_empty() {
        return;
    }
    let prog = cx.p.program();
    let lib = prog.unit(cx.unit).library;
    let Some(&defining) = prog.library(lib).units.first() else { return };
    let Some(uri_def) = cx.p.unit_uri(defining) else { return };
    let u = prog.unit(defining);
    let mut b = Constructor::new(uri_def, u.path.clone(), u.source.clone(), Some(defining), None);
    for x in uris {
        b.import_library(cx, x, None, None, false);
    }
    let eol = Text::new(&u.source).eol();
    b.finish(cx, m, eol);
}

pub(crate) fn run(cx: &Context<'_>, uri: &str, offset: usize, length: usize, target_: &str) -> RefactoringResult {
    let p = cx.p;
    let prog = p.program();
    let Some(members) = cx.members_to_move(offset, length) else { return R::Failure(None) };
    let Ok(target_url) = url::Url::parse(target_) else {
        return R::InternalError(format!("FormatException: Invalid URI: {target_}"));
    };
    let Ok(target_path) = target_url.to_file_path() else {
        return R::InternalError(format!("Unsupported operation: Cannot extract a file path from a {} URI", target_url.scheme()));
    };
    let target_uri = url::Url::from_file_path(&target_path).map(|u| u.to_string()).unwrap_or_else(|_| target_.to_string());
    let target_import = path_uri(&target_path);
    let tx = Text::new(cx.source);
    let eol = tx.eol();
    let lines = Lines::from(cx.source);
    // Does the destination exist (on disk or open)?
    let target_unit = p.uri_unit(&target_uri);
    let target_text: Option<String> = match target_unit {
        Some(u) => Some(prog.unit(u).source.clone()),
        None => std::fs::read_to_string(&target_path).ok(),
    };
    let exists = target_text.is_some();
    let mut header: Option<String> = None;
    let mut position = 0usize;
    let mut line_before = false;
    if !exists {
        if let Some((a, b)) = file_header(cx.source) {
            header = Some(cx.source[a..b].to_string());
        }
    } else {
        position = target_text.as_ref().map_or(0, |t| t.len());
        line_before = true;
    }
    // The group's range.
    let nodes: Vec<usize> = members
        .iter()
        .filter_map(|(d, _)| cx.tree.nodes.iter().position(|k| k.mark == crate::analyzer_tree::Mark::Decl(*d) || top_function(cx, k.mark, *d)))
        .collect();
    let range = |with_previous_line: bool| -> (usize, usize) {
        let first = nodes.first().copied().unwrap_or(0);
        let last = nodes.last().copied().unwrap_or(0);
        let mut start = cx.tree.nodes[first].start;
        if with_previous_line {
            let initial_line = lines.number(start);
            // `beginToken.previous`: null when the member starts at a comment.
            let starts_in_comment = cx.comments.iter().any(|c| c.start == start);
            if !starts_in_comment {
                let previous = cx.tokens.iter().rev().find(|t| t.span.end <= start && t.kind != Kind::Eof);
                if let Some(t) = previous {
                    let previous_line = lines.number(t.span.start);
                    if previous_line + 1 < initial_line {
                        start = lines.start(previous_line);
                    }
                }
            }
        }
        let mut end = cx.tree.nodes[last].end;
        let final_line = lines.number(end);
        match cx.tokens.iter().find(|t| t.span.start >= end && t.kind != Kind::Eof) {
            None => end = cx.source.len(),
            Some(t) => {
                let next = lines.number(t.span.start);
                if final_line + 1 < next {
                    end = lines.start(final_line);
                }
            }
        }
        (start, end)
    };
    let range_without = range(false);
    let range_with = range(true);
    // The `ImportAnalyzer`.
    let analyzer = analyze_imports(cx, range_without);
    let lib = prog.unit(cx.unit).library;
    let lib_imports: Vec<&dartforge_elements::model::Import> = {
        let defining = prog.library(lib).units.first().copied();
        prog.library(lib).imports.iter().filter(|i| Some(i.unit) == defining).collect()
    };
    let mut m = Change::default();
    // The destination.
    let mut target_b = Constructor::new(
        target_uri.clone(),
        Some(target_path.clone()),
        target_text.clone().unwrap_or_default(),
        target_unit,
        header.map(|c| format!("{c}{eol}")),
    );
    {
        let mut t = String::new();
        if line_before {
            t.push_str(eol);
        }
        t.push_str(&cx.source[range_without.0..range_without.1]);
        m.add(&target_uri, Span { start: position, end: position }, t);
    }
    if analyzer.has_moving_to_staying()
        && let Some(u) = p.unit_uri(cx.unit)
    {
        let target = unit_library_uri(cx, &u);
        target_b.import_library(cx, &target, None, None, false);
    }
    for (el, imps) in &analyzer.refs_moving {
        for &k in imps {
            let Some(imp) = lib_imports.get(k) else { continue };
            let target = prog.library(imp.library);
            if target.uri == "dart:core" {
                continue;
            }
            let has_show = imp.combinators.iter().any(|c| matches!(c, ast::Combinator::Show(_)));
            let prefix = imp.prefix.map(|x| p.name(x).to_string());
            let name = element_name(cx, *el);
            target_b.import_library(cx, &target.uri.clone(), prefix.as_deref(), name.as_deref(), has_show);
        }
    }
    target_b.finish(cx, &mut m, eol);
    // The source.
    let mut origin_b = Constructor::new(uri.to_string(), prog.unit(cx.unit).path.clone(), cx.source.to_string(), Some(cx.unit), None);
    if analyzer.has_staying_to_moving() {
        origin_b.import_library(cx, &target_import, None, None, false);
    }
    m.add(uri, Span { start: range_with.0, end: range_with.1 }, "");
    origin_b.finish(cx, &mut m, eol);
    // The other libraries that refer to the moved ones.
    let mut by_library: Vec<(LibraryId, Vec<String>)> = Vec::new();
    for el in &analyzer.moving {
        for (b, prefixes) in used_prefixes(cx, *el) {
            if b == lib {
                continue;
            }
            let i = match by_library.iter().position(|(x, _)| *x == b) {
                Some(i) => i,
                None => {
                    by_library.push((b, Vec::new()));
                    by_library.len() - 1
                }
            };
            for pf in prefixes {
                if !by_library[i].1.contains(&pf) {
                    by_library[i].1.push(pf);
                }
            }
        }
    }
    for (b, prefixes) in by_library {
        let Some(&u) = prog.library(b).units.first() else { continue };
        let Some(uri_b) = p.unit_uri(u) else { continue };
        let mut cb = Constructor::new(uri_b, prog.unit(u).path.clone(), prog.unit(u).source.clone(), Some(u), None);
        for pf in &prefixes {
            cb.import_library(cx, &target_import, Some(pf), None, false);
        }
        cb.finish(cx, &mut m, eol);
    }
    // The new file: the creation, with the content of its edits.
    if !exists {
        let edits = m.files.iter().position(|(u, _)| *u == target_uri).map(|i| m.files.remove(i).1).unwrap_or_default();
        let mut content = String::new();
        // The edits are in descending order (at the same offset, the new one
        // first): applied in sequence over the empty text.
        for e in edits.iter() {
            content.insert_str(e.span.start.min(content.len()), &e.text);
        }
        m.create = Some((target_uri, content));
    }
    m.result()
}

/// The mark of a top-level `FunctionDeclaration` is the function's.
fn top_function(cx: &Context<'_>, mark: crate::analyzer_tree::Mark, d: ast::DeclId) -> bool {
    match (mark, &cx.ast.decl(d).kind) {
        (crate::analyzer_tree::Mark::Function(f), DeclKind::Function(g)) => f == *g,
        _ => false,
    }
}

/// The URI of the unit's owning library (`unitResult.uri`: the unit's own,
/// also for a part).
fn unit_library_uri(cx: &Context<'_>, unit_uri: &str) -> String {
    let prog = cx.p.program();
    let lib = prog.unit(cx.unit).library;
    if prog.library(lib).units.first() == Some(&cx.unit) {
        return prog.library(lib).uri.clone();
    }
    match url::Url::parse(unit_uri).ok().and_then(|u| u.to_file_path().ok()) {
        Some(c) => path_uri(&c),
        None => unit_uri.to_string(),
    }
}

/// The name of a top-level element.
fn element_name(cx: &Context<'_>, el: Element) -> Option<String> {
    let p = cx.p;
    let prog = p.program();
    Some(match el {
        Element::Class(c) => p.name(prog.class(c).name).to_string(),
        Element::Typedef(t) => p.name(prog.typedef(t).name).to_string(),
        Element::Extension(x) => p.name(prog.extension(x).name?).to_string(),
        Element::Function(f) => p.name(prog.function(f).name).trim_end_matches('=').to_string(),
        Element::Variable(v) => p.name(prog.variable(v).name).to_string(),
        Element::Prefix(..) => return None,
    })
}

/// The interesting element (`isInterestingReference`) of a resolution:
/// top-level, without a prefix; a synthetic accessor becomes the variable;
/// an extension instance member becomes the extension.
fn interesting_element(cx: &Context<'_>, r: &Resolved) -> Option<Element> {
    let prog = cx.p.program();
    match r {
        Resolved::Element(Element::Prefix(..)) => None,
        Resolved::Element(Element::Function(f)) => {
            let fe = prog.function(*f);
            match (fe.kind, fe.variable) {
                (FunctionKind::ImplicitAccessor, Some(v)) => Some(Element::Variable(v)),
                (FunctionKind::ImplicitAccessor, None) => None,
                _ => Some(Element::Function(*f)),
            }
        }
        Resolved::Element(el) => Some(*el),
        Resolved::ExtensionMember { extension, member } => (!prog.function(*member).static_).then_some(Element::Extension(*extension)),
        Resolved::Constructor(f) => prog.function(*f).class.map(Element::Class),
        _ => None,
    }
}

/// `_getImportForElement`: the import (by the prefix used) that provides
/// the element.
fn element_import(cx: &Context<'_>, unit: UnitId, el: Element, prefix: Option<dartforge_intern::SymbolId>) -> Option<usize> {
    let p = cx.p;
    let prog = p.program();
    let lib = prog.unit(unit).library;
    let defining = prog.library(lib).units.first().copied();
    let imports: Vec<&dartforge_elements::model::Import> = prog.library(lib).imports.iter().filter(|i| Some(i.unit) == defining).collect();
    let name = element_name(cx, el)?;
    let provides = |imp: &dartforge_elements::model::Import| -> bool {
        let target = prog.library(imp.library);
        let Some(b) = target.exported.iter().find(|(s, _)| p.name(**s) == name).map(|(_, b)| *b) else { return false };
        if b.getter != Some(el) && b.setter != Some(el) {
            return false;
        }
        imp.combinators.iter().all(|c| match c {
            ast::Combinator::Show(ns) => ns.iter().any(|n| p.name(n.sym) == name),
            ast::Combinator::Hide(ns) => !ns.iter().any(|n| p.name(n.sym) == name),
        })
    };
    let found = imports.iter().position(|i| i.prefix == prefix && provides(i));
    if found.is_none() && prefix.is_none() && matches!(el, Element::Extension(_)) {
        return imports.iter().position(|i| provides(i));
    }
    found
}

/// The `ImportAnalyzer` of the unit's library, with the moved range.
fn analyze_imports(cx: &Context<'_>, range: (usize, usize)) -> ImportsAnalyzer {
    let p = cx.p;
    let prog = p.program();
    let lib = prog.unit(cx.unit).library;
    let mut a = ImportsAnalyzer::default();
    let moved = |u: UnitId, o: usize| u == cx.unit && range.0 <= o && o <= range.1;
    for &u in prog.library(lib).units.iter() {
        let unit = prog.unit(u);
        let ast = &unit.ast;
        let bodies = &p.query.bodies.units[u.0 as usize];
        // The declarations (by name offset).
        for &d in unit.unit.declarations.iter() {
            let decl = ast.decl(d);
            let mut register = |el: Element, name_: usize| {
                if moved(u, name_) {
                    a.moving.push(el);
                } else {
                    a.staying.push(el);
                }
            };
            match &decl.kind {
                DeclKind::Class(c) => {
                    if let Some(id) = cx.declaration_class(u, d) {
                        register(Element::Class(id), c.name.span.start);
                    }
                }
                DeclKind::Mixin(m) => {
                    if let Some(id) = cx.declaration_class(u, d) {
                        register(Element::Class(id), m.name.span.start);
                    }
                }
                DeclKind::Enum(e) => {
                    if let Some(id) = cx.declaration_class(u, d) {
                        register(Element::Class(id), e.name.span.start);
                    }
                }
                DeclKind::Typedef(t) => {
                    if let Some(i) = prog.typedefs.iter().position(|x| x.decl.unit == u && x.decl.decl == d) {
                        register(Element::Typedef(dartforge_elements::model::TypedefId(i as u32)), t.name.span.start);
                    }
                }
                DeclKind::Extension(x) => {
                    if let Some(i) = prog.extensions.iter().position(|e| e.decl.unit == u && e.decl.decl == d) {
                        let id = dartforge_elements::model::ExtensionId(i as u32);
                        let name_ = x.name.map_or(decl.span.start, |n| n.span.start);
                        register(Element::Extension(id), name_);
                    }
                }
                DeclKind::Function(f) => {
                    if let Some(fe) = p.node_function(u, *f)
                        && let Some(n) = ast.function(*f).name
                    {
                        register(Element::Function(fe), n.span.start);
                    }
                }
                DeclKind::Variables(l) => {
                    for (k, v) in l.variables.iter().enumerate() {
                        let id = prog.variables.iter().position(|x| {
                            x.node == dartforge_elements::model::VariableRef::TopLevel { unit: u, decl: d, index: k }
                        });
                        if let Some(i) = id {
                            register(Element::Variable(dartforge_elements::model::VariableId(i as u32)), v.name.span.start);
                        }
                    }
                }
                DeclKind::ExtensionType(_) => {}
            }
        }
        // The references in the expressions.
        for (i, e) in ast.exprs.iter().enumerate() {
            let x = ExprId(i as u32);
            let Some(r) = bodies.get_resolved(x) else { continue };
            if matches!(r, Resolved::Member { member: MemberRef::Function(_) | MemberRef::Variable(_), .. }) {
                continue;
            }
            let Some(el) = interesting_element(cx, r) else { continue };
            let (offset, prefix) = match &e.kind {
                ExprKind::Identifier(n) => (n.span.start, None),
                ExprKind::Property { target, name, .. } => {
                    let pfx = match &ast.expr(*target).kind {
                        ExprKind::Identifier(pn) if matches!(bodies.get_resolved(*target), Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..)))) => Some(pn.sym),
                        _ => None,
                    };
                    (name.span.start, pfx)
                }
                _ => (e.span.start, None),
            };
            let imp = element_import(cx, u, el, prefix);
            if moved(u, offset) {
                ImportsAnalyzer::register(&mut a.refs_moving, el, imp);
            } else {
                ImportsAnalyzer::register(&mut a.refs_staying, el, imp);
            }
        }
        // The references in the written types.
        for t in ast.types.iter() {
            let ast::TypeKind::Named { name, .. } = &t.kind else { continue };
            let (el, prefix) = match &name[..] {
                [n] => (prog.lookup(unit.library, n.sym).and_then(|b| b.getter), None),
                [pf, n] => (prog.lookup_prefixed(unit.library, pf.sym, n.sym).and_then(|b| b.getter), Some(pf.sym)),
                _ => (None, None),
            };
            let Some(el) = el else { continue };
            if matches!(el, Element::Prefix(..) | Element::Function(_) | Element::Variable(_)) {
                continue;
            }
            let imp = element_import(cx, u, el, prefix);
            if moved(u, t.span.start) {
                ImportsAnalyzer::register(&mut a.refs_moving, el, imp);
            } else {
                ImportsAnalyzer::register(&mut a.refs_staying, el, imp);
            }
        }
    }
    // Without the references that stay in the same file.
    let moving = a.moving.clone();
    let staying = a.staying.clone();
    a.refs_moving.retain(|(e, _)| !moving.contains(e));
    a.refs_staying.retain(|(e, _)| !staying.contains(e));
    a
}

/// The libraries (other than the source one) that refer to `el`, with the
/// prefixes used (`searchPrefixesUsedInLibrary`; `''` without a prefix).
fn used_prefixes(cx: &Context<'_>, el: Element) -> Vec<(LibraryId, Vec<String>)> {
    let p = cx.p;
    let prog = p.program();
    let mut v: Vec<(LibraryId, Vec<String>)> = Vec::new();
    let mut by = |b: LibraryId, pf: String| {
        let i = match v.iter().position(|(x, _)| *x == b) {
            Some(i) => i,
            None => {
                v.push((b, Vec::new()));
                v.len() - 1
            }
        };
        if !v[i].1.contains(&pf) {
            v[i].1.push(pf);
        }
    };
    let mut libraries: Vec<LibraryId> = p.libraries.iter().copied().collect();
    libraries.sort_by_key(|b| b.0);
    for b in libraries {
        for &u in prog.library(b).units.iter() {
            let unit = prog.unit(u);
            let ast = &unit.ast;
            let bodies = &p.query.bodies.units[u.0 as usize];
            for (i, e) in ast.exprs.iter().enumerate() {
                let x = ExprId(i as u32);
                let Some(r) = bodies.get_resolved(x) else { continue };
                if interesting_element(cx, r) != Some(el) {
                    continue;
                }
                let pf = match &e.kind {
                    ExprKind::Property { target, .. } => match &ast.expr(*target).kind {
                        ExprKind::Identifier(pn)
                            if matches!(bodies.get_resolved(*target), Some(Resolved::Prefix(_)) | Some(Resolved::Element(Element::Prefix(..)))) =>
                        {
                            p.name(pn.sym).to_string()
                        }
                        _ => String::new(),
                    },
                    _ => String::new(),
                };
                by(b, pf);
            }
            for t in ast.types.iter() {
                let ast::TypeKind::Named { name, .. } = &t.kind else { continue };
                let (found, pf) = match &name[..] {
                    [n] => (prog.lookup(unit.library, n.sym).and_then(|x| x.getter), String::new()),
                    [pn, n] => (prog.lookup_prefixed(unit.library, pn.sym, n.sym).and_then(|x| x.getter), p.name(pn.sym).to_string()),
                    _ => (None, String::new()),
                };
                if found == Some(el) {
                    by(b, pf);
                }
            }
        }
    }
    v
}
