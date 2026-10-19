//! The tenth batch of lint rules that only look at the tree and the text
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), written directly from the 3.6.2
//! emitters (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//! `prefer_inlined_adds`, `prefer_spread_collections`,
//! `avoid_field_initializers_in_const_classes`,
//! `avoid_classes_with_only_static_members` and `prefer_final_in_for_each`.
//!
//! With the same data as the original (those that need the element only
//! report with the unit's semantics):
//! - `avoid_classes_with_only_static_members`: the inherited interface (class
//!   members that are not from `Object`; mixin ones do not count), the
//!   constructors, methods and fields of the whole class; without the
//!   semantics, only a class without `extends`, `with` or `implements`.
//! - `prefer_final_in_for_each`: the mutation by element
//!   (`super::is_mutated`), only inside a function body.
//! - `avoid_field_initializers_in_const_classes`: the use of a parameter by
//!   element and the constructors of the whole class.
//! - `prefer_spread_collections`: the target list outside a constant context.
//!
//! Written without compiling or running (2026-10-05).

use super::codes_g as c;
use super::rules::LintReport;
use super::LintCode;
use crate::Unit;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    self, Ast, CollectionElement, DeclKind, ExprKind, ForInTarget, FunctionKind, Initializer, ListPatternElement, MemberId, MemberKind,
    ParameterKind, PatternId, PatternKind, StmtKind,
};
use dartforge_intern::Interner;

/// The collection `for-in`s with a declared variable or a pattern: the target
/// and the end of the iterable (where the region in which the variable lives
/// starts).
fn collection_for_ins<'x>(a: &Ast, el: &'x CollectionElement, out: &mut Vec<(&'x ForInTarget, usize)>) {
    match el {
        CollectionElement::ForIn { target, iterable, body, .. } => {
            out.push((target, a.expr(*iterable).span.end));
            collection_for_ins(a, body, out);
        }
        CollectionElement::For { body, .. } => collection_for_ins(a, body, out),
        CollectionElement::If { then, else_, .. } => {
            collection_for_ins(a, then, out);
            if let Some(x) = else_ {
                collection_for_ins(a, x, out);
            }
        }
        _ => {}
    }
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unit<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let a = u.ast;
    let mut report = |code: &'static LintCode, span: Span, args: &[&str]| {
        out.push(LintReport { code, span, args: args.iter().map(|x| x.to_string()).collect() });
    };

    // `prefer_inlined_adds` and `prefer_spread_collections`: the first
    // section of a cascade on a list literal.
    let (inline_adds, spread) = (enabled("prefer_inlined_adds"), enabled("prefer_spread_collections"));
    if inline_adds || spread {
        // `target.inConstantContext` (only for `prefer_spread_collections`).
        let parents = dartforge_frontend::parents::Parents::new(a, u.unit, u.source, sem.is_some_and(|s| !super::version_at_least(s, 3, 0)));
        for e in a.exprs.iter() {
            let ExprKind::Cascade { target, sections, .. } = &e.kind else { continue };
            if !matches!(a.expr(*target).kind, ExprKind::List { .. }) {
                continue;
            }
            let Some(&first) = sections.first() else { continue };
            let ExprKind::Call { target: callee, arguments } = &a.expr(first).kind else { continue };
            let ExprKind::Property { target: receiver, name, .. } = &a.expr(*callee).kind else { continue };
            if !matches!(a.expr(*receiver).kind, ExprKind::CascadeTarget) {
                continue;
            }
            let [argument] = &arguments.args[..] else { continue };
            let is_list = matches!(a.expr(argument.value).kind, ExprKind::List { .. });
            match interner.resolve(name.sym) {
                "add" if inline_adds => report(&c::PREFER_INLINED_ADDS_SINGLE, name.span, &[]),
                "addAll" if is_list && inline_adds => report(&c::PREFER_INLINED_ADDS_MULTIPLE, name.span, &[]),
                "addAll" if !is_list && spread && !parents.in_constant_context(a, *target) => report(&c::PREFER_SPREAD_COLLECTIONS, name.span, &[]),
                _ => {}
            }
        }
    }

    // `avoid_field_initializers_in_const_classes`.
    if enabled("avoid_field_initializers_in_const_classes") {
        for d in a.decls.iter() {
            let DeclKind::Class(x) = &d.kind else { continue };
            let constructors: Vec<(MemberId, &ast::Constructor)> = x
                .members
                .iter()
                .filter_map(|&m| match &a.member(m).kind {
                    MemberKind::Constructor(k) => Some((m, k)),
                    _ => None,
                })
                .collect();
            // The constructors of the whole class (`allConstructors`, with the
            // augmentations): through the semantics; without it, this declaration's.
            let class_id = sem.and_then(|s| x.members.first().and_then(|&m| super::member_class(s, m)).map(|c| (s, c)));
            let (count, any_const) = match class_id {
                Some((s, c)) => {
                    let k = &s.program.class(c).constructors;
                    (k.len(), k.values().any(|f| s.program.function(*f).const_))
                }
                None => (constructors.len(), constructors.iter().any(|(_, k)| k.const_)),
            };
            // The `final` instance field with an initializer, in a class with
            // some `const` constructor.
            if any_const {
                for &m in x.members.iter() {
                    let member = a.member(m);
                    if let MemberKind::Field(l) = &member.kind
                        && !member.augment
                        && !l.static_
                        && l.final_
                    {
                        for v in l.variables.iter() {
                            if let Some(i) = v.initializer {
                                report(&c::AVOID_FIELD_INITIALIZERS_IN_CONST_CLASSES, Span { start: v.name.span.start, end: a.expr(i).span.end }, &[]);
                            }
                        }
                    }
                }
            }
            // The field initializer of the `const` constructor of a class with
            // a single constructor that reads no parameter (by element: needs
            // the unit's semantics).
            if let Some(s) = sem
                && count == 1
            {
                for (_, k) in constructors.iter().filter(|(_, k)| k.const_) {
                    let param_starts: Vec<usize> = k.parameters.iter().filter_map(|p| p.name).map(|n| n.span.start).collect();
                    for i in k.initializers.iter() {
                        if let Initializer::Field { span, value, .. } = i {
                            let region = a.expr(*value).span;
                            let uses = a.exprs.iter().enumerate().any(|(j, e)| {
                                matches!(e.kind, ExprKind::Identifier(_))
                                    && region.start <= e.span.start
                                    && e.span.end <= region.end
                                    && s.body.local_declaration(ast::ExprId(j as u32)).is_some_and(|dd| param_starts.contains(&dd))
                            });
                            if !uses {
                                report(&c::AVOID_FIELD_INITIALIZERS_IN_CONST_CLASSES, *span, &[]);
                            }
                        }
                    }
                }
            }
        }
    }
    // `avoid_classes_with_only_static_members`: a class (not `sealed`, outside
    // an augmentation) whose interface has no class member that is not from
    // `Object` (mixin members do not count), without a constructor other than
    // the default one, with only static methods, and with some method or
    // non-`const` field (accessors count as a synthetic field). Through the
    // semantics, the inherited interface and the members with the
    // augmentations; without it, only a class without `extends`, `with` or
    // `implements`.
    if enabled("avoid_classes_with_only_static_members") {
        for d in a.decls.iter().filter(|d| !d.augment) {
            let DeclKind::Class(x) = &d.kind else { continue };
            if x.modifiers.sealed {
                continue;
            }
            let class_id = sem.and_then(|s| x.members.first().and_then(|&m| super::member_class(s, m)).map(|c| (s, c)));
            let reported = match class_id {
                Some((s, c)) => {
                    use dartforge_elements::model::{ClassKind, FunctionKind as FnKind};
                    let p = s.program;
                    // The interface: the class itself and its supertypes.
                    let mut types: Vec<dartforge_elements::model::ClassId> = vec![c];
                    if let Some(h) = s.outline.hierarchy.get(c) {
                        types.extend(h.supertypes.keys().copied());
                    }
                    let inherits_class_member = types.iter().any(|&k| {
                        Some(k) != s.core.object_class
                            && matches!(p.class(k).kind, ClassKind::Class | ClassKind::MixinApplication)
                            && p.class(k).instance_members.values().any(|f| {
                                let g = p.function(*f);
                                // The member is declared in the class (it does not come from a mixin).
                                g.class == Some(k)
                            })
                    });
                    let e = p.class(c);
                    let own_ctor = e.constructors.iter().any(|(name, f)| {
                        let unnamed = interner.resolve(*name).is_empty();
                        let required = s.outline.functions.get(f.0 as usize).is_some_and(|dd| dd.parameters.iter().any(|q| q.kind == ParameterKind::Required || q.required));
                        !(unnamed && !required)
                    });
                    let methods: Vec<_> = e
                        .instance_members
                        .values()
                        .chain(e.static_members.values())
                        .filter(|f| matches!(p.function(**f).kind, FnKind::Function | FnKind::Operator))
                        .collect();
                    let all_static = methods.iter().all(|f| p.function(**f).static_);
                    let accessor = e.instance_members.values().chain(e.static_members.values()).any(|f| matches!(p.function(*f).kind, FnKind::Getter | FnKind::Setter));
                    let non_const_field = e.fields.iter().any(|v| !p.variable(*v).const_) || accessor;
                    !inherits_class_member && !own_ctor && all_static && (!methods.is_empty() || non_const_field)
                }
                None => {
                    if x.mixin_application || x.extends.is_some() || !x.with.is_empty() || !x.implements.is_empty() {
                        continue;
                    }
                    let mut has_instance = false;
                    let mut own_constructor = false;
                    let mut static_methods = false;
                    let mut non_const_field = false;
                    for &m in x.members.iter() {
                        match &a.member(m).kind {
                            MemberKind::Field(l) => {
                                has_instance |= !l.static_;
                                non_const_field |= !l.const_;
                            }
                            MemberKind::Method(f) => {
                                let f = a.function(*f);
                                has_instance |= !f.static_;
                                match f.kind {
                                    FunctionKind::Getter | FunctionKind::Setter => non_const_field = true,
                                    FunctionKind::Function | FunctionKind::Operator => static_methods = true,
                                }
                            }
                            MemberKind::Constructor(k) => {
                                own_constructor |= k.name.is_some() || k.parameters.iter().any(|p| p.kind == ParameterKind::Required || p.required);
                            }
                        }
                    }
                    !has_instance && !own_constructor && (static_methods || non_const_field)
                }
            };
            if reported {
                report(&c::AVOID_CLASSES_WITH_ONLY_STATIC_MEMBERS, d.span, &[]);
            }
        }
    }
    // `prefer_final_in_for_each`: the variable (or the pattern's variables) of
    // a `for-in` inside a function body that no write changes (by element:
    // needs the unit's semantics).
    if enabled("prefer_final_in_for_each")
        && let Some(s) = sem
    {
        // `potentiallyMutates`: only a declared, non-mutated variable passes.
        let unmutated = |p: PatternId| matches!(&a.pattern(p).kind, PatternKind::Variable { name, .. } if !super::is_mutated(s, a, name.span.start));
        let mut check_target = |target: &ForInTarget| match target {
            ForInTarget::Declared { final_: false, name, .. } => {
                if !super::is_mutated(s, a, name.span.start) {
                    report(&c::PREFER_FINAL_IN_FOR_EACH_VARIABLE, name.span, &[interner.resolve(name.sym)]);
                }
            }
            ForInTarget::Pattern { final_: false, pattern } => {
                let n = a.pattern(*pattern);
                let reported = match &n.kind {
                    PatternKind::Record { fields } | PatternKind::Object { fields, .. } => fields.iter().all(|f| unmutated(f.pattern)),
                    PatternKind::List { elements, .. } => elements.iter().all(|el| matches!(el, ListPatternElement::Pattern(x) if unmutated(*x))),
                    PatternKind::Map { entries, rest, .. } => !*rest && entries.iter().all(|en| unmutated(en.value)),
                    _ => false,
                };
                if reported {
                    report(&c::PREFER_FINAL_IN_FOR_EACH_PATTERN, n.span, &[]);
                }
            }
            _ => {}
        };
        for st in a.stmts.iter() {
            if let StmtKind::ForIn { target, .. } = &st.kind {
                check_target(target);
            }
        }
        // The collection `for-in`, only inside a function body.
        let bodies: Vec<Span> = a
            .functions
            .iter()
            .filter_map(|f| match &f.body {
                ast::FunctionBody::Block(b) => Some(a.stmt(*b).span),
                ast::FunctionBody::Expression(e) => Some(a.expr(*e).span),
                _ => None,
            })
            .chain(a.members.iter().filter_map(|m| match &m.kind {
                MemberKind::Constructor(k) => match &k.body {
                    ast::FunctionBody::Block(b) => Some(a.stmt(*b).span),
                    ast::FunctionBody::Expression(e) => Some(a.expr(*e).span),
                    _ => None,
                },
                _ => None,
            }))
            .collect();
        for e in a.exprs.iter() {
            if let ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } = &e.kind {
                if !bodies.iter().any(|c| c.start <= e.span.start && e.span.end <= c.end) {
                    continue;
                }
                let mut for_ins = Vec::new();
                for el in elements.iter() {
                    collection_for_ins(a, el, &mut for_ins);
                }
                for (target, _) in for_ins {
                    check_target(target);
                }
            }
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
        let u = Unit { ast: &p.ast, unit: &p.unit, source };
        let mut reports = run(u, &names, &|r| r == rule, None);
        reports.sort_by_key(|r| (r.span.start, r.span.end));
        reports.into_iter().map(|r| (r.code.unique, source[r.span.start..r.span.end].to_string())).collect()
    }

    #[test]
    fn cascades_on_lists() {
        let source = "var a = [1]..add(2);\nvar b = [1]..addAll([2, 3]);\nvar c = [1]..addAll(a);\nvar d = [1]..length..add(2);\nvar e = a..add(2);\n";
        assert_eq!(
            with_code("prefer_inlined_adds", source),
            vec![("prefer_inlined_adds_single", "add".to_string()), ("prefer_inlined_adds_multiple", "addAll".to_string())]
        );
        assert_eq!(with_code("prefer_spread_collections", source), vec![("prefer_spread_collections", "addAll".to_string())]);
    }

    #[test]
    fn classes() {
        let source = "class A {\n  final int a = 1;\n  final int b;\n  static final int c = 2;\n  const A(int x) : b = 0;\n}\nclass B {\n  final int d;\n  const B(int x) : d = x;\n}\n";
        let v: Vec<String> = with_code("avoid_field_initializers_in_const_classes", source).into_iter().map(|x| x.1).collect();
        // The initializer that reads no parameter needs the unit's semantics;
        // the field with an initializer does not.
        assert_eq!(v, vec!["a = 1".to_string()]);
        let source = "class A {\n  static int f() => 0;\n}\nclass B {\n  static const int k = 1;\n}\nclass C {\n  static int v = 1;\n  int m() => 0;\n}\nclass D {\n  static int v = 1;\n  D(int x);\n}\nclass E {\n  static int v = 1;\n}\n";
        let v = with_code("avoid_classes_with_only_static_members", source);
        assert_eq!(v.len(), 2, "{v:?}");
        assert!(v[0].1.starts_with("class A") && v[1].1.starts_with("class E"));
    }

    #[test]
    fn for_each_final() {
        let source = "void f(List<int> l, List<(int, int)> p) {\n  for (var a in l) {}\n  for (var b in l) {\n    b++;\n  }\n  for (final c in l) {}\n  for (var (x, y) in p) {}\n  for (var (z, w) in p) {\n    z = w;\n  }\n  var q = [for (var d in l) d];\n}\n";
        // The mutation by element needs the unit's semantics.
        assert!(with_code("prefer_final_in_for_each", source).is_empty());
    }
}
