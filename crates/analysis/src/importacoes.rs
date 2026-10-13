//! Unused imports: the `ImportsVerifier` of analyzer 6.11.0
//! (`src/error/imports_verifier.dart`) — `UNUSED_IMPORT` and
//! `UNUSED_SHOWN_NAME` — with the `LibraryAnalyzer`'s suppression
//! (`_hasDiagnosticReportedThatPreventsImportWarnings`: with an unresolved
//! name in the library, no import warning comes out).
//!
//! The use is the analyzer's tracking (`ImportsTrackingOfPrefix`), done over
//! the unit's lexical lookups with the local and member scopes of
//! `dartforge_types::anotacoes::sombreado`, and the extensions chosen during
//! member resolution (`BodyTypes::extensoes_usadas`); see [`unused`].

use dartforge_diagnostics::codes::{compile_time_error as c, warning as w};
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::{Element, LibraryId, Program, UnitRole};
use dartforge_frontend::ast::{Combinator, DirectiveKind, ExprKind, PatternKind, TypeKind};
use dartforge_intern::{Interner, SymbolId};
use std::collections::HashSet;

/// Codes that, reported in the library, suppress the import warnings.
fn suppresses(d: &Diagnostic) -> bool {
    let Some(code) = d.code else { return false };
    [
        c::AMBIGUOUS_IMPORT,
        c::CONST_WITH_NON_TYPE,
        c::EXTENDS_NON_CLASS,
        c::IMPLEMENTS_NON_CLASS,
        c::MIXIN_OF_NON_CLASS,
        c::NEW_WITH_NON_TYPE,
        c::NOT_A_TYPE,
        c::PREFIX_IDENTIFIER_NOT_FOLLOWED_BY_DOT,
        c::UNDEFINED_ANNOTATION,
        c::UNDEFINED_CLASS,
        c::UNDEFINED_FUNCTION,
        c::UNDEFINED_IDENTIFIER,
        c::UNDEFINED_PREFIXED_NAME,
        w::DEPRECATED_EXPORT_USE,
    ]
    .contains(&code)
}

/// `DEFERRED_IMPORT_OF_EXTENSION` (`ErrorVerifier._checkForDeferredImportOfExtensions`,
/// `analyzer/lib/src/generated/error_verifier.dart:3053-3066`;
/// docs/ANALYZER-ESPECIFICACAO.md §A): a `deferred` import whose namespace,
/// after `show`/`hide`, still brings some extension. On the import's URI.
/// Written without compiling or running (2026-10-04).
/// The types of the formal parameters (that of a function-typed parameter is
/// its return type), also those of the nested old-style ones.
fn param_types(ps: &[dartforge_frontend::ast::Parameter], out: &mut Vec<dartforge_frontend::ast::TypeId>) {
    for p in ps {
        out.extend(p.ty);
        for tp in p.function_type_params.iter() {
            out.extend(tp.bound);
        }
        if let Some(fs) = &p.function_parameters {
            param_types(fs, out);
        }
    }
}

/// `TYPE_ANNOTATION_DEFERRED_CLASS` (`_checkForTypeAnnotationDeferredClass`,
/// `an611:src/generated/error_verifier.dart:5369-5377`): a named type `p.T`
/// whose prefix has exactly one import, and that one `deferred`, in one of the
/// positions the `ErrorVerifier` checks: `as`/`is`, the `on` of a `catch`,
/// the type of a formal parameter, the return of a function, method or
/// function-typed parameter, the bound of a type parameter, the type of a
/// variable list and each element of every type-argument list. Not: clauses,
/// the new-style `typedef`, the function type and the record itself, the
/// patterns, the `for-in` and the type of a creation (`new p.T()`). On the
/// whole type, with `p.T`.
pub fn deferred_types(program: &Program, lib: LibraryId, names: &Interner) -> Vec<(dartforge_elements::model::UnitId, Diagnostic)> {
    use dartforge_frontend::ast::{self, DeclKind, MemberKind, StmtKind};
    let library = program.library(lib);
    let mut by_prefix: std::collections::HashMap<SymbolId, (usize, bool)> = std::collections::HashMap::new();
    for i in &library.imports {
        if let Some(p) = i.prefix {
            let e = by_prefix.entry(p).or_insert((0, false));
            e.0 += 1;
            e.1 = i.deferred;
        }
    }
    let deferred: HashSet<SymbolId> = by_prefix.into_iter().filter(|(_, (n, d))| *n == 1 && *d).map(|(p, _)| p).collect();
    let mut out = Vec::new();
    if deferred.is_empty() {
        return out;
    }
    let bounds = |tps: &[ast::TypeParameter], targets: &mut Vec<ast::TypeId>| {
        for tp in tps {
            targets.extend(tp.bound);
        }
    };
    for &u in &library.units {
        let a = &program.unit(u).ast;
        let mut targets: Vec<ast::TypeId> = Vec::new();
        for ty in &a.types {
            match &ty.kind {
                TypeKind::Named { args, .. } => targets.extend(args.iter().copied()),
                TypeKind::Function { parameters, type_params, .. } => {
                    param_types(parameters, &mut targets);
                    bounds(type_params, &mut targets);
                }
                _ => {}
            }
        }
        for e in &a.exprs {
            match &e.kind {
                ExprKind::As { ty, .. } | ExprKind::Is { ty, .. } => targets.push(*ty),
                ExprKind::List { type_args, .. } | ExprKind::SetOrMap { type_args, .. } | ExprKind::TypeArguments { type_args, .. } => {
                    targets.extend(type_args.iter().copied())
                }
                ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => targets.extend(arguments.type_args.iter().copied()),
                _ => {}
            }
        }
        for s in &a.stmts {
            match &s.kind {
                StmtKind::Variables(l) => targets.extend(l.ty),
                StmtKind::Try { catches, .. } => targets.extend(catches.iter().filter_map(|c| c.on_type)),
                _ => {}
            }
        }
        for f in &a.functions {
            if f.name.is_some() {
                targets.extend(f.return_type);
            }
            if let Some(ps) = &f.parameters {
                param_types(ps, &mut targets);
            }
            bounds(&f.type_params, &mut targets);
        }
        for m in &a.members {
            match &m.kind {
                MemberKind::Field(l) => targets.extend(l.ty),
                MemberKind::Constructor(c) => param_types(&c.parameters, &mut targets),
                MemberKind::Method(_) => {}
            }
        }
        for d in &a.decls {
            match &d.kind {
                DeclKind::Variables(l) => targets.extend(l.ty),
                DeclKind::Class(k) => bounds(&k.type_params, &mut targets),
                DeclKind::Mixin(k) => bounds(&k.type_params, &mut targets),
                DeclKind::Enum(k) => bounds(&k.type_params, &mut targets),
                DeclKind::Extension(k) => bounds(&k.type_params, &mut targets),
                DeclKind::ExtensionType(k) => bounds(&k.type_params, &mut targets),
                DeclKind::Typedef(k) => {
                    bounds(&k.type_params, &mut targets);
                    if let ast::TypedefKind::Legacy { return_type, parameters } = &k.kind {
                        targets.extend(*return_type);
                        param_types(parameters, &mut targets);
                    }
                }
                DeclKind::Function(_) => {}
            }
        }
        targets.sort();
        targets.dedup();
        for t in targets {
            let annotation = a.ty(t);
            let TypeKind::Named { name, .. } = &annotation.kind else { continue };
            let [p, n] = &name[..] else { continue };
            if !deferred.contains(&p.sym) {
                continue;
            }
            let text = format!("{}.{}", names.resolve(p.sym), names.resolve(n.sym));
            out.push((u, Diagnostic::with_code(c::TYPE_ANNOTATION_DEFERRED_CLASS, annotation.span, [text.as_str()])));
        }
    }
    out
}

pub fn deferred_extensions(program: &Program, lib: LibraryId) -> Vec<(dartforge_elements::model::UnitId, Diagnostic)> {
    let mut out = Vec::new();
    for imp in &program.library(lib).imports {
        if !imp.deferred {
            continue;
        }
        let visible = |name: SymbolId| -> bool {
            imp.combinators.iter().all(|k| match k {
                Combinator::Show(names) => names.iter().any(|n| n.sym == name),
                Combinator::Hide(names) => !names.iter().any(|n| n.sym == name),
            })
        };
        let has_extension = program
            .library(imp.library)
            .exported
            .iter()
            .any(|(name, b)| matches!(b.getter, Some(Element::Extension(_))) && visible(*name));
        if !has_extension {
            continue;
        }
        let Some(dir) = program.unit(imp.unit).unit.directives.get(imp.directive) else { continue };
        let DirectiveKind::Import { uri, .. } = &dir.kind else { continue };
        out.push((imp.unit, Diagnostic::with_code(c::DEFERRED_IMPORT_OF_EXTENSION, uri.span, std::iter::empty::<&str>())));
    }
    out
}

/// The internal libraries of SDK 3.6.2 (`isInternal`: empty `categories` in
/// `lib/_internal/sdk_library_metadata/lib/libraries.dart`).
const INTERNAL_LIBRARIES: &[&str] = &[
    "_http", "_native_typed_data", "_internal", "_js_helper", "_late_helper", "_rti", "_dart2js_only",
    "_dart2js_runtime_metrics", "_interceptors", "_foreign_helper", "_js_names", "_js_primitives",
    "_js_embedded_names", "_js_shared_embedded_names", "_js_types", "_async_status_codes",
    "_invocation_mirror_constants", "_recipe_syntax", "_load_library_priority", "_metadata",
    "_js_annotations", "_wasm", "_macros",
];

fn is_internal(uri: &str) -> bool {
    uri.strip_prefix("dart:").is_some_and(|n| INTERNAL_LIBRARIES.contains(&n))
}

/// `_checkForImportInternalLibrary` (`error_verifier.dart:3800-3822`, on the
/// URI), `_checkForExportInternalLibrary` (`:3263-3302`, on the whole
/// directive) and `_checkDeferredPrefixCollision` (`:1911-1923`: two or more
/// imports of the unit with the same prefix, each `deferred` on its token).
/// Outside the `dart:` libraries; `dart:_wasm` is allowed in
/// `package:js/js.dart` and `package:ui/`, `dart:_macros` re-exported in `package:macros`.
pub fn internal_and_deferred_directives(program: &Program, lib: LibraryId) -> Vec<(dartforge_elements::model::UnitId, Diagnostic)> {
    let mut out = Vec::new();
    let library = program.library(lib);
    let current = library.uri.as_str();
    let in_sdk = current.starts_with("dart:");
    if !in_sdk {
        for imp in &library.imports {
            let target = program.library(imp.library).uri.as_str();
            if !is_internal(target) || program.library(imp.library).units.is_empty() {
                continue;
            }
            if target == "dart:_wasm" && (current == "package:js/js.dart" || current.starts_with("package:ui/")) {
                continue;
            }
            let Some(dir) = program.unit(imp.unit).unit.directives.get(imp.directive) else { continue };
            let DirectiveKind::Import { uri, .. } = &dir.kind else { continue };
            out.push((imp.unit, Diagnostic::with_code(c::IMPORT_INTERNAL_LIBRARY, uri.span, [target])));
        }
        for exp in &library.exports {
            let target = program.library(exp.library).uri.as_str();
            if !is_internal(target) || program.library(exp.library).units.is_empty() {
                continue;
            }
            if target == "dart:_macros" && current.starts_with("package:macros/") {
                continue;
            }
            let Some(dir) = program.unit(exp.unit).unit.directives.get(exp.directive) else { continue };
            out.push((exp.unit, Diagnostic::with_code(c::EXPORT_INTERNAL_LIBRARY, dir.span, [target])));
        }
    }
    // `SHARED_DEFERRED_PREFIX`, unit by unit.
    for &u in &library.units {
        let unit = program.unit(u);
        let mut by_prefix: std::collections::HashMap<SymbolId, Vec<usize>> = std::collections::HashMap::new();
        for (i, d) in unit.unit.directives.iter().enumerate() {
            if let DirectiveKind::Import { prefix: Some(p), .. } = &d.kind {
                by_prefix.entry(p.sym).or_default().push(i);
            }
        }
        let mut groups: Vec<Vec<usize>> = by_prefix.into_values().filter(|v| v.len() > 1).collect();
        groups.sort();
        for g in groups {
            for i in g {
                let d = &unit.unit.directives[i];
                let DirectiveKind::Import { uri, deferred: true, prefix: Some(p), .. } = &d.kind else { continue };
                // The `deferred` token, between the URI and the prefix.
                let fragment = unit.source.get(uri.span.end..p.span.start).unwrap_or("");
                if let Some(k) = fragment.find("deferred") {
                    let start = uri.span.end + k;
                    out.push((u, Diagnostic::with_code(c::SHARED_DEFERRED_PREFIX, dartforge_diagnostics::Span { start, end: start + "deferred".len() }, [] as [&str; 0])));
                }
            }
        }
    }
    out
}

/// `UNDEFINED_SHOWN_NAME` (`DeadCodeVerifier._checkCombinator`,
/// `analyzer/lib/src/error/dead_code_verifier.dart:131-155`;
/// docs/ANALYZER-ESPECIFICACAO.md §F): a `show` name, in an `import` or
/// `export`, that the target library does not export (neither as a getter nor
/// as a setter). On the combinator's identifier; it depends neither on use nor
/// on the import suppression. Written without compiling or running (2026-10-04).
pub fn undefined_shown_names(program: &Program, lib: LibraryId, interner: &Interner) -> Vec<(dartforge_elements::model::UnitId, Diagnostic)> {
    let mut out = Vec::new();
    let library = program.library(lib);
    let directives = library
        .imports
        .iter()
        .map(|i| (i.unit, i.library, &i.combinators))
        .chain(library.exports.iter().map(|e| (e.unit, e.library, &e.combinators)));
    for (unit, target, combinadores) in directives {
        // A target that does not exist (`importedLibrary == null`, only
        // `URI_DOES_NOT_EXIST`): the library has no units.
        if program.library(target).units.is_empty() {
            continue;
        }
        let exportado = &program.library(target).exported;
        let uri = program.library(target).uri.as_str();
        for k in combinadores.iter() {
            let Combinator::Show(names) = k else { continue };
            for n in names.iter() {
                if !exportado.contains_key(&n.sym) {
                    out.push((unit, Diagnostic::with_code(w::UNDEFINED_SHOWN_NAME, n.span, [uri, interner.resolve(n.sym)])));
                }
            }
        }
    }
    out
}

/// The library that declares a top-level element.
fn element_library(program: &Program, e: Element) -> Option<LibraryId> {
    Some(match e {
        Element::Class(c) => program.class(c).library,
        Element::Extension(x) => program.extension(x).library,
        Element::Typedef(t) => program.typedef(t).library,
        Element::Function(f) => program.function(f).library,
        Element::Variable(v) => program.variable(v).library,
        Element::Prefix(..) => return None,
    })
}

/// `AMBIGUOUS_EXPORT` (`ErrorVerifier._checkForAmbiguousExport`,
/// `analyzer/lib/src/generated/error_verifier.dart:2085-2111`): the exports in
/// order, with a shared name → element map; the first name of a directive
/// that already came from another one with ANOTHER element is reported on its
/// URI, and the rest of the directive does not count. Written without
/// compiling or running.
pub fn ambiguous_exports(program: &Program, lib: LibraryId, interner: &Interner) -> Vec<(dartforge_elements::model::UnitId, Diagnostic)> {
    let mut out = Vec::new();
    // (name, is the setter slot) → element.
    let mut seen: std::collections::HashMap<(SymbolId, bool), Element> = std::collections::HashMap::new();
    for exp in &program.library(lib).exports {
        let visible = |name: SymbolId| -> bool {
            exp.combinators.iter().all(|k| match k {
                Combinator::Show(names) => names.iter().any(|n| n.sym == name),
                Combinator::Hide(names) => !names.iter().any(|n| n.sym == name),
            })
        };
        // The order of a namespace's names is not the source's: by text order,
        // so the report is stable.
        let mut entries: Vec<((SymbolId, bool), Element)> = Vec::new();
        for (name, b) in program.library(exp.library).exported.iter() {
            if !visible(*name) {
                continue;
            }
            if let Some(e) = b.getter {
                entries.push(((*name, false), e));
            }
            if let Some(e) = b.setter {
                entries.push(((*name, true), e));
            }
        }
        entries.sort_by(|x, y| interner.resolve(x.0 .0).cmp(interner.resolve(y.0 .0)).then(x.0 .1.cmp(&y.0 .1)));
        let mut conflito: Option<(SymbolId, Element, Element)> = None;
        let mut new: Vec<((SymbolId, bool), Element)> = Vec::new();
        for (key, e) in entries {
            match seen.get(&key) {
                Some(previous) if *previous != e => {
                    conflito = Some((key.0, *previous, e));
                    break;
                }
                Some(_) => {}
                None => new.push((key, e)),
            }
        }
        for (key, e) in new {
            seen.insert(key, e);
        }
        let Some((name, previous, current)) = conflito else { continue };
        let Some(dir) = program.unit(exp.unit).unit.directives.get(exp.directive) else { continue };
        let DirectiveKind::Export { uri, .. } = &dir.kind else { continue };
        let uri_of = |e: Element| -> String { element_library(program, e).map(|l| program.library(l).uri.clone()).unwrap_or_default() };
        let (one, other) = (uri_of(previous), uri_of(current));
        out.push((exp.unit, Diagnostic::with_code(c::AMBIGUOUS_EXPORT, uri.span, [interner.resolve(name), one.as_str(), other.as_str()])));
    }
    out
}

/// `ImportAddShow` (`import_add_show.dart`, the `_ReferenceFinder`): the names
/// unit `u` uses from the import of directive `directive` — each name lookup
/// whose element, in the import scope, is the one this import provides under
/// the name. The scope's element follows `PrefixScope._merge`: two imports
/// with different elements under the same name give the one outside the SDK
/// when only one is from the SDK, and otherwise the multiple element, which
/// belongs to none. An import with a prefix has no names (its namespace is
/// indexed by `p.x`). The extensions used without a name are left to the
/// caller, which has the unit's bodies. `None`: the directive is not an
/// import with a target.
pub fn nomes_usados_do_import(program: &Program, u: dartforge_elements::model::UnitId, directive: usize, interner: &Interner) -> Option<Vec<SymbolId>> {
    let lib = program.unit(u).library;
    let library = program.library(lib);
    let mut imps: Vec<Imp<'_>> = Vec::new();
    let mut this_one = None;
    for imp in &library.imports {
        let unit = program.unit(imp.unit);
        let Some(dir) = unit.unit.directives.get(imp.directive) else { continue };
        let DirectiveKind::Import { uri, combinators, .. } = &dir.kind else { continue };
        let Some(text) = dartforge_elements::load::string_lit_value(uri) else { continue };
        if imp.unit == u && imp.directive == directive {
            this_one = Some(imps.len());
        }
        imps.push(Imp { unit: imp.unit, prefix: imp.prefix, deferred: imp.deferred, target: imp.library, combinators, uri: uri.span, text });
    }
    let this_one = this_one?;
    if imps[this_one].prefix.is_some() {
        return Some(Vec::new());
    }
    let prefixes: HashSet<SymbolId> = imps.iter().filter_map(|i| i.prefix).collect();
    let declared = &library.declared;
    let is_sdk = |e: Element| element_library(program, e).is_some_and(|l| program.library(l).is_sdk);
    // `_merge` over the elements the unprefixed imports provide.
    // (the multiple element is not from the SDK and absorbs what comes after).
    let from_scope = |elements: &mut dyn Iterator<Item = Element>| -> Option<Element> {
        let mut current: Option<Element> = None;
        for e in elements {
            match current {
                None => current = Some(e),
                Some(a) if a == e => {}
                Some(a) if is_sdk(a) && !is_sdk(e) => current = Some(e),
                Some(a) if !is_sdk(a) && is_sdk(e) => {}
                Some(_) => return None,
            }
        }
        current
    };
    let mut names: Vec<SymbolId> = Vec::new();
    let is_prefix = |a: &dartforge_frontend::ast::Ast, pos: usize, p: SymbolId| {
        prefixes.contains(&p) && !declared.contains_key(&p) && !dartforge_types::anotacoes::sombreado(a, interner, pos, p)
    };
    let unit = program.unit(u);
    unit_lookups(unit, interner, &is_prefix, &|p| prefixes.contains(&p) && !declared.contains_key(&p), &mut |b| {
        let Lookup::Name(n) = b else { return };
        if declared.contains_key(&n) || prefixes.contains(&n) || names.contains(&n) {
            return;
        }
        let Some(own) = provides(program, &imps[this_one], n) else { return };
        let unprefixed = || imps.iter().filter(|i| i.prefix.is_none()).filter_map(|i| provides(program, i, n));
        let getter = from_scope(&mut unprefixed().filter_map(|b| b.getter));
        let setter = from_scope(&mut unprefixed().filter_map(|b| b.setter));
        if (own.getter.is_some() && own.getter == getter) || (own.setter.is_some() && own.setter == setter) {
            names.push(n);
        }
    });
    Some(names)
}

/// A lexical name lookup that reaches the import scope
/// ([`unit_lookups`]).
enum Lookup {
    /// An unprefixed name, already outside the local scopes.
    Name(SymbolId),
    /// `p.x` with `p` an import prefix.
    Prefixed(SymbolId, SymbolId),
    /// `[p]`: the prefix cited in a documentation comment.
    PrefixInComment(SymbolId),
}

/// The name lookups of a unit that the import usage tracking sees
/// (`ImportsTrackingOfPrefix`): expression identifiers, type names,
/// annotation names, loose constant patterns and the references of
/// documentation comments. `is_prefix(ast, pos, p)`: `p` is an import prefix
/// at that place; `simple_prefix(p)`: `p` is a prefix (in comments, which
/// have no scope position of their own).
fn unit_lookups(
    unit: &dartforge_elements::model::Unit,
    interner: &Interner,
    is_prefix: &dyn Fn(&dartforge_frontend::ast::Ast, usize, SymbolId) -> bool,
    simple_prefix: &dyn Fn(SymbolId) -> bool,
    f: &mut dyn FnMut(Lookup),
) {
    let a = &unit.ast;
    let source = unit.source.as_str();
    // The identifiers that are the prefix of a `p.x`.
    let mut from_prefix: HashSet<dartforge_frontend::ast::ExprId> = HashSet::new();
    for e in a.exprs.iter() {
        if let ExprKind::Property { target, name, .. } = &e.kind
            && let ExprKind::Identifier(p) = &a.expr(*target).kind
            && is_prefix(a, p.span.start, p.sym)
        {
            from_prefix.insert(*target);
            f(Lookup::Prefixed(p.sym, name.sym));
        }
    }
    // The expression identifiers.
    for (k, e) in a.exprs.iter().enumerate() {
        if let ExprKind::Identifier(n) = &e.kind
            && !from_prefix.contains(&dartforge_frontend::ast::ExprId(k as u32))
            && !dartforge_types::anotacoes::sombreado(a, interner, n.span.start, n.sym)
        {
            f(Lookup::Name(n.sym));
        }
    }
    // The type names.
    for t in a.types.iter() {
        if let TypeKind::Named { name, .. } = &t.kind {
            match &name[..] {
                [p, n, ..] if is_prefix(a, p.span.start, p.sym) => f(Lookup::Prefixed(p.sym, n.sym)),
                [n, ..] => {
                    if !dartforge_types::anotacoes::sombreado(a, interner, n.span.start, n.sym) {
                        f(Lookup::Name(n.sym));
                    }
                }
                [] => {}
            }
        }
    }
    // The annotation names.
    for m in dartforge_frontend::pais::todas_as_anotacoes(a, &unit.unit) {
        match &m.name[..] {
            [p, n, ..] if is_prefix(a, m.span.start, p.sym) => f(Lookup::Prefixed(p.sym, n.sym)),
            [n, ..] => {
                if !dartforge_types::anotacoes::sombreado(a, interner, m.span.start, n.sym) {
                    f(Lookup::Name(n.sym));
                }
            }
            [] => {}
        }
    }
    // The loose name in a refutable pattern: a constant pattern.
    for n in pattern_constants(a) {
        if !dartforge_types::anotacoes::sombreado(a, interner, n.span.start, n.sym) {
            f(Lookup::Name(n.sym));
        }
    }
    // The references of documentation comments, in the scope of the
    // documented declaration.
    let comments = dartforge_frontend::comentarios::Comentarios::de(source);
    for (doc, pos, extras) in doc_comments(a, &unit.unit, source, &comments) {
        for parts in doc_references(&source[doc.start..doc.end]) {
            let simbolos: Vec<Option<SymbolId>> = parts.iter().map(|x| interner.lookup(x)).collect();
            let Some(first) = simbolos[0] else { continue };
            if extras.contains(&first) || dartforge_types::anotacoes::sombreado(a, interner, pos, first) {
                continue;
            }
            let prefix = simple_prefix(first);
            if prefix {
                match simbolos.get(1) {
                    Some(Some(n)) => f(Lookup::Prefixed(first, *n)),
                    Some(None) => {}
                    // `[p]`: the prefix cited in a comment.
                    None => f(Lookup::PrefixInComment(first)),
                }
            } else {
                f(Lookup::Name(first));
            }
        }
    }
}

/// An import of the library, with what the verifier reads from it.
struct Imp<'a> {
    unit: dartforge_elements::model::UnitId,
    prefix: Option<SymbolId>,
    deferred: bool,
    target: LibraryId,
    combinators: &'a [Combinator],
    uri: dartforge_diagnostics::Span,
    text: String,
}

/// What import `imp` provides under `name` (the target's exported namespace
/// with the combinators).
fn provides<'p>(program: &'p Program, imp: &Imp<'_>, name: SymbolId) -> Option<&'p dartforge_elements::model::Binding> {
    let b = program.library(imp.target).exported.get(&name)?;
    imp.combinators
        .iter()
        .all(|c| match c {
            Combinator::Show(ns) => ns.iter().any(|n| n.sym == name),
            Combinator::Hide(ns) => !ns.iter().any(|n| n.sym == name),
        })
        .then_some(b)
}

/// The references of a documentation comment (the `DocCommentBuilder`'s
/// `_parseReferences`), outside fenced or indented code blocks: each `[…]`
/// that is not link text (followed by `(`, by `:` at the start of the line,
/// or by another `[`), nor `[:…:]`, outside `` `…` ``; the parts `a`, `a.b` or
/// `a.b.c` (without the leading `new`).
fn doc_references(text: &str) -> Vec<Vec<String>> {
    let mut v = Vec::new();
    // The content lines: without the `///`, or without the `/**`, the `*/` and
    // the leading `*`.
    let lines: Vec<&str> = if let Some(r) = text.strip_prefix("/**") {
        let r = r.strip_suffix("*/").unwrap_or(r);
        r.split('\n')
            .map(|l| {
                let l = l.trim_end_matches('\r');
                let t = l.trim_start();
                t.strip_prefix('*').unwrap_or(l)
            })
            .collect()
    } else {
        text.split('\n').map(|l| l.trim_start().strip_prefix("///").unwrap_or(l).trim_end_matches('\r')).collect()
    };
    let mut fenced = false;
    let mut previous_empty = true;
    for contents in lines {
        let indent = contents.len() - contents.trim_start().len();
        if contents.trim_start().starts_with("```") {
            fenced = !fenced;
            previous_empty = false;
            continue;
        }
        if fenced || (previous_empty && indent >= 4) {
            previous_empty = contents.trim().is_empty();
            continue;
        }
        previous_empty = contents.trim().is_empty();
        let b = contents.as_bytes();
        let mut i = 0;
        let mut only_blanks = true;
        while i < b.len() {
            match b[i] {
                b'[' => {
                    i += 1;
                    if b.get(i) == Some(&b':') {
                        match contents[i + 1..].find(":]") {
                            Some(k) => i = i + 1 + k + 1,
                            None => break,
                        }
                    } else {
                        let start = i;
                        let Some(k) = contents[i..].find(']') else {
                            // Without the `]` (`_findCommentReferenceEnd`): the
                            // identifier, and its `.identifier`.
                            let letter = |c: u8| c.is_ascii_alphabetic() || c == b'_' || c == b'$';
                            let letter_or_digit = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
                            let mut j = i;
                            if b.get(j).is_some_and(|c| letter(*c)) {
                                while b.get(j).is_some_and(|c| letter_or_digit(*c)) {
                                    j += 1;
                                }
                                if b.get(j) == Some(&b'.') && b.get(j + 1).is_some_and(|c| letter(*c)) {
                                    j += 1;
                                    while b.get(j).is_some_and(|c| letter_or_digit(*c)) {
                                        j += 1;
                                    }
                                }
                                v.push(contents[start..j].split('.').map(|x| x.to_string()).collect());
                            }
                            break;
                        };
                        let end = i + k;
                        // `_isLinkText`.
                        let mut j = end + 1;
                        let link = match b.get(j) {
                            Some(b'(') => true,
                            Some(b':') if only_blanks => true,
                            _ => {
                                while b.get(j).is_some_and(|c| c.is_ascii_whitespace()) {
                                    j += 1;
                                }
                                b.get(j) == Some(&b'[')
                            }
                        };
                        if !link {
                            let mut parts: Vec<String> = contents[start..end].trim().split('.').map(|x| x.trim().to_string()).collect();
                            if let Some(first) = parts.first_mut()
                                && let Some(r) = first.strip_prefix("new ")
                            {
                                *first = r.trim().to_string();
                            }
                            let identifier = |x: &String| {
                                !x.is_empty()
                                    && x.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
                                    && x.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
                            };
                            if !parts.is_empty() && parts.len() <= 3 && parts.iter().all(identifier) {
                                v.push(parts);
                            }
                        }
                        i = end;
                    }
                    only_blanks = false;
                }
                b'`' => {
                    if let Some(k) = contents[i + 1..].find('`') {
                        i = i + 1 + k;
                    }
                    only_blanks = false;
                }
                c if !c.is_ascii_whitespace() => only_blanks = false,
                _ => {}
            }
            i += 1;
        }
    }
    v
}

/// 3.6.2's `ImportsVerifier.generateUnusedImportHints`,
/// `generateUnusedShownNameHints` and `generateUnnecessaryImportHints`, over
/// the analyzer's usage tracking (`ImportsTrackingOfPrefix`,
/// `dart/element/scope.dart`): each lexical lookup of a name that does not
/// stop at a local or member scope (`anotacoes::sombreado`) nor at the
/// library's declarations (or at a prefix) and reaches the import scope marks,
/// in each unprefixed import that provides the name, the getter and setter
/// found; `p.x` marks the imports of prefix `p` (a `deferred` prefix's
/// `loadLibrary` does not mark); the extension chosen in a member resolution
/// (`BodyTypes::extensoes_usadas`) marks the imports that provide it. The
/// lookups: expression identifiers, type names, annotation names, loose
/// constant patterns and documentation comment references (in the scope of
/// the documented declaration); `[p]` of a prefix in a comment frees its
/// imports. Without the bodies (`bodies` absent), the brought extension counts
/// as used.
pub fn unused(
    program: &Program,
    lib: LibraryId,
    interner: &Interner,
    already_reported: &[Diagnostic],
    bodies: Option<&dartforge_types::resolved::BodyTypes>,
) -> Vec<(dartforge_elements::model::UnitId, Diagnostic)> {
    if already_reported.iter().any(suppresses) {
        return Vec::new();
    }
    let library = program.library(lib);
    // The imports with a target.
    let mut imps: Vec<Imp<'_>> = Vec::new();
    for imp in &library.imports {
        let unit = program.unit(imp.unit);
        let Some(dir) = unit.unit.directives.get(imp.directive) else { continue };
        let DirectiveKind::Import { uri, combinators, .. } = &dir.kind else { continue };
        let Some(text) = dartforge_elements::load::string_lit_value(uri) else { continue };
        imps.push(Imp {
            unit: imp.unit,
            prefix: imp.prefix,
            deferred: imp.deferred,
            target: imp.library,
            combinators,
            uri: uri.span,
            text,
        });
    }
    let prefixes: HashSet<SymbolId> = imps.iter().filter_map(|i| i.prefix).collect();
    let declared = &library.declared;
    let load_library = interner.lookup("loadLibrary");
    let mut used: Vec<HashSet<Element>> = vec![HashSet::new(); imps.len()];
    let mut prefix_in_comment: HashSet<SymbolId> = HashSet::new();

    // The lexical lookup of `name` (already without the local scopes): the
    // library's declarations and the prefixes stop it; otherwise, the unprefixed imports.
    let mark = |used: &mut Vec<HashSet<Element>>, name: SymbolId| {
        if declared.contains_key(&name) || prefixes.contains(&name) {
            return;
        }
        for (i, imp) in imps.iter().enumerate() {
            if imp.prefix.is_none()
                && let Some(b) = provides(program, imp, name)
            {
                used[i].extend(b.getter);
                used[i].extend(b.setter);
            }
        }
    };
    // `p.x` with `p` a prefix.
    let mark_prefixed = |used: &mut Vec<HashSet<Element>>, p: SymbolId, name: SymbolId| {
        for (i, imp) in imps.iter().enumerate() {
            if imp.prefix != Some(p) {
                continue;
            }
            if imp.deferred && Some(name) == load_library {
                continue;
            }
            if let Some(b) = provides(program, imp, name) {
                used[i].extend(b.getter);
                used[i].extend(b.setter);
            }
        }
    };
    // `p` is a prefix at that place (neither shadowed nor declared).
    let is_prefix = |a: &dartforge_frontend::ast::Ast, pos: usize, p: SymbolId| {
        prefixes.contains(&p) && !declared.contains_key(&p) && !dartforge_types::anotacoes::sombreado(a, interner, pos, p)
    };

    for &u in &library.units {
        let unit = program.unit(u);
        if unit.role == UnitRole::Patch {
            continue;
        }
        unit_lookups(unit, interner, &is_prefix, &|p| prefixes.contains(&p) && !declared.contains_key(&p), &mut |b| match b {
            Lookup::Name(n) => mark(&mut used, n),
            Lookup::Prefixed(p, n) => mark_prefixed(&mut used, p, n),
            Lookup::PrefixInComment(p) => {
                prefix_in_comment.insert(p);
            }
        });
    }
    // `notifyExtensionUsed`.
    match bodies {
        Some(c) => {
            for &(l, x) in c.extensoes_usadas.iter() {
                if l != lib {
                    continue;
                }
                let Some(name) = program.extension(x).name else { continue };
                for (i, imp) in imps.iter().enumerate() {
                    if provides(program, imp, name).is_some_and(|b| b.getter == Some(Element::Extension(x))) {
                        used[i].insert(Element::Extension(x));
                    }
                }
            }
        }
        None => {
            for (i, imp) in imps.iter().enumerate() {
                for (k, b) in program.library(imp.target).exported.iter() {
                    if let Some(x @ Element::Extension(_)) = b.getter
                        && provides(program, imp, *k).is_some()
                    {
                        used[i].insert(x);
                    }
                }
            }
        }
    }

    let mut out = Vec::new();
    // `generateUnusedImportHints`.
    let mut unused_one = vec![false; imps.len()];
    for (i, imp) in imps.iter().enumerate() {
        if imp.prefix.is_some_and(|p| prefix_in_comment.contains(&p)) {
            continue;
        }
        let target = program.library(imp.target);
        // An explicit `dart:core` and a target that does not exist (reported elsewhere).
        if target.uri == "dart:core" || target.units.is_empty() {
            continue;
        }
        if used[i].is_empty() {
            unused_one[i] = true;
            out.push((imp.unit, Diagnostic::with_code(w::UNUSED_IMPORT, imp.uri, [imp.text.as_str()])));
        }
    }
    // `generateUnusedShownNameHints`: the shown name whose element was not
    // used from this import.
    for (i, imp) in imps.iter().enumerate() {
        let target = program.library(imp.target);
        if unused_one[i] || target.uri == "dart:core" || target.units.is_empty() {
            continue;
        }
        for comb in imp.combinators.iter() {
            let Combinator::Show(ns) = comb else { continue };
            for n in ns.iter() {
                let Some(b) = target.exported.get(&n.sym) else { continue };
                let Some(element) = b.getter.or(b.setter) else { continue };
                if !used[i].contains(&element) {
                    out.push((imp.unit, Diagnostic::with_code(w::UNUSED_SHOWN_NAME, n.span, [interner.resolve(n.sym)])));
                }
            }
        }
    }
    // `generateUnnecessaryImportHints`: a used import is unnecessary if another
    // used import with the same prefix provides all of its used elements and
    // strictly more (without those of a deprecated export).
    let acessados: Vec<HashSet<Element>> = imps
        .iter()
        .enumerate()
        .map(|(i, imp)| used[i].iter().copied().filter(|e| !from_deprecated_export(program, interner, imp.target, *e)).collect())
        .collect();
    for (i, first) in imps.iter().enumerate() {
        if unused_one[i] || program.library(first.target).uri == "dart:core" || program.library(first.target).units.is_empty() {
            continue;
        }
        for (j, segundo) in imps.iter().enumerate() {
            if i == j || unused_one[j] || program.library(segundo.target).units.is_empty() || first.prefix != segundo.prefix {
                continue;
            }
            if acessados[i].is_subset(&acessados[j]) && acessados[j].len() > acessados[i].len() {
                out.push((
                    first.unit,
                    Diagnostic::with_code(
                        dartforge_diagnostics::codes::hint::UNNECESSARY_IMPORT,
                        first.uri,
                        [first.text.as_str(), segundo.text.as_str()],
                    ),
                ));
                break;
            }
        }
    }
    out
}

/// `isFromDeprecatedExport`: the element is not declared in library `target`
/// and every `export` of it that provides it is `@deprecated`.
fn from_deprecated_export(program: &Program, interner: &Interner, target: LibraryId, e: Element) -> bool {
    let b = program.library(target);
    if b.declared.values().any(|x| x.getter == Some(e) || x.setter == Some(e)) {
        return false;
    }
    let mut any = false;
    for ex in b.exports.iter() {
        let provides = program.library(ex.library).exported.iter().any(|(name, x)| {
            (x.getter == Some(e) || x.setter == Some(e))
                && ex.combinators.iter().all(|c| match c {
                    Combinator::Show(ns) => ns.iter().any(|n| n.sym == *name),
                    Combinator::Hide(ns) => !ns.iter().any(|n| n.sym == *name),
                })
        });
        if !provides {
            continue;
        }
        any = true;
        let Some(dir) = program.unit(ex.unit).unit.directives.get(ex.directive) else { return false };
        if !dir.metadata.iter().any(|m| dartforge_types::anotacoes::e_deprecated(program, interner, ex.unit, m)) {
            return false;
        }
    }
    any
}

/// The loose names of refutable patterns (`case x:`, `if (v case x)`): the
/// constant patterns the parser keeps as an untyped variable.
fn pattern_constants(a: &dartforge_frontend::ast::Ast) -> Vec<dartforge_frontend::ast::Name> {
    use dartforge_frontend::ast::{CollectionElement, ListPatternElement, PatternId, StmtKind};
    let mut roots: Vec<PatternId> = Vec::new();
    for s in a.stmts.iter() {
        match &s.kind {
            StmtKind::Switch { cases, .. } => roots.extend(cases.iter().filter_map(|c| c.pattern)),
            StmtKind::If { case_pattern: Some(p), .. } => roots.push(*p),
            _ => {}
        }
    }
    fn from_collection(el: &CollectionElement, roots: &mut Vec<PatternId>) {
        match el {
            CollectionElement::If { case_pattern, then, else_, .. } => {
                roots.extend(case_pattern.iter().copied());
                from_collection(then, roots);
                if let Some(x) = else_ {
                    from_collection(x, roots);
                }
            }
            CollectionElement::For { body, .. } | CollectionElement::ForIn { body, .. } => from_collection(body, roots),
            _ => {}
        }
    }
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::Switch { cases, .. } => roots.extend(cases.iter().map(|c| c.pattern)),
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                for el in elements.iter() {
                    from_collection(el, &mut roots);
                }
            }
            _ => {}
        }
    }
    let mut v = Vec::new();
    while let Some(p) = roots.pop() {
        match &a.pattern(p).kind {
            PatternKind::Variable { name, final_: false, var_: false, ty: None } => v.push(*name),
            PatternKind::Or(l, r) | PatternKind::And(l, r) => roots.extend([*l, *r]),
            PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) | PatternKind::Cast { pattern: x, .. } => roots.push(*x),
            PatternKind::List { elements, .. } => roots.extend(elements.iter().filter_map(|e| match e {
                ListPatternElement::Pattern(x) | ListPatternElement::Rest(Some(x)) => Some(*x),
                ListPatternElement::Rest(None) => None,
            })),
            PatternKind::Map { entries, .. } => roots.extend(entries.iter().map(|e| e.value)),
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                // A field's `:x` is the variable, not a lookup.
                for f in fields.iter() {
                    let sp = a.pattern(f.pattern).span;
                    let shorthand = f.name.is_some_and(|n| n.span.start >= sp.start && n.span.end <= sp.end);
                    if !shorthand {
                        roots.push(f.pattern);
                    }
                }
            }
            _ => {}
        }
    }
    v
}

/// The documentation comments of the unit, each one with the position for the
/// lexical lookup of the references and the names of the documented
/// declaration's scope that the position does not cover (the parameters and
/// type parameters of the documented function, method or typedef).
fn doc_comments(
    a: &dartforge_frontend::ast::Ast,
    unit: &dartforge_frontend::ast::CompilationUnit,
    source: &str,
    comments: &dartforge_frontend::comentarios::Comentarios,
) -> Vec<(dartforge_diagnostics::Span, usize, Vec<SymbolId>)> {
    use dartforge_frontend::ast::{Annotation, DeclKind, MemberKind, Parameter, TypedefKind};
    // `_findComment(metadata, tokenAfterMetadata)`.
    let doc = |metadata: &[Annotation], start: usize| {
        let after = match metadata.last() {
            Some(m) => dartforge_frontend::fonte::pular_brancos(source.as_bytes(), m.span.end),
            None => start,
        };
        comments.dart_doc(source, after).or_else(|| metadata.iter().rev().find_map(|m| comments.dart_doc(source, m.span.start)))
    };
    // The whole comment: the `/**`, or the sequence of `///` that starts at
    // the found one.
    let whole = |d: dartforge_diagnostics::Span| {
        if source[d.start..d.end].starts_with("///") {
            let mut end = d.end;
            for c in comments.todos().iter().filter(|c| c.start > d.start) {
                let between = &source[end..c.start];
                if !between.trim().is_empty() || !source[c.start..c.end].starts_with("///") || between.matches('\n').count() > 1 {
                    break;
                }
                end = c.end;
            }
            dartforge_diagnostics::Span { start: d.start, end }
        } else {
            d
        }
    };
    let names_of = |ps: &[Parameter], tps: &[dartforge_frontend::ast::TypeParameter]| -> Vec<SymbolId> {
        ps.iter().filter_map(|p| p.name.map(|n| n.sym)).chain(tps.iter().map(|t| t.name.sym)).collect()
    };
    let mut v = Vec::new();
    for d in unit.directives.iter() {
        if let Some(x) = doc(&d.metadata, d.span.start) {
            v.push((whole(x), d.span.start, Vec::new()));
        }
    }
    for decl in a.decls.iter() {
        let Some(x) = doc(&decl.metadata, decl.span.start) else { continue };
        let (pos, extras) = match &decl.kind {
            DeclKind::Function(f) => {
                let g = a.function(*f);
                (decl.span.start, names_of(g.parameters.as_deref().unwrap_or(&[]), &g.type_params))
            }
            DeclKind::Typedef(t) => {
                let ps: &[Parameter] = match &t.kind {
                    TypedefKind::Legacy { parameters, .. } => parameters,
                    TypedefKind::Alias(_) => &[],
                };
                (decl.span.start, names_of(ps, &t.type_params))
            }
            // The type: inside it (the members and the type parameters).
            DeclKind::Class(c) => (c.name.span.end + 1, Vec::new()),
            DeclKind::Mixin(c) => (c.name.span.end + 1, Vec::new()),
            DeclKind::Enum(c) => (c.name.span.end + 1, Vec::new()),
            DeclKind::ExtensionType(c) => (c.name.span.end + 1, Vec::new()),
            DeclKind::Extension(c) => (c.name.map_or(decl.span.start, |n| n.span.end) + 1, c.type_params.iter().map(|t| t.name.sym).collect()),
            DeclKind::Variables(_) => (decl.span.start, Vec::new()),
        };
        v.push((whole(x), pos, extras));
        if let DeclKind::Enum(c) = &decl.kind {
            for k in c.constants.iter() {
                if let Some(y) = doc(&k.metadata, k.span.start) {
                    v.push((whole(y), k.span.start, Vec::new()));
                }
            }
        }
    }
    for m in a.members.iter() {
        let Some(x) = doc(&m.metadata, m.span.start) else { continue };
        let extras = match &m.kind {
            MemberKind::Method(f) => {
                let g = a.function(*f);
                names_of(g.parameters.as_deref().unwrap_or(&[]), &g.type_params)
            }
            MemberKind::Constructor(k) => names_of(&k.parameters, &[]),
            MemberKind::Field(_) => Vec::new(),
        };
        v.push((whole(x), m.span.start, extras));
    }
    v
}
