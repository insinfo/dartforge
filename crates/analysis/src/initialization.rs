//! `final_not_initialized` as in the analyzer `ErrorVerifier`'s
//! `_checkForNotInitializedFieldDeclaration` and top-level variable check.

use crate::Unit;
use dartforge_diagnostics::codes::compile_time_error as c;
use dartforge_diagnostics::Diagnostic;
use dartforge_frontend::ast::{Ast, DeclKind, Initializer, MemberKind, TypeId, TypeKind, VariableList};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};

fn final_without_initializer(v: &VariableList, names: &Interner, out: &mut Vec<(usize, Diagnostic)>, unit: usize, enum_index: bool) {
    if !v.final_ || v.const_ || v.late || v.external || v.abstract_ {
        return;
    }
    for var in v.variables.iter().filter(|var| var.initializer.is_none()) {
        if enum_index && names.resolve(var.name.sym) == "index" { continue; }
        out.push((unit, Diagnostic::with_code(
            c::FINAL_NOT_INITIALIZED,
            var.name.span,
            [names.resolve(var.name.sym)],
        )));
    }
}

/// Enough syntactic evidence of non-nullability. Imported types, aliases and
/// type parameters are left to the semantic phase; assuming them here could
/// create false positives (`typedef T = int?`, for example).
fn surely_non_nullable_type(ast: &Ast, id: TypeId, names: &Interner, aliases: &HashSet<SymbolId>) -> bool {
    let ty = ast.ty(id);
    if ty.nullable { return false; }
    match &ty.kind {
        TypeKind::Function { .. } | TypeKind::Record { .. } => true,
        TypeKind::Named { name, .. } if name.len() == 1 && !aliases.contains(&name[0].sym) => matches!(
            names.resolve(name[0].sym),
            "Object" | "Never" | "num" | "int" | "double" | "bool" | "String"
                | "List" | "Map" | "Set" | "Iterable" | "Iterator" | "Future" | "Stream"
        ),
        _ => false,
    }
}

/// `_checkForNotInitializedNonNullableInstanceFields` (3.6.2,
/// `error_verifier.dart:4837-4862`): the non-static, non-`late`, non-`final`
/// list (a `const` one counts: the list's `isFinal` is only the `final`
/// keyword); each variable without an initializer, neither abstract nor
/// external, of a potentially non-nullable type. `non_nullable` is the
/// predicate on the element's type (unit, start of the name); without it, the
/// syntactic evidence.
fn non_final_non_nullable_instance(v: &VariableList, ast: &Ast, names: &Interner, aliases: &HashSet<SymbolId>, out: &mut Vec<(usize, Diagnostic)>, unit: usize, enum_index: bool, non_nullable: &dyn Fn(usize, usize) -> Option<bool>) {
    if v.static_ || v.final_ || v.late || v.external || v.abstract_ {
        return;
    }
    for var in v.variables.iter().filter(|var| var.initializer.is_none()) {
        let potential = match non_nullable(unit, var.name.span.start) {
            Some(b) => b,
            None => !v.const_ && v.ty.is_some_and(|t| surely_non_nullable_type(ast, t, names, aliases)),
        };
        if !potential {
            continue;
        }
        if enum_index && names.resolve(var.name.sym) == "index" { continue; }
        out.push((unit, Diagnostic::with_code(
            c::NOT_INITIALIZED_NON_NULLABLE_INSTANCE_FIELD,
            var.name.span,
            [names.resolve(var.name.sym)],
        )));
    }
}

/// The class of the fields: (enum?, name, unit, declaration); the unit and the
/// declaration are zero when declarations merge by name.
type Key = (bool, SymbolId, usize, usize);

/// A class with an explicit generative constructor delegates the check of its
/// instance fields to the `ConstructorFieldsVerifier`; factories do not.
/// `CONST_NOT_INITIALIZED` (the parser's `parseFieldInitializerOpt`,
/// `parser_impl.dart:3942-3947`, and the `ErrorVerifier`'s
/// `_checkForFinalNotInitialized`, `error_verifier.dart:3576-3617`): each
/// variable without an initializer in a top-level, field (any declaration) or
/// local `const` list — not the one in a `for` header. On the name. Written
/// without compiling or running (2026-10-05).
pub fn uninitialized_constants(u: &Unit<'_>, names: &Interner) -> Vec<Diagnostic> {
    let a = u.ast;
    let mut out = Vec::new();
    let mut list = |l: &VariableList| {
        if !l.const_ {
            return;
        }
        for var in l.variables.iter().filter(|v| v.initializer.is_none()) {
            out.push(Diagnostic::with_code(c::CONST_NOT_INITIALIZED, var.name.span, [names.resolve(var.name.sym)]));
        }
    };
    for d in &a.decls {
        if let DeclKind::Variables(l) = &d.kind {
            list(l);
        }
    }
    for m in &a.members {
        if let MemberKind::Field(l) = &m.kind {
            list(l);
        }
    }
    for s in &a.stmts {
        if let dartforge_frontend::ast::StmtKind::Variables(l) = &s.kind {
            list(l);
        }
    }
    out
}

pub fn uninitialized_finals(units: &[Unit<'_>], names: &Interner) -> Vec<(usize, Diagnostic)> {
    uninitialized_finals_with(units, names, &|_, _| None)
}

/// Like [`uninitialized_finals`], with the semantic predicate
/// "potentially non-nullable" on each field's type (by unit, in the order of
/// `units`, and by the start of the name); `None` falls back to the syntactic
/// evidence.
pub fn uninitialized_finals_with(units: &[Unit<'_>], names: &Interner, non_nullable: &dyn Fn(usize, usize) -> Option<bool>) -> Vec<(usize, Diagnostic)> {
    let aliases: HashSet<_> = units.iter().flat_map(|u| u.unit.declarations.iter().filter_map(|&id| {
        match &u.ast.decl(id).kind {
            DeclKind::Typedef(x) => Some(x.name.sym),
            _ => None,
        }
    })).collect();
    // `units` holds a library and its parts. The augmented declarations of
    // the same class may be in different units, and with the experiment they
    // merge by name. Without it (no `augment` declaration), each declaration
    // has its own fields, including a class with the same name (T1.1 and,
    // `c11`: each one's constructor flags its own fields).
    let merge = units.iter().any(|u| u.unit.declarations.iter().any(|&d| u.ast.decl(d).augment));
    let key_of = |e: bool, name: SymbolId, unit: usize, id: dartforge_frontend::ast::DeclId| -> Key {
        if merge { (e, name, 0, 0) } else { (e, name, unit, id.0 as usize) }
    };
    let mut with_generative = HashSet::<Key>::new();
    let mut fields = HashMap::<Key, Vec<(SymbolId, bool, bool)>>::new();
    for (ui, unit) in units.iter().enumerate() {
        for &id in &unit.unit.declarations {
            let (key, members, ffi) = match &unit.ast.decl(id).kind {
                DeclKind::Class(x) => {
                    let ffi = x.extends.is_some_and(|t| {
                        if let TypeKind::Named { name, .. } = &unit.ast.ty(t).kind {
                            name.last().is_some_and(|n| matches!(names.resolve(n.sym), "Struct" | "Union"))
                        } else { false }
                    });
                    (key_of(false, x.name.sym, ui, id), &x.members, ffi)
                }
                DeclKind::Enum(x) => (key_of(true, x.name.sym, ui, id), &x.members, false),
                _ => continue,
            };
            if members.iter().any(|&m| {
                matches!(&unit.ast.member(m).kind, MemberKind::Constructor(k) if !k.factory)
            }) {
                with_generative.insert(key);
            }
            for &m in members {
                if let MemberKind::Field(v) = &unit.ast.member(m).kind {
                    if v.static_ { continue; }
                    for var in &v.variables {
                        // The implicit `Enum.index` member takes no part in the
                        // initialization check, even if the user declares
                        // another `index` (an enum error of its own).
                        if key.0 && names.resolve(var.name.sym) == "index" { continue; }
                        fields.entry(key).or_default().push((
                            var.name.sym,
                            v.final_ && !v.const_ && !v.late && !v.external && !v.abstract_ && var.initializer.is_none(),
                            !ffi && !v.final_ && !v.const_ && !v.late && !v.external && !v.abstract_
                                && var.initializer.is_none()
                                && match non_nullable(ui, var.name.span.start) {
                                    Some(b) => b,
                                    None => v.ty.is_some_and(|t| surely_non_nullable_type(unit.ast, t, names, &aliases)),
                                },
                        ));
                    }
                }
            }
        }
    }

    // The official verifier ignores duplicate field names; the duplicate
    // diagnostic is already emitted by another verifier.
    let mut pending = HashMap::<Key, Vec<SymbolId>>::new();
    let mut pending_non_finals = HashMap::<Key, Vec<SymbolId>>::new();
    // A repeated name counts once (the 3.6.2 oracle reports `'v'` once for
    // `final int v; final int v;` without initializing), by the first.
    for (key, fields) in fields {
        let mut seen = std::collections::HashSet::<SymbolId>::new();
        let first: Vec<&(SymbolId, bool, bool)> = fields.iter().filter(|(name, _, _)| seen.insert(*name)).collect();
        pending.insert(key, first.iter().filter_map(|(name, missing, _)| missing.then_some(*name)).collect());
        pending_non_finals.insert(key, first.iter().filter_map(|(name, _, missing)| missing.then_some(*name)).collect());
    }

    let mut out = Vec::new();
    for (i, unit) in units.iter().enumerate() {
        for &id in &unit.unit.declarations {
            let members = match &unit.ast.decl(id).kind {
                DeclKind::Variables(v) => {
                    final_without_initializer(v, names, &mut out, i, false);
                    continue;
                }
                DeclKind::Class(x) => (&x.members, with_generative.contains(&key_of(false, x.name.sym, i, id))),
                DeclKind::Enum(x) => (&x.members, with_generative.contains(&key_of(true, x.name.sym, i, id))),
                DeclKind::Mixin(x) => (&x.members, false),
                DeclKind::Extension(x) => (&x.members, false),
                // An extension type's primary is a written generative constructor.
                DeclKind::ExtensionType(x) => (&x.members, true),
                _ => continue,
            };
            for &m in members.0 {
                match &unit.ast.member(m).kind {
                    MemberKind::Field(v) => {
                        // `_checkForFinalNotInitializedInClass` (3.6.2
                        // `error_verifier.dart:3624-3643`): with a written
                        // generative constructor, it returns early; not even
                        // the `static final` ones are checked.
                        if !members.1 {
                            let enum_index = !v.static_ && matches!(&unit.ast.decl(id).kind, DeclKind::Enum(_));
                            final_without_initializer(v, names, &mut out, i, enum_index);
                            if !members.1 {
                                let valid = match &unit.ast.decl(id).kind {
                                    DeclKind::Class(x) => !x.extends.is_some_and(|t| {
                                        if let TypeKind::Named { name, .. } = &unit.ast.ty(t).kind {
                                            name.last().is_some_and(|n| matches!(names.resolve(n.sym), "Struct" | "Union"))
                                        } else { false }
                                    }),
                                    // Class, enum, extension and mixin
                                    // (`_checkForFinalNotInitializedInClass`).
                                    DeclKind::Enum(_) | DeclKind::Mixin(_) | DeclKind::Extension(_) => true,
                                    _ => false,
                                };
                                if valid {
                                    non_final_non_nullable_instance(v, unit.ast, names, &aliases, &mut out, i, enum_index, non_nullable);
                                }
                            }
                        }
                    }
                    MemberKind::Constructor(k) if !k.factory && !k.external && k.redirect.is_none() => {
                        // Extension type: the representation is the final field
                        // that every generative, non-redirecting constructor
                        // initializes (`this.it` or `it = …`); the `this` part of
                        // the primary is the primary itself.
                        if let DeclKind::ExtensionType(x) = &unit.ast.decl(id).kind {
                            if k.primary_body_part || k.initializers.iter().any(|y| matches!(y, Initializer::Redirect { .. })) {
                                continue;
                            }
                            let rep = x.representation_name.sym;
                            let initializes = k.parameters.iter().any(|p| p.this_ && p.name.is_some_and(|n| n.sym == rep))
                                || k.initializers.iter().any(|y| matches!(y, Initializer::Field { name, .. } if name.sym == rep));
                            if !initializes {
                                // The synthetic representation (missing primary
                                // constructor or field) has an empty name, which the
                                // message writes as `<empty>`.
                                let name = match names.resolve(rep) {
                                    "" => "<empty>".to_string(),
                                    r => r.to_string(),
                                };
                                out.push((i, Diagnostic::with_code(c::FINAL_NOT_INITIALIZED_CONSTRUCTOR_1, k.class_name.span, [name])));
                            }
                            continue;
                        }
                        let (key, primary) = match &unit.ast.decl(id).kind {
                            DeclKind::Class(x) => (key_of(false, x.name.sym, i, id), x.primary_constructor),
                            DeclKind::Enum(x) => (key_of(true, x.name.sym, i, id), x.primary_constructor),
                            _ => continue,
                        };
                        // The primary constructor (3.13 syntax, checked by the
                        // 3.13 oracle) also initializes: report on the class
                        // name, like the others.
                        let _ = primary;
                        if k.initializers.iter().any(|x| matches!(x, Initializer::Redirect { .. })) { continue; }
                        let initialized = |wanted: &SymbolId| {
                            k.parameters.iter().any(|p| p.this_ && p.name.is_some_and(|n| n.sym == *wanted))
                                || k.initializers.iter().any(|x| matches!(x, Initializer::Field { name, .. } if name.sym == *wanted))
                        };
                        let mut missing = pending.get(&key).cloned().unwrap_or_default();
                        missing.retain(|name| !initialized(name));
                        // The 3.6.2 oracle marks only the class name, even in
                        // `A.named()`; analyzer main extends it to `.named`.
                        if !missing.is_empty() {
                            let mut missing_names: Vec<_> = missing.iter().map(|s| names.resolve(*s).to_string()).collect();
                            missing_names.sort();
                            let code = match missing_names.len() {
                                1 => c::FINAL_NOT_INITIALIZED_CONSTRUCTOR_1,
                                2 => c::FINAL_NOT_INITIALIZED_CONSTRUCTOR_2,
                                _ => c::FINAL_NOT_INITIALIZED_CONSTRUCTOR_3_PLUS,
                            };
                            let arguments = match missing_names.len() {
                                1 | 2 => missing_names,
                                n => vec![missing_names[0].clone(), missing_names[1].clone(), (n - 2).to_string()],
                            };
                            out.push((i, Diagnostic::with_code(code, k.class_name.span, arguments)));
                        }
                        let mut non_finals = pending_non_finals.get(&key).cloned().unwrap_or_default();
                        non_finals.retain(|name| !initialized(name));
                        let mut non_final_names: Vec<_> = non_finals.iter().map(|s| names.resolve(*s).to_string()).collect();
                        non_final_names.sort();
                        for name in non_final_names {
                            out.push((i, Diagnostic::with_code(
                                c::NOT_INITIALIZED_NON_NULLABLE_INSTANCE_FIELD_CONSTRUCTOR,
                                k.class_name.span,
                                [name],
                            )));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use dartforge_frontend::parser::parse;

    fn run(source: &str) -> Vec<(String, String)> {
        let mut names = Interner::new();
        let parsed = parse(source, &mut names);
        uninitialized_finals(&[Unit { ast: &parsed.ast, unit: &parsed.unit, source }], &names)
            .into_iter()
            .map(|(_, d)| {
                let name = source[d.span.start as usize..d.span.end as usize].to_string();
                (name, d.message)
            })
            .collect()
    }

    #[test]
    fn top_level_static_instance_and_exceptions_from_oracle() {
        let source = "final int topo; class A { static final int estatico; final int instancia; factory A() => throw 0; } class B { final int delegado; B(); } abstract class C { abstract final int abstrato; external final int externo; late final int tardio; }";
        let mut found_items = run(source);
        found_items.sort();
        assert_eq!(found_items, vec![
            ("B".into(), "All final variables must be initialized, but 'delegado' isn't.".into()),
            ("estatico".into(), "The final variable 'estatico' must be initialized.".into()),
            ("instancia".into(), "The final variable 'instancia' must be initialized.".into()),
            ("topo".into(), "The final variable 'topo' must be initialized.".into()),
        ]);
    }

    #[test]
    fn partial_list_and_nullable_follow_same_rule() {
        let found_items = run("final Object? v; class A { static final int a = 0, b, c = 0; }");
        assert_eq!(found_items, vec![
            ("v".into(), "The final variable 'v' must be initialized.".into()),
            ("b".into(), "The final variable 'b' must be initialized.".into()),
        ]);
    }

    #[test]
    fn three_sources_from_official_corpus() {
        for (source, name) in [
            (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__topLevelVariabl_dd6c7267.dart")), "v"),
            (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__class_instanceF_5d207b49.dart")), "v"),
        ] {
            assert_eq!(run(source), vec![(
                name.into(),
                format!("The final variable '{name}' must be initialized."),
            )]);
        }
    }

    /// With a written generative constructor, `_checkForFinalNotInitializedInClass`
    /// returns early and the `static final` without an initializer is not
    /// reported here (the 3.6.2 oracle only has `not_initialized_non_nullable_variable`).
    #[test]
    fn static_with_generative_constructor_not_reported() {
        let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__class_staticFie_99366e40.dart"));
        assert!(run(source).iter().all(|(_, m)| !m.starts_with("The final variable")));
    }

    #[test]
    fn constructors_initialize_fields_by_parameter_or_list() {
        let mut found_items = run("class A { final int x; final int y; A(this.x); A.named() : y = 0; } class B { final int v; B() : this.named(); B.named() : v = 0; }");
        found_items.sort();
        assert_eq!(found_items, vec![
            ("A".into(), "All final variables must be initialized, but 'x' isn't.".into()),
            ("A".into(), "All final variables must be initialized, but 'y' isn't.".into()),
        ]);
    }

    #[test]
    fn constructor_with_three_fields_uses_3_plus_variant() {
        let found_items = run("class A { final int z; final int x; final int y; A(); }");
        assert_eq!(found_items, vec![(
            "A".into(),
            "All final variables must be initialized, but 'x', 'y', and 1 others aren't.".into(),
        )]);
    }

    #[test]
    fn primary_constructor_reports_on_class_name_like_3_13_oracle() {
        // A new-syntax file: the oracle is 3.13, which checks the primary.
        let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__class_instanceF_01f4790b.dart"));
        assert_eq!(run(source), vec![("A".into(), "All final variables must be initialized, but 'v2' isn't.".into())]);
    }

    #[test]
    fn secondary_constructors_match_3_6_oracle() {
        for (source, offset, code, message) in [
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__class_instanceF_0811e46c.dart")),
                60,
                c::FINAL_NOT_INITIALIZED_CONSTRUCTOR_3_PLUS,
                "All final variables must be initialized, but 'v1', 'v2', and 1 others aren't.",
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__class_instanceF_568a572a.dart")),
                45,
                c::FINAL_NOT_INITIALIZED_CONSTRUCTOR_1,
                "All final variables must be initialized, but 'v' isn't.",
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__enum_instanceFi_5eb8f155.dart")),
                70,
                c::FINAL_NOT_INITIALIZED_CONSTRUCTOR_1,
                "All final variables must be initialized, but 'v' isn't.",
            ),
        ] {
            let mut names = Interner::new();
            let parsed = parse(source, &mut names);
            let found = uninitialized_finals(&[Unit { ast: &parsed.ast, unit: &parsed.unit, source }], &names);
            assert_eq!(found.len(), 1, "{found:?}");
            let d = &found[0].1;
            assert_eq!(d.code, Some(code), "{found:?}");
            assert_eq!(d.span.start, offset, "{found:?}");
            assert_eq!(d.span.end, offset + 1, "{found:?}");
            assert_eq!(d.message, message, "{found:?}");
        }
    }

    #[test]
    fn enum_index_uses_own_diagnostic() {
        let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/illegal_concrete_enum_member_declaration/IllegalConcreteEnumMemberDeclarationEnu_9000f3c7.dart"));
        assert!(run(source).is_empty());
        assert!(run("enum E { v; final int index; }").is_empty());
        assert_eq!(run("enum E { v; static final int index; }"), vec![(
            "index".into(), "The final variable 'index' must be initialized.".into(),
        )]);
    }

    #[test]
    fn non_final_non_nullable_instance_fields_from_oracle() {
        for source in [
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__class_instanceF_87a3792e.dart")),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__mixin_instanceF_fa8a8f5a.dart")),
        ] {
            assert_eq!(run(source), vec![(
                "v".into(), "Non-nullable instance field 'v' must be initialized.".into(),
            )]);
            let mut names = Interner::new();
            let parsed = parse(source, &mut names);
            let found = uninitialized_finals(&[Unit { ast: &parsed.ast, unit: &parsed.unit, source }], &names);
            assert_eq!(found[0].1.code, Some(c::NOT_INITIALIZED_NON_NULLABLE_INSTANCE_FIELD));
            assert_eq!((found[0].1.span.start, found[0].1.span.end), (16, 17));
            assert_eq!(found[0].1.code.unwrap().info().correction, Some("Try adding an initializer expression, or a generative constructor that initializes it, or mark it 'late'."));
        }
        assert_eq!(run("class A { int x; Object? y; late int z; factory A() => throw 0; }"), vec![(
            "x".into(), "Non-nullable instance field 'x' must be initialized.".into(),
        )]);
        assert!(run("class A { int x; A(this.x); }").is_empty());
    }

    #[test]
    fn constructors_with_non_final_non_nullable_fields_from_oracle() {
        for (source, offset, field) in [
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__class_instanceF_50a84c0d.dart")),
                31,
                "v2",
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__class_instanceF_b270bd6f.dart")),
                40,
                "v",
            ),
        ] {
            let mut names = Interner::new();
            let parsed = parse(source, &mut names);
            let found = uninitialized_finals(&[Unit { ast: &parsed.ast, unit: &parsed.unit, source }], &names);
            assert_eq!(found.len(), 1, "{found:?}");
            let d = &found[0].1;
            assert_eq!(d.code, Some(c::NOT_INITIALIZED_NON_NULLABLE_INSTANCE_FIELD_CONSTRUCTOR));
            assert_eq!((d.span.start, d.span.end), (offset, offset + 1));
            assert_eq!(d.message, format!("Non-nullable instance field '{field}' must be initialized."));
            assert_eq!(d.code.unwrap().info().correction, Some("Try adding an initializer expression, or add a field initializer in this constructor, or mark it 'late'."));
        }
        assert!(run("class A { int x; A(this.x); A.named() : x = 0; }").is_empty());
    }
}
