//! The element an annotation refers to by name, through the scopes with which
//! analyzer 3.6.2 resolves it (`analyzer/lib/src/dart/element/scope.dart`):
//! the locals declared before it in the enclosing block, the parameters and
//! the type parameters of the enclosing functions (`FormalParameterScope`,
//! `TypeParameterScope`), the declared members of the enclosing class, mixin,
//! enum, extension type or extension (`InstanceScope`: its accessors and
//! methods, not the inherited ones) with its type parameters, and finally the
//! unit and the library (with the import prefix).
//!
//! A name found in a local or member scope is not the top-level element of
//! the same name: the annotation is not the constant being looked for.
//! Written without compiling or running (2026-10-05).

use dartforge_diagnostics::Span;
use dartforge_elements::model::{Element, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, ExprKind, ForInTarget, ForInit, FunctionBody, Initializer, MemberKind, PatternId, PatternKind, StmtKind, TypeKind};
use dartforge_intern::{Interner, SymbolId};

/// The names declared by a pattern (its variables).
fn pattern_variables(a: &ast::Ast, p: PatternId, output: &mut Vec<ast::Name>) {
    match &a.pattern(p).kind {
        PatternKind::Variable { name, .. } => output.push(*name),
        PatternKind::Or(l, r) | PatternKind::And(l, r) => {
            pattern_variables(a, *l, output);
            pattern_variables(a, *r, output);
        }
        PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) | PatternKind::Cast { pattern: x, .. } => {
            pattern_variables(a, *x, output)
        }
        PatternKind::List { elements, .. } => {
            for e in elements.iter() {
                match e {
                    ast::ListPatternElement::Pattern(x) | ast::ListPatternElement::Rest(Some(x)) => pattern_variables(a, *x, output),
                    ast::ListPatternElement::Rest(None) => {}
                }
            }
        }
        PatternKind::Map { entries, .. } => {
            for e in entries.iter() {
                pattern_variables(a, e.value, output);
            }
        }
        PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
            for f in fields.iter() {
                pattern_variables(a, f.pattern, output);
            }
        }
        _ => {}
    }
}

/// The names of the variables a pattern declares. In a refutable pattern
/// (`case`, `if-case`), a bare name (without `var`, `final` or a type) is a
/// constant, not a variable.
fn declared_in_pattern(a: &ast::Ast, p: PatternId, refutable: bool, output: &mut Vec<ast::Name>) {
    let mut all: Vec<ast::Name> = Vec::new();
    pattern_variables(a, p, &mut all);
    if !refutable {
        output.extend(all);
        return;
    }
    // The variables with a keyword or a type.
    let mut stack = vec![p];
    while let Some(q) = stack.pop() {
        match &a.pattern(q).kind {
            PatternKind::Variable { name, final_, var_, ty } => {
                if *final_ || *var_ || ty.is_some() {
                    output.push(*name);
                }
            }
            PatternKind::Or(l, r) | PatternKind::And(l, r) => stack.extend([*l, *r]),
            PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) | PatternKind::Cast { pattern: x, .. } => stack.push(*x),
            PatternKind::List { elements, .. } => stack.extend(elements.iter().filter_map(|e| match e {
                ast::ListPatternElement::Pattern(x) | ast::ListPatternElement::Rest(Some(x)) => Some(*x),
                ast::ListPatternElement::Rest(None) => None,
            })),
            PatternKind::Map { entries, .. } => stack.extend(entries.iter().map(|e| e.value)),
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => stack.extend(fields.iter().map(|f| f.pattern)),
            _ => {}
        }
    }
}

/// The span of a collection element (of its expressions).
fn element_snippet(a: &ast::Ast, el: &ast::CollectionElement) -> Option<Span> {
    use ast::CollectionElement as E;
    let joins_with = |x: Option<Span>, y: Option<Span>| match (x, y) {
        (Some(p), Some(q)) => Some(Span { start: p.start.min(q.start), end: p.end.max(q.end) }),
        (p, q) => p.or(q),
    };
    match el {
        E::Expression(e) | E::NullAwareExpression(e) | E::Spread { value: e, .. } => Some(a.expr(*e).span),
        E::MapEntry { key, value, .. } => joins_with(Some(a.expr(*key).span), Some(a.expr(*value).span)),
        E::If { condition, then, else_, .. } => {
            let mut s = joins_with(Some(a.expr(*condition).span), element_snippet(a, then));
            if let Some(x) = else_ {
                s = joins_with(s, element_snippet(a, x));
            }
            s
        }
        E::For { body, .. } | E::ForIn { body, .. } => element_snippet(a, body),
    }
}

/// The local scopes of the collection elements: (span, declared names).
fn collection_scopes(a: &ast::Ast, el: &ast::CollectionElement, output: &mut Vec<(Span, Vec<ast::Name>)>) {
    use ast::CollectionElement as E;
    match el {
        E::If { case_pattern, then, else_, .. } => {
            if let Some(p) = case_pattern
                && let Some(t) = element_snippet(a, then)
            {
                let mut names = Vec::new();
                declared_in_pattern(a, *p, true, &mut names);
                output.push((Span { start: a.pattern(*p).span.start, end: t.end }, names));
            }
            collection_scopes(a, then, output);
            if let Some(x) = else_ {
                collection_scopes(a, x, output);
            }
        }
        E::For { init, condition, updates, body, .. } => {
            let mut names = Vec::new();
            let mut start: Option<usize> = None;
            match init {
                Some(ForInit::Variables(l)) => {
                    names.extend(l.variables.iter().map(|v| v.name));
                    start = l.variables.first().map(|v| v.name.span.start);
                }
                Some(ForInit::Pattern { pattern, .. }) => {
                    declared_in_pattern(a, *pattern, false, &mut names);
                    start = Some(a.pattern(*pattern).span.start);
                }
                _ => {}
            }
            let end = element_snippet(a, body).map(|s| s.end).or(condition.map(|c| a.expr(c).span.end)).or(updates.last().map(|u| a.expr(*u).span.end));
            if let (Some(i), Some(f)) = (start, end) {
                output.push((Span { start: i, end: f }, names));
            }
            collection_scopes(a, body, output);
        }
        E::ForIn { target, body, .. } => {
            let mut names = Vec::new();
            match target {
                ForInTarget::Declared { name, .. } => names.push(*name),
                ForInTarget::Pattern { pattern, .. } => declared_in_pattern(a, *pattern, false, &mut names),
                ForInTarget::Expression(_) => {}
            }
            if let Some(t) = element_snippet(a, body) {
                output.push((t, names));
            }
            collection_scopes(a, body, output);
        }
        _ => {}
    }
}

/// The name `n` is declared in a local or member scope that contains position
/// `pos` of the unit (the analyzer's lexical lookup would stop before the
/// library scope).
pub fn is_shadowed(a: &ast::Ast, interner: &Interner, pos: usize, n: SymbolId) -> bool {
    let in_ = |s: Span| s.start <= pos && pos < s.end;
    let declares = |names: &[ast::Name]| names.iter().any(|x| x.sym == n && x.span.start < pos);
    // The smallest block that contains the span `s` (the scope of a local
    // declaration); without a block, the span itself.
    let block_of = |s: Span| {
        a.stmts
            .iter()
            .filter(|x| matches!(x.kind, StmtKind::Block(_)) && x.span.start < s.start && s.end <= x.span.end)
            .map(|x| x.span)
            .min_by_key(|x| x.end - x.start)
            .unwrap_or(s)
    };
    // The locals declared before `pos`, in the scope that contains them.
    for s in a.stmts.iter() {
        let mut names: Vec<ast::Name> = Vec::new();
        // A block local is in scope in the whole block, also before its declaration
        // (a use before it is `REFERENCED_BEFORE_DECLARATION`, and a prefix of the
        // same name is shadowed: `PREFIX_SHADOWED_BY_LOCAL_DECLARATION`).
        let mut in_whole_block = false;
        let scope = match &s.kind {
            StmtKind::Variables(l) => {
                names.extend(l.variables.iter().map(|v| v.name));
                in_whole_block = true;
                block_of(s.span)
            }
            StmtKind::Function(f) => {
                names.extend(a.function(*f).name);
                in_whole_block = true;
                block_of(s.span)
            }
            StmtKind::PatternVariables { pattern, .. } => {
                declared_in_pattern(a, *pattern, false, &mut names);
                in_whole_block = true;
                block_of(s.span)
            }
            // The loop variables are in scope in the loop itself.
            StmtKind::For { init: Some(ForInit::Variables(l)), .. } => {
                names.extend(l.variables.iter().map(|v| v.name));
                s.span
            }
            StmtKind::For { init: Some(ForInit::Pattern { pattern, .. }), .. } => {
                declared_in_pattern(a, *pattern, false, &mut names);
                s.span
            }
            StmtKind::ForIn { target: ForInTarget::Declared { name, .. }, .. } => {
                names.push(*name);
                s.span
            }
            StmtKind::ForIn { target: ForInTarget::Pattern { pattern, .. }, .. } => {
                declared_in_pattern(a, *pattern, false, &mut names);
                s.span
            }
            // The `if-case` variables are in scope in the guard and in the `then`.
            StmtKind::If { case_pattern: Some(p), then, .. } => {
                declared_in_pattern(a, *p, true, &mut names);
                Span { start: a.pattern(*p).span.start, end: a.stmt(*then).span.end }
            }
            // Those of each case are in scope in the guard and in the case body.
            StmtKind::Switch { cases, .. } => {
                for c in cases.iter() {
                    let Some(p) = c.pattern else { continue };
                    let mut vs = Vec::new();
                    declared_in_pattern(a, p, true, &mut vs);
                    if in_(c.span) && declares(&vs) {
                        return true;
                    }
                }
                continue;
            }
            StmtKind::Try { catches, .. } => {
                for c in catches.iter() {
                    let the_body = a.stmt(c.body).span;
                    if in_(the_body) && c.exception.iter().chain(c.stack_trace.iter()).any(|x| x.sym == n) {
                        return true;
                    }
                }
                continue;
            }
            _ => continue,
        };
        if in_(scope) && (if in_whole_block { names.iter().any(|x| x.sym == n) } else { declares(&names) }) {
            return true;
        }
    }
    // The cases of a `switch` expression and the collection elements.
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::Switch { cases, .. } => {
                for c in cases.iter() {
                    let mut vs = Vec::new();
                    declared_in_pattern(a, c.pattern, true, &mut vs);
                    if in_(c.span) && declares(&vs) {
                        return true;
                    }
                }
            }
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                if !in_(e.span) {
                    continue;
                }
                let mut scopes = Vec::new();
                for el in elements.iter() {
                    collection_scopes(a, el, &mut scopes);
                }
                if scopes.iter().any(|(s, names)| in_(*s) && declares(names)) {
                    return true;
                }
            }
            _ => {}
        }
    }
    // The parameters (in the body) and the type parameters (in the whole
    // function) of the functions that contain `pos`.
    let common = |ps: &[ast::Parameter]| ps.iter().any(|p| !p.this_ && !p.super_ && p.name.is_some_and(|x| x.sym == n));
    for f in a.functions.iter() {
        if in_(f.span) && f.type_params.iter().any(|t| t.name.sym == n) {
            return true;
        }
        let the_body = match &f.body {
            FunctionBody::Block(s) => a.stmt(*s).span,
            FunctionBody::Expression(e) => a.expr(*e).span,
            _ => continue,
        };
        if in_(the_body) && f.parameters.as_deref().is_some_and(common) {
            return true;
        }
    }
    // The constructor: in the initializer list all parameters are in scope
    // (`this.x` and `super.x` too); in the body, the ordinary ones.
    for m in a.members.iter() {
        let MemberKind::Constructor(k) = &m.kind else { continue };
        let in_initializers = k.initializers.iter().any(|i| {
            in_(match i {
                Initializer::Field { span, .. } | Initializer::Super { span, .. } | Initializer::Redirect { span, .. } | Initializer::Assert { span, .. } => *span,
            })
        });
        if in_initializers && k.parameters.iter().any(|p| p.name.is_some_and(|x| x.sym == n)) {
            return true;
        }
        let the_body = match &k.body {
            FunctionBody::Block(s) => Some(a.stmt(*s).span),
            FunctionBody::Expression(e) => Some(a.expr(*e).span),
            _ => None,
        };
        if the_body.is_some_and(in_) && common(&k.parameters) {
            return true;
        }
    }
    // The type parameters of a `Function<T>(…)` type and of a function-typed
    // parameter `f<T>(…)`.
    fn parameter_types(ps: &[ast::Parameter], pos: usize, n: SymbolId) -> bool {
        ps.iter().any(|p| {
            (p.span.start <= pos && pos < p.span.end && p.function_type_params.iter().any(|t| t.name.sym == n))
                || p.function_parameters.as_deref().is_some_and(|fs| parameter_types(fs, pos, n))
        })
    }
    for t in a.types.iter() {
        if let TypeKind::Function { type_params, parameters, .. } = &t.kind
            && ((in_(t.span) && type_params.iter().any(|x| x.name.sym == n)) || parameter_types(parameters, pos, n))
        {
            return true;
        }
    }
    for f in a.functions.iter() {
        if f.parameters.as_deref().is_some_and(|ps| parameter_types(ps, pos, n)) {
            return true;
        }
    }
    // The declared members (and the type parameters) of the type that contains
    // `pos` after its name; the type parameters of the typedef.
    let values = interner.lookup("values");
    for d in a.decls.iter() {
        let (the_name, types, members, extras): (Option<ast::Name>, &[ast::TypeParameter], &[ast::MemberId], Vec<SymbolId>) = match &d.kind {
            DeclKind::Class(x) => (Some(x.name), &x.type_params[..], &x.members[..], Vec::new()),
            DeclKind::Mixin(x) => (Some(x.name), &x.type_params[..], &x.members[..], Vec::new()),
            // The enum's synthetic `values` is also in scope.
            DeclKind::Enum(x) => (Some(x.name), &x.type_params[..], &x.members[..], x.constants.iter().map(|c| c.name.sym).chain(values).collect()),
            DeclKind::Extension(x) => (x.name, &x.type_params[..], &x.members[..], Vec::new()),
            DeclKind::ExtensionType(x) => (Some(x.name), &x.type_params[..], &x.members[..], vec![x.representation_name.sym]),
            DeclKind::Typedef(x) => {
                if in_(d.span) && x.type_params.iter().any(|t| t.name.sym == n) {
                    return true;
                }
                continue;
            }
            _ => continue,
        };
        let after_name = the_name.map_or(d.span.start, |x| x.span.end);
        if !(pos > after_name && pos < d.span.end) {
            continue;
        }
        if types.iter().any(|t| t.name.sym == n) || extras.contains(&n) {
            return true;
        }
        for &mid in members {
            let declares = match &a.member(mid).kind {
                MemberKind::Method(f) => a.function(*f).name.is_some_and(|x| x.sym == n),
                MemberKind::Field(l) => l.variables.iter().any(|v| v.name.sym == n),
                MemberKind::Constructor(_) => false,
            };
            if declares {
                return true;
            }
        }
    }
    false
}

/// The top-level element of annotation `m`, written in unit `u`: the simple
/// name, the prefixed one (`p.x`, `p.C.ctor`) or the class one (`C.ctor`,
/// `C.x`, which returns the class). `None` when the name is from a local or
/// member scope, or does not resolve.
pub fn annotation_element(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation) -> Option<Element> {
    let a = &program.unit(u).ast;
    let pos = m.span.start;
    let first = m.name.first()?;
    if is_shadowed(a, interner, pos, first.sym) {
        return None;
    }
    let in_scope = program.lookup_in_unit(u, first.sym).and_then(|b| b.getter);
    match &m.name[..] {
        [_] => in_scope,
        // `p.x` with `p` an import prefix, or `C.x`.
        [p, x, ..] => match in_scope {
            Some(e) => Some(e),
            None => program.lookup_prefixed_in_unit(u, p.sym, x.sym).and_then(|b| b.getter),
        },
        [] => None,
    }
}

/// `ElementAnnotation.element`: the invoked constructor, the class referenced
/// without arguments, or the getter read (that of the top-level or static
/// variable, or the explicit one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvokedElement {
    Constructor(dartforge_elements::model::ClassId, dartforge_elements::model::FunctionElementId),
    Class(dartforge_elements::model::ClassId),
    Getter(dartforge_elements::model::FunctionElementId),
    Other,
}

/// The [`InvokedElement`] of annotation `m`, written in unit `u`.
pub fn invoked_element(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation) -> InvokedElement {
    use dartforge_elements::model::FunctionKind;
    let a = &program.unit(u).ast;
    let Some(first) = m.name.first() else { return InvokedElement::Other };
    if is_shadowed(a, interner, m.span.start, first.sym) {
        return InvokedElement::Other;
    }
    let (tgt, member) = match &m.name[..] {
        [c] => (program.lookup_in_unit(u, c.sym).and_then(|b| b.getter), None),
        [x, y] => match program.lookup_in_unit(u, x.sym).and_then(|b| b.getter) {
            Some(el @ Element::Class(_)) => (Some(el), Some(y.sym)),
            _ => (program.lookup_prefixed_in_unit(u, x.sym, y.sym).and_then(|b| b.getter), None),
        },
        [x, c, n] => (program.lookup_prefixed_in_unit(u, x.sym, c.sym).and_then(|b| b.getter), Some(n.sym)),
        _ => (None, None),
    };
    let getter = |f: dartforge_elements::model::FunctionElementId| {
        let e = program.function(f);
        let is_getter = match e.kind {
            FunctionKind::Getter => true,
            FunctionKind::ImplicitAccessor => e.variable.is_some_and(|v| program.variable(v).getter == Some(f)),
            _ => false,
        };
        if is_getter { InvokedElement::Getter(f) } else { InvokedElement::Other }
    };
    match (tgt, m.arguments.is_some(), member) {
        (Some(Element::Class(c)), true, _) => {
            let the_key = member.or_else(|| interner.lookup(""));
            match the_key.and_then(|k| program.class(c).constructors.get(&k)) {
                Some(&f) => InvokedElement::Constructor(c, f),
                None => InvokedElement::Other,
            }
        }
        (Some(Element::Class(c)), false, None) => InvokedElement::Class(c),
        (Some(Element::Class(c)), false, Some(n)) => match program.class(c).static_members.get(&n) {
            Some(&f) => getter(f),
            None => InvokedElement::Other,
        },
        (Some(Element::Variable(v)), _, None) => match program.variable(v).getter {
            Some(f) => InvokedElement::Getter(f),
            None => InvokedElement::Other,
        },
        (Some(Element::Function(g)), _, None) => getter(g),
        _ => InvokedElement::Other,
    }
}

/// The library name (`library a.b;`), like `LibraryElement.name`.
pub fn library_name(program: &Program, interner: &Interner, l: dartforge_elements::model::LibraryId) -> String {
    program.library(l).name.as_ref().map(|n| n.iter().map(|s| interner.resolve(*s)).collect::<Vec<_>>().join(".")).unwrap_or_default()
}

/// `_isTopGetter`: the annotation reads the getter `the_name` of a library named
/// `library` (`_isPackageMetaGetter` with `meta`).
pub fn is_getter_of(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation, library: &str, the_name: &str) -> bool {
    match invoked_element(program, interner, u, m) {
        InvokedElement::Getter(f) => {
            let e = program.function(f);
            interner.resolve(e.name) == the_name && library_name(program, interner, e.library) == library
        }
        _ => false,
    }
}

/// `_isConstructor`: the annotation invokes a constructor of class `class` of
/// a library named `library`.
pub fn is_constructor_of(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation, library: &str, class: &str) -> bool {
    match invoked_element(program, interner, u, m) {
        InvokedElement::Constructor(c, _) => {
            let k = program.class(c);
            interner.resolve(k.name) == class && library_name(program, interner, k.library) == library
        }
        _ => false,
    }
}

/// Annotation `m` (written in unit `u`), without arguments, is the top-level
/// variable `the_name` of a library whose URI satisfies `of_library`.
pub fn annotation_and_variable(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation, the_name: &str, of_library: &dyn Fn(&str) -> bool) -> bool {
    if m.arguments.is_some() || m.name.last().is_none_or(|n| interner.resolve(n.sym) != the_name) {
        return false;
    }
    match annotation_element(program, interner, u, m) {
        Some(Element::Variable(v)) => {
            let x = program.variable(v);
            interner.resolve(x.name) == the_name && of_library(&program.library(x.library).uri)
        }
        _ => false,
    }
}

/// `ElementAnnotation.isDeprecated`: the `deprecated` variable of `dart:core`
/// (without arguments), or a constructor of the `Deprecated` class of
/// `dart:core` (`@Deprecated('…')`, with arguments).
pub fn is_deprecated(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation) -> bool {
    match annotation_element(program, interner, u, m) {
        Some(Element::Variable(v)) => {
            let x = program.variable(v);
            m.arguments.is_none() && interner.resolve(x.name) == "deprecated" && program.library(x.library).uri == "dart:core"
        }
        Some(Element::Class(c)) => {
            let x = program.class(c);
            m.arguments.is_some() && interner.resolve(x.name) == "Deprecated" && program.library(x.library).uri == "dart:core"
        }
        _ => false,
    }
}

/// `ElementAnnotation.isDeprecated` without arguments: the `deprecated`
/// variable of `dart:core`.
pub fn is_core_deprecated(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation) -> bool {
    annotation_and_variable(program, interner, u, m, "deprecated", &|uri| uri == "dart:core")
}
