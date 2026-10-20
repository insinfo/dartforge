//! The ninth batch of lint rules that only look at the tree and the text
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), written directly from the 3.6.2
//! emitters (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//! `unnecessary_final`, `hash_and_equals`, `unnecessary_getters_setters`,
//! `recursive_getters` and `prefer_initializing_formals`.
//!
//! The four rules that the original decides by element use the unit's
//! semantics (without it, they do not report; `hash_and_equals` keeps the
//! members of the declaration itself):
//! - `unnecessary_getters_setters`: the synthetic accessor of the private
//!   field of the same class, the write type equal to the value's type, the
//!   parameter by element.
//! - `recursive_getters`: the identifier (or `this.x`) resolved to the getter
//!   itself, with the exceptions for a prefix, a `?.` target and a constant
//!   literal.
//! - `prefer_initializing_formals`: the target's canonical field and the
//!   parameter by element.
//! - `hash_and_equals`: the members of the whole class, with the augmentations.
//!
//! Written without compiling or running (2026-10-05).

use super::LintCode;
use super::codes_g as c;
use super::rules::LintReport;
use crate::Unit;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, AssignOp, Ast, DeclKind, ExprId, ExprKind, ForInTarget, ForInit, FunctionBody,
    FunctionId, FunctionKind, Initializer, ListPatternElement, MemberId, MemberKind, PatternId,
    PatternKind, StmtKind,
};
use dartforge_intern::{Interner, SymbolId};
use std::collections::HashMap;

/// The variables declared in a pattern (the `DeclaredVariablePattern`s).
pub(super) fn pattern_variables(a: &Ast, p: PatternId, out: &mut Vec<PatternId>) {
    match &a.pattern(p).kind {
        PatternKind::Variable { .. } => out.push(p),
        PatternKind::Wildcard { .. }
        | PatternKind::Constant(_)
        | PatternKind::Relational { .. } => {}
        PatternKind::Or(x, y) | PatternKind::And(x, y) => {
            pattern_variables(a, *x, out);
            pattern_variables(a, *y, out);
        }
        PatternKind::NullCheck(x)
        | PatternKind::NullAssert(x)
        | PatternKind::Parenthesized(x)
        | PatternKind::Cast { pattern: x, .. } => pattern_variables(a, *x, out),
        PatternKind::List { elements, .. } => {
            for el in elements.iter() {
                if let ListPatternElement::Pattern(x) | ListPatternElement::Rest(Some(x)) = el {
                    pattern_variables(a, *x, out);
                }
            }
        }
        PatternKind::Map { entries, .. } => {
            for en in entries.iter() {
                pattern_variables(a, en.value, out);
            }
        }
        PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
            for field in fields.iter() {
                pattern_variables(a, field.pattern, out);
            }
        }
    }
}

/// The place of the word `final` in `source[from..until]` (the last one: after
/// annotations and `late`/`required`/`covariant`).
fn final_word(source: &str, from: usize, until: usize) -> Option<Span> {
    let slice = source.get(from..until)?;
    let mut search_end = slice.len();
    while let Some(i) = slice[..search_end].rfind("final") {
        let before = slice[..i].bytes().next_back();
        let after = slice.as_bytes().get(i + 5).copied();
        let is_word_char = |c: u8| c == b'_' || c == b'$' || c.is_ascii_alphanumeric();
        if !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char) {
            return Some(Span {
                start: from + i,
                end: from + i + 5,
            });
        }
        search_end = i;
    }
    None
}

/// The body's single expression: that of `=> e`, or that of the block's single
/// statement (a `return e;` if `is_return`, otherwise an expression statement).
fn single_expression(a: &Ast, body: &FunctionBody, is_return: bool) -> Option<ExprId> {
    match body {
        FunctionBody::Expression(e) => Some(*e),
        FunctionBody::Block(s) => {
            let StmtKind::Block(stmts) = &a.stmt(*s).kind else {
                return None;
            };
            let [single] = &stmts[..] else { return None };
            match &a.stmt(*single).kind {
                StmtKind::Return(e) if is_return => *e,
                StmtKind::Expression(e) if !is_return => Some(*e),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(
    u: Unit<'_>,
    interner: &Interner,
    enabled: &dyn Fn(&str) -> bool,
    sem: Option<&super::Semantics<'_>>,
) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let a = u.ast;
    let source = u.source;
    let mut report = |code: &'static LintCode, span: Span, args: &[&str]| {
        out.push(LintReport {
            code,
            span,
            args: args.iter().map(|x| x.to_string()).collect(),
        });
    };
    let with_type = |has: bool| {
        if has {
            &c::UNNECESSARY_FINAL_WITH_TYPE
        } else {
            &c::UNNECESSARY_FINAL_WITHOUT_TYPE
        }
    };

    // `unnecessary_final`: parameters, local variables, the `for-in` variable
    // and pattern variables.
    if enabled("unnecessary_final") {
        // Every parameter list (the `FormalParameterList`), with the nested
        // ones and those of function types.
        fn collect_lists<'x>(list: &'x [ast::Parameter], out: &mut Vec<&'x [ast::Parameter]>) {
            out.push(list);
            for p in list {
                if let Some(inner) = &p.function_parameters {
                    collect_lists(inner, out);
                }
            }
        }
        let mut all_lists: Vec<&[ast::Parameter]> = Vec::new();
        for f in a.functions.iter() {
            if let Some(ps) = &f.parameters {
                collect_lists(ps, &mut all_lists);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                collect_lists(&k.parameters, &mut all_lists);
            }
        }
        for t in a.types.iter() {
            if let ast::TypeKind::Function { parameters, .. } = &t.kind {
                collect_lists(parameters, &mut all_lists);
            }
        }
        for d in a.decls.iter() {
            if let DeclKind::Typedef(x) = &d.kind
                && let ast::TypedefKind::Legacy { parameters, .. } = &x.kind
            {
                collect_lists(parameters, &mut all_lists);
            }
        }
        for list in all_lists {
            // A function parameter has no word (`getParameterDetails`).
            for p in list
                .iter()
                .filter(|p| p.final_ && p.function_parameters.is_none())
            {
                let until =
                    p.ty.map(|t| a.ty(t).span.start)
                        .or(p.name.map(|n| n.span.start))
                        .unwrap_or(p.span.end);
                if let Some(word) = final_word(source, p.span.start, until) {
                    report(with_type(p.ty.is_some()), word, &[]);
                }
            }
        }
        for s in a.stmts.iter() {
            match &s.kind {
                StmtKind::Variables(l) if l.final_ => {
                    let until =
                        l.ty.map(|t| a.ty(t).span.start)
                            .or(l.variables.first().map(|v| v.name.span.start))
                            .unwrap_or(s.span.end);
                    if let Some(word) = final_word(source, s.span.start, until) {
                        report(with_type(l.ty.is_some()), word, &[]);
                    }
                }
                StmtKind::ForIn {
                    target:
                        ForInTarget::Declared {
                            final_: true,
                            ty,
                            name,
                            ..
                        },
                    ..
                } => {
                    let until = ty.map_or(name.span.start, |t| a.ty(t).span.start);
                    if let Some(word) = final_word(source, s.span.start, until) {
                        report(with_type(ty.is_some()), word, &[]);
                    }
                }
                StmtKind::ForIn {
                    target:
                        ForInTarget::Pattern {
                            final_: true,
                            pattern,
                        },
                    ..
                } => {
                    if let Some(word) =
                        final_word(source, s.span.start, a.pattern(*pattern).span.start)
                    {
                        report(&c::UNNECESSARY_FINAL_WITHOUT_TYPE, word, &[]);
                    }
                }
                // `final (a, b) = …`: each pattern variable reports at the
                // declaration's word (the driver drops the duplicates).
                StmtKind::PatternVariables {
                    final_: true,
                    pattern,
                    ..
                }
                | StmtKind::For {
                    init:
                        Some(ForInit::Pattern {
                            final_: true,
                            pattern,
                            ..
                        }),
                    ..
                } => {
                    let mut variables = Vec::new();
                    pattern_variables(a, *pattern, &mut variables);
                    let without_word = variables.iter().any(|v| {
                        !matches!(
                            a.pattern(*v).kind,
                            PatternKind::Variable { final_: true, .. }
                                | PatternKind::Variable { var_: true, .. }
                        )
                    });
                    if without_word
                        && let Some(word) =
                            final_word(source, s.span.start, a.pattern(*pattern).span.start)
                    {
                        report(&c::UNNECESSARY_FINAL_WITH_TYPE, word, &[]);
                    }
                }
                _ => {}
            }
        }
        // `case final x`, `final int x` in a pattern: the word is the pattern's.
        for p in a.patterns.iter() {
            if let PatternKind::Variable {
                final_: true,
                name,
                ty,
                ..
            } = &p.kind
            {
                let until = ty.map_or(name.span.start, |t| a.ty(t).span.start);
                if let Some(word) = final_word(source, p.span.start, until) {
                    report(&c::UNNECESSARY_FINAL_WITH_TYPE, word, &[]);
                }
            }
        }
    }

    // The members of each class, enum and extension type.
    struct Decl<'x> {
        is_class: bool,
        is_extension_type: bool,
        augment: bool,
        members: &'x [MemberId],
    }
    let decls: Vec<Decl<'_>> = a
        .decls
        .iter()
        .filter_map(|d| {
            let (members, is_class, is_extension_type): (&[MemberId], bool, bool) = match &d.kind {
                DeclKind::Class(x) => (&x.members[..], true, false),
                DeclKind::Enum(x) => (&x.members[..], false, false),
                DeclKind::ExtensionType(x) => (&x.members[..], false, true),
                _ => return None,
            };
            Some(Decl {
                is_class,
                is_extension_type,
                augment: d.augment,
                members,
            })
        })
        .collect();

    // `hash_and_equals`: a class that declares `==` without `hashCode` (field
    // or method, in the whole augmented class: `allFields`/`allMethods`), or
    // the reverse. Without the semantics, the members are this declaration's.
    if enabled("hash_and_equals") {
        let symbol = |s: &str| interner.lookup(s);
        for d in decls.iter().filter(|d| d.is_class) {
            let mut equals: Option<Span> = None;
            let mut hash: Option<Span> = None;
            for &m in d.members {
                match &a.member(m).kind {
                    MemberKind::Method(f) => {
                        let Some(n) = a.function(*f).name else {
                            continue;
                        };
                        match interner.resolve(n.sym) {
                            "==" => equals = Some(n.span),
                            "hashCode" => hash = Some(n.span),
                            _ => {}
                        }
                    }
                    MemberKind::Field(l) => {
                        if let Some(v) = l
                            .variables
                            .iter()
                            .find(|v| interner.resolve(v.name.sym) == "hashCode")
                        {
                            hash = Some(v.name.span);
                        }
                    }
                    MemberKind::Constructor(_) => {}
                }
            }
            // The members of the whole class (with the augmentations).
            let class_id = sem.and_then(|s| {
                d.members
                    .first()
                    .and_then(|&m| super::member_class(s, m))
                    .map(|c| (s, c))
            });
            let has_method = |name: &str| match class_id {
                Some((s, c)) => symbol(name)
                    .and_then(|x| s.program.class(c).instance_members.get(&x))
                    .is_some_and(|f| {
                        matches!(
                            s.program.function(*f).kind,
                            dartforge_elements::model::FunctionKind::Function
                                | dartforge_elements::model::FunctionKind::Operator
                        )
                    }),
                None => match name {
                    "==" => equals.is_some(),
                    _ => false,
                },
            };
            let has_field = |name: &str| match class_id {
                Some((s, c)) => {
                    let x = s.program.class(c);
                    x.fields
                        .iter()
                        .any(|v| interner.resolve(s.program.variable(*v).name) == name)
                        || symbol(name)
                            .and_then(|y| x.instance_members.get(&y))
                            .is_some_and(|f| {
                                matches!(
                                    s.program.function(*f).kind,
                                    dartforge_elements::model::FunctionKind::Getter
                                        | dartforge_elements::model::FunctionKind::Setter
                                )
                            })
                }
                None => hash.is_some(),
            };
            match (equals, hash) {
                (None, Some(h)) if !has_method("==") => {
                    report(&c::HASH_AND_EQUALS, h, &["==", "hashCode"])
                }
                (Some(i), None) if !has_field("hashCode") && !has_method("hashCode") => {
                    report(&c::HASH_AND_EQUALS, i, &["hashCode", "=="])
                }
                _ => {}
            }
        }
    }
    // `unnecessary_getters_setters`: the `get x`/`set x` pair of a class or
    // extension type (outside an augmentation) without annotations, with the
    // getter returning the synthetic accessor of a private field of the same
    // declaration and the setter assigning (`=`) its single parameter to a
    // synthetic accessor, with the write type equal to the parameter's. Needs
    // the unit's semantics.
    if enabled("unnecessary_getters_setters")
        && let Some(s) = sem
    {
        use dartforge_types::resolved::{MemberRef, Resolved};
        // The field (with a synthetic accessor) that the expression reads or writes.
        let synthetic_field = |e: ExprId| -> Option<dartforge_elements::model::VariableId> {
            match s.body.get_resolved(e)? {
                Resolved::Member {
                    member: MemberRef::Variable(v),
                    ..
                } => Some(*v),
                Resolved::Member {
                    member: MemberRef::Function(f),
                    ..
                } => s.program.function(*f).variable,
                _ => None,
            }
        };
        for d in decls
            .iter()
            .filter(|d| (d.is_class || d.is_extension_type) && !d.augment)
        {
            let Some(class_id) = d.members.first().and_then(|&m| super::member_class(s, m)) else {
                continue;
            };
            let mut getters: HashMap<SymbolId, MemberId> = HashMap::new();
            let mut setters: HashMap<SymbolId, MemberId> = HashMap::new();
            for &m in d.members {
                if let MemberKind::Method(f) = &a.member(m).kind
                    && let Some(n) = a.function(*f).name
                {
                    match a.function(*f).kind {
                        FunctionKind::Getter => {
                            getters.insert(n.sym, m);
                        }
                        FunctionKind::Setter => {
                            setters.insert(n.sym, m);
                        }
                        _ => {}
                    }
                }
            }
            let mut findings: Vec<Span> = Vec::new();
            for (name, &g) in getters.iter() {
                let Some(&st) = setters.get(name) else {
                    continue;
                };
                let (MemberKind::Method(fg), MemberKind::Method(fs)) =
                    (&a.member(g).kind, &a.member(st).kind)
                else {
                    continue;
                };
                if !a.member(g).metadata.is_empty() || !a.member(st).metadata.is_empty() {
                    continue;
                }
                let (getter, setter) = (a.function(*fg), a.function(*fs));
                // `_checkForSimpleGetter`.
                let returns_field = single_expression(a, &getter.body, true).is_some_and(|e| {
                    matches!(a.expr(e).kind, ExprKind::Identifier(_))
                        && synthetic_field(e).is_some_and(|v| {
                            let x = s.program.variable(v);
                            x.class == Some(class_id) && interner.resolve(x.name).starts_with('_')
                        })
                });
                // `_checkForSimpleSetter`.
                let assigns_field = single_expression(a, &setter.body, false).is_some_and(|e| {
                    let ExprKind::Assign {
                        op: AssignOp::Assign,
                        target,
                        value,
                    } = &a.expr(e).kind
                    else {
                        return false;
                    };
                    if !matches!(a.expr(*target).kind, ExprKind::Identifier(_))
                        || !matches!(a.expr(*value).kind, ExprKind::Identifier(_))
                    {
                        return false;
                    }
                    let Some(v) = synthetic_field(*target) else {
                        return false;
                    };
                    let Some([param]) = setter.parameters.as_deref() else {
                        return false;
                    };
                    let same_type = match (super::variable_type(s, v), s.body.get_type(*value)) {
                        (Some(x), Some(y)) => s.table.canonical(x) == s.table.canonical(y),
                        _ => false,
                    };
                    same_type
                        && param
                            .name
                            .is_some_and(|n| s.body.local_declaration(*value) == Some(n.span.start))
                });
                if returns_field
                    && assigns_field
                    && let Some(n) = getter.name
                {
                    findings.push(n.span);
                }
            }
            findings.sort_by_key(|x| x.start);
            for x in findings {
                report(&c::UNNECESSARY_GETTERS_SETTERS, x, &[]);
            }
        }
    }
    // `prefer_initializing_formals`: in the body, the assignment statement
    // whose target is a public, non-synthetic field of the constructor's
    // class, and whose value is the parameter with the same name; in the
    // initializer list, `x = x` with `x` a parameter and a public field. (The
    // original's "used more than once" condition is always true: the same
    // name is already required.) Needs the unit's semantics.
    if enabled("prefer_initializing_formals")
        && let Some(s) = sem
    {
        for d in decls.iter() {
            for &m in d.members {
                let MemberKind::Constructor(k) = &a.member(m).kind else {
                    continue;
                };
                if k.factory {
                    continue;
                }
                let Some(class_id) = super::member_class(s, m) else {
                    continue;
                };
                // The constructor parameter that the expression reads.
                let param = |e: ExprId| -> Option<&ast::Parameter> {
                    let decl = s.body.local_declaration(e)?;
                    k.parameters
                        .iter()
                        .find(|p| p.name.is_some_and(|n| n.span.start == decl))
                };
                if let FunctionBody::Block(body) = &k.body
                    && let StmtKind::Block(stmts) = &a.stmt(*body).kind
                {
                    for &st in stmts.iter() {
                        let StmtKind::Expression(e) = &a.stmt(st).kind else {
                            continue;
                        };
                        let ExprKind::Assign { target, value, .. } = &a.expr(*e).kind else {
                            continue;
                        };
                        let Some(super::Canonical::Variable(v)) =
                            super::canonical(s, interner, *target)
                        else {
                            continue;
                        };
                        let field = s.program.variable(v);
                        let Some(p) = param(*value)
                            .filter(|_| matches!(a.expr(*value).kind, ExprKind::Identifier(_)))
                        else {
                            continue;
                        };
                        let Some(n) = p.name else { continue };
                        let name = interner.resolve(field.name);
                        if field.class == Some(class_id)
                            && !name.starts_with('_')
                            && field.name == n.sym
                        {
                            report(
                                &c::PREFER_INITIALIZING_FORMALS,
                                a.expr(*e).span,
                                &[interner.resolve(n.sym)],
                            );
                        }
                    }
                }
                for i in k.initializers.iter() {
                    if let Initializer::Field {
                        span, name, value, ..
                    } = i
                        && matches!(&a.expr(*value).kind, ExprKind::Identifier(v) if v.sym == name.sym)
                        && param(*value).is_some()
                        && super::class_field(s, m, name.sym).is_some()
                        && !interner.resolve(name.sym).starts_with('_')
                    {
                        report(
                            &c::PREFER_INITIALIZING_FORMALS,
                            *span,
                            &[interner.resolve(name.sym)],
                        );
                    }
                }
            }
        }
    }
    // `recursive_getters`: in a top-level or member getter, the identifier (or
    // `this.x`) whose element is the getter itself, outside a `const` literal,
    // that is not the prefix of `x.y` nor the target of `x?.y` (the target of
    // a call `x.f()` counts). Needs the unit's semantics.
    if enabled("recursive_getters")
        && let Some(s) = sem
    {
        use dartforge_types::resolved::{MemberRef, Resolved};
        let old_version = !super::version_at_least(s, 3, 0);
        let parents = dartforge_frontend::parents::Parents::new(a, u.unit, u.source, old_version);
        // The getters (without a parameter list) and each one's element.
        let mut getters: Vec<(Span, dartforge_elements::model::FunctionElementId, SymbolId)> =
            Vec::new();
        for (i, f) in a.functions.iter().enumerate() {
            if f.parameters.is_some() || f.kind != FunctionKind::Getter {
                continue;
            }
            let Some(n) = f.name else { continue };
            let Some(e) = super::function_element(s, FunctionId(i as u32)) else {
                continue;
            };
            let body = match &f.body {
                FunctionBody::Block(b) => a.stmt(*b).span,
                FunctionBody::Expression(x) => a.expr(*x).span,
                _ => continue,
            };
            getters.push((body, e, n.sym));
        }
        let is_getter_element =
            |e: ExprId, target: dartforge_elements::model::FunctionElementId| match s
                .body
                .get_resolved(e)
            {
                Some(Resolved::Member {
                    member: MemberRef::Function(f),
                    ..
                })
                | Some(Resolved::Element(dartforge_elements::model::Element::Function(f))) => {
                    *f == target
                }
                _ => false,
            };
        // The node is inside a constant list, set or map literal (the visitor
        // does not descend into it).
        let in_const_literal = |e: ExprId, bound: Span| {
            let mut current = parents.parent(e);
            while let dartforge_frontend::parents::Parent::Expr(p) = current {
                let n = a.expr(p);
                if n.span.start < bound.start || n.span.end > bound.end {
                    break;
                }
                let is_const = match &n.kind {
                    ExprKind::List { const_, .. } | ExprKind::SetOrMap { const_, .. } => {
                        *const_ || parents.in_constant_context(a, p)
                    }
                    _ => false,
                };
                if is_const {
                    return true;
                }
                current = parents.parent(p);
            }
            false
        };
        let mut findings: Vec<(Span, SymbolId)> = Vec::new();
        for (k, ex) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            let Some(&(body, element, getter_name)) = getters
                .iter()
                .find(|(c, _, _)| c.start <= ex.span.start && ex.span.end <= c.end)
            else {
                continue;
            };
            let parent = match parents.parent(id) {
                dartforge_frontend::parents::Parent::Expr(p) => Some(p),
                _ => None,
            };
            match &ex.kind {
                ExprKind::Identifier(n) => {
                    if !is_getter_element(id, element) || in_const_literal(id, body) {
                        continue;
                    }
                    if let Some(p) = parent {
                        match &a.expr(p).kind {
                            // Target of an assignment or of `++`/`--`: no
                            // `staticElement`.
                            ExprKind::Assign { target, .. } if *target == id => continue,
                            ExprKind::Unary {
                                op:
                                    dartforge_frontend::ast::UnaryOp::PrefixInc
                                    | dartforge_frontend::ast::UnaryOp::PrefixDec
                                    | dartforge_frontend::ast::UnaryOp::PostfixInc
                                    | dartforge_frontend::ast::UnaryOp::PostfixDec,
                                ..
                            } => continue,
                            // Prefix of `x.y`/target of `x?.y` (not of `x.f()`).
                            ExprKind::Property { target, .. } if *target == id => {
                                let call_target = matches!(parents.parent(p), dartforge_frontend::parents::Parent::Expr(c)
                                    if matches!(&a.expr(c).kind, ExprKind::Call { target, .. } if *target == p));
                                if !call_target {
                                    continue;
                                }
                            }
                            _ => {}
                        }
                    }
                    findings.push((n.span, getter_name));
                }
                // `this.x`.
                ExprKind::Property { target, name, .. }
                    if matches!(a.expr(*target).kind, ExprKind::This)
                        && is_getter_element(id, element)
                        && !in_const_literal(id, body)
                        && !matches!(parent.map(|p| &a.expr(p).kind), Some(ExprKind::Assign { target, .. }) if *target == id) =>
                {
                    findings.push((name.span, getter_name));
                }
                _ => {}
            }
        }
        for (x, name) in findings {
            report(&c::RECURSIVE_GETTERS, x, &[interner.resolve(name)]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_code(rule: &str, source: &str) -> Vec<(&'static str, String)> {
        let mut names = Interner::new();
        let p = dartforge_frontend::parser::parse(source, &mut names);
        let u = Unit {
            ast: &p.ast,
            unit: &p.unit,
            source,
        };
        let mut reports = run(u, &names, &|r| r == rule, None);
        reports.sort_by_key(|r| (r.span.start, r.span.end));
        reports.dedup_by_key(|r| (r.span.start, r.span.end));
        reports
            .into_iter()
            .map(|r| (r.code.unique, source[r.span.start..r.span.end].to_string()))
            .collect()
    }

    fn only(rule: &str, source: &str) -> Vec<String> {
        with_code(rule, source).into_iter().map(|x| x.1).collect()
    }

    #[test]
    fn unnecessary_final() {
        let source = "void f(final int a, final b, List<int> l) {\n  final x = 1;\n  final int y = 2;\n  var z = 3;\n  for (final e in l) {}\n  final (p, q) = (1, 2);\n}\nclass A {\n  final int c = 0;\n}\n";
        let v = with_code("unnecessary_final", source);
        let codes: Vec<&str> = v.iter().map(|x| x.0).collect();
        assert_eq!(
            codes,
            vec![
                "unnecessary_final_with_type",
                "unnecessary_final_without_type",
                "unnecessary_final_without_type",
                "unnecessary_final_with_type",
                "unnecessary_final_without_type",
                "unnecessary_final_with_type",
            ]
        );
        assert!(v.iter().all(|x| x.1 == "final"));
    }

    #[test]
    fn equality_and_hash() {
        assert_eq!(
            only(
                "hash_and_equals",
                "class A {\n  @override\n  bool operator ==(Object o) => true;\n}\nclass B {\n  @override\n  int get hashCode => 0;\n}\nclass C {\n  @override\n  bool operator ==(Object o) => true;\n  @override\n  final int hashCode = 0;\n}\n"
            ),
            vec!["==".to_string(), "hashCode".to_string()]
        );
    }

    #[test]
    fn getters_and_setters() {
        let source = "class A {\n  int _x = 0;\n  int get x => _x;\n  set x(int v) {\n    _x = v;\n  }\n  int _y = 0;\n  int get y => _y;\n  set y(int v) => _y = v + 1;\n}\n";
        // The accessor and the types need the unit's semantics.
        assert!(only("unnecessary_getters_setters", source).is_empty());
        let source = "class A {\n  int get a => a;\n  int get b => this.b + 1;\n  int get c {\n    final c = 1;\n    return c;\n  }\n  int get d => e;\n  int get e => 0;\n}\n";
        assert!(only("recursive_getters", source).is_empty());
    }

    #[test]
    fn initializing_formals() {
        let source = "class A {\n  int x;\n  int y;\n  int _z;\n  A(int x, int y, int z)\n      : y = y,\n        _z = z,\n        x = 0 {\n    this.x = x;\n  }\n  factory A.f(int x) => A(x, 0, 0);\n}\n";
        // The field and the parameter by element need the unit's semantics.
        assert!(only("prefer_initializing_formals", source).is_empty());
    }
}
