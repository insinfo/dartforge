//! More quick fixes for the published codes, with the titles, kinds and
//! edits of the Dart 3.6.2 producers
//! (`pkg/analysis_server/lib/src/services/correction/dart/*.dart`, bound to
//! the codes in `fix_internal.dart`):
//!
//! | Code | Fixes |
//! | --- | --- |
//! | `non_abstract_class_inherits_abstract_member` | `Create N missing override(s)`, `Create 'noSuchMethod' method`, `Make class 'C' abstract` |
//! | `concrete_class_with_abstract_member` | `Convert to block body`, `Make class 'C' abstract` |
//! | `unused_field` | `Remove unused field` |
//! | `unused_catch_clause` / `unused_catch_stack` | `Remove unused 'catch' clause` / `Remove unused stack trace variable` |
//! | `assignment_to_final_local` | `Make variable 'x' not final` |
//! | `missing_default_value_for_parameter` | `Add 'required' keyword` (named), `Make 'x' nullable` |
//! | `await_in_wrong_context` | `Add 'async' modifier` (with `Future<…>` on the return) |
//! | `nullable_type_in_{extends,implements,on,with}_clause` | `Remove the '?'` |
//! | `const_instance_field` | `Add 'static' modifier` |
//! | `non_final_field_in_enum` | `Make final` |
//! | `extension_declares_member_of_object`, `extension_type_declares_member_of_object` | `Remove method declaration` |
//! | `assert_in_redirecting_constructor` | `Remove the assertion` |
//!
//! Each edit is checked against the tree of the current text: a diagnostic
//! that does not match the expected node produces no action.

use crate::actions::CodeAction;
use crate::project::{Project, base_name};
use crate::Edit;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{ClassId, FunctionElementId, FunctionKind, FunctionRef, UnitId};
use dartforge_frontend::ast::{self, DeclKind, MemberKind, ParameterKind, StmtKind};
use dartforge_types::{Type, TypeId};
use std::collections::{BTreeMap, HashMap, HashSet};

/// An action whose edits (in one file) come with the imports that
/// `writeType`/`writeReference` scheduled (`Change`, with the
/// `ChangeBuilder`'s conflict detection).
fn action_with_imports(
    cx: &crate::refactorings::Context<'_>,
    uri: &str,
    title: String,
    kind: &str,
    edits: Vec<(Span, String)>,
    import: &std::collections::BTreeSet<dartforge_elements::model::LibraryId>,
    d: &Diagnostic,
) -> Option<CodeAction> {
    let mut m = crate::refactorings_exec::Change::default();
    for (s, x) in edits {
        m.add(uri, s, x);
    }
    crate::refactorings_move::builder_imports(cx, &mut m, import);
    if m.conflict.is_some() {
        return None;
    }
    Some(CodeAction {
        title,
        kind: kind.into(),
        edits: m.files.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect(),
        diagnostic: Some(d.clone()),
        create_file: None, linked: Default::default(),
    })
}

/// `AddMissingRequiredArgument` on the calls of the requested lines (the
/// `missing_required_argument` code is not published; the action comes out
/// without a diagnostic).
pub(crate) fn required_arguments(project: &Project, uri: &str, start: usize, end: usize) -> Vec<CodeAction> {
    let Some(unit) = project.uri_unit(uri) else { return Vec::new() };
    let cx = crate::refactorings::Context::new(project, unit);
    cx.required_arguments(start, end)
        .into_iter()
        .map(|(name, s, text, cursor)| {
            let a = CodeAction {
                title: format!("Add required argument '{name}'"),
                kind: "quickfix.add.missingRequiredArgument".into(),
                edits: vec![Edit { uri: uri.to_string(), span: s, text }],
                diagnostic: None,
                create_file: None,
                linked: Default::default(),
            };
            match cursor {
                Ok(k) => a.with_selection(0, k, 0),
                Err((k, n)) => a.with_group(0, &[k], n, Vec::new()),
            }
        })
        .collect()
}

/// `ConvertIntoBlockBody.missingBody`: `selectHere` before the line break
/// that precedes the final `}` (`convert_into_block_body.dart:52-53`).
pub(crate) fn block_body(a: CodeAction) -> CodeAction {
    match a.edits.first().and_then(|e| e.text.rfind('\n')) {
        Some(k) => a.with_selection(0, k, 0),
        None => a,
    }
}

fn action(uri: &str, title: String, kind: &str, edits: Vec<(Span, String)>, d: &Diagnostic) -> CodeAction {
    CodeAction {
        title,
        kind: kind.into(),
        edits: edits.into_iter().map(|(span, text)| Edit { uri: uri.to_string(), span, text }).collect(),
        diagnostic: Some(d.clone()),
        create_file: None, linked: Default::default(),
    }
}

/// The span extended to the whole line when it is alone on the line.
fn whole_lines(text: &str, span: Span) -> Span {
    let start = text[..span.start].rfind('\n').map_or(0, |i| i + 1);
    let end = text[span.end..].find('\n').map_or(text.len(), |i| span.end + i);
    if text[start..span.start].trim().is_empty() && text[span.end..end].trim().is_empty() {
        Span { start: start, end: (end + 1).min(text.len()) }
    } else {
        span
    }
}

/// The fixes of this list for the diagnostics touching `start..end`.
pub(crate) fn fix(project: &mut Project, uri: &str, diagnostics: &[Diagnostic], start_: usize, end: usize) -> Vec<CodeAction> {
    let Some(unit) = project.uri_unit(uri) else { return Vec::new() };
    let mut output = Vec::new();
    for d in diagnostics {
        if !(d.span.start <= end && start_ <= d.span.end) {
            continue;
        }
        let Some(code) = d.code.map(|c| c.info().name) else { continue };
        let text = project.program().unit(unit).source.clone();
        if d.span.end > text.len() {
            continue;
        }
        match code {
            "non_abstract_class_inherits_abstract_member" => {
                let Some((c, decl)) = class_with_name(project, unit, d.span) else { continue };
                if let Some((title, edit, selection, groups)) = missing_overrides(project, c, unit, decl) {
                    // The `<method>:TYPE<i>`/`PARAM<i>` groups of the parameters
                    // (`writeParameters` with `methodBeingCopied`) and the selection.
                    let mut a = action(uri, title, "quickfix.create.missingOverrides", vec![edit], d);
                    for (r, n, suggestions) in groups {
                        a = a.with_group(0, &[r], n, suggestions);
                    }
                    output.push(match selection {
                        Some((rel, n)) => a.with_selection(0, rel, n),
                        None => a,
                    });
                }
                if let Some(e) = no_such_method(project, unit, decl) {
                    // `selectHere` before everything (`create_no_such_method.dart:33`).
                    output.push(action(uri, "Create 'noSuchMethod' method".into(), "quickfix.create.noSuchMethod", vec![e], d).with_selection(0, 0, 0));
                }
                if let Some((name, e)) = abstract_class(project, unit, decl) {
                    output.push(action(uri, format!("Make class '{name}' abstract"), "quickfix.makeClassAbstract", vec![e], d));
                }
            }
            "concrete_class_with_abstract_member" => {
                // `ConvertIntoBlockBody.missingBody`, `CreateNoSuchMethod` (only with
                // `node` on the `ClassDeclaration`: here the error is on the member, so it
                // never comes out) and `MakeClassAbstract`.
                let cx = crate::refactorings::Context::new(project, unit);
                if let Some(e) = cx.convert_to_block_body(d.span) {
                    output.push(block_body(action(uri, "Convert to block body".into(), "quickfix.convert.bodyToBlock", vec![e], d)));
                }
                drop(cx);
                let Some(decl) = containing_class(project, unit, d.span.start) else { continue };
                if let Some((name, e)) = abstract_class(project, unit, decl) {
                    output.push(action(uri, format!("Make class '{name}' abstract"), "quickfix.makeClassAbstract", vec![e], d));
                }
            }
            "unused_field" => {
                // `RemoveUnusedField`: the declaration and each reference.
                let cx = crate::refactorings::Context::new(project, unit);
                if let Some(ranges) = cx.remove_field(d.span) {
                    let edits = ranges.into_iter().map(|s| (s, String::new())).collect();
                    output.push(action(uri, "Remove unused field".into(), "quickfix.remove.unusedField", edits, d));
                }
            }
            "unused_catch_clause" | "unused_catch_stack" => {
                let ast = &project.program().unit(unit).ast;
                for s in &ast.stmts {
                    let StmtKind::Try { catches, .. } = &s.kind else { continue };
                    for k in catches.iter() {
                        let body = ast.stmt(k.body).span.start;
                        if code == "unused_catch_clause" && k.exception.is_some_and(|n| n.span == d.span) {
                            // From `catch` to the body (the `on T` stays).
                            let Some(c) = text[k.span.start..body].find("catch").map(|i| k.span.start + i) else { continue };
                            if k.on_type.is_some() {
                                output.push(action(uri, "Remove unused 'catch' clause".into(), "quickfix.remove.unusedCatchClause", vec![(Span { start: c, end: body }, String::new())], d));
                            }
                        }
                        if code == "unused_catch_stack"
                            && let (Some(e), Some(st)) = (k.exception, k.stack_trace)
                            && st.span == d.span
                        {
                            output.push(action(uri, "Remove unused stack trace variable".into(), "quickfix.remove.unusedCatchStack", vec![(Span { start: e.span.end, end: st.span.end }, String::new())], d));
                        }
                    }
                }
            }
            "assignment_to_final_local" => {
                if let Some((name, e)) = local_not_final(project, unit, d.span) {
                    output.push(action(uri, format!("Make variable '{name}' not final"), "quickfix.makeVariableNotFinal", e, d));
                }
            }
            "missing_default_value_for_parameter" => {
                let ast = &project.program().unit(unit).ast;
                if let Some(p) = parameter_with_name(ast, d.span) {
                    if p.kind == ParameterKind::Named && !p.required {
                        let start = p.metadata.last().map_or(p.span.start, |a| {
                            let rest = &text[a.span.end..];
                            a.span.end + (rest.len() - rest.trim_start().len())
                        });
                        output.push(action(uri, "Add 'required' keyword".into(), "quickfix.add.required", vec![(Span { start: start, end: start }, "required ".into())], d));
                    }
                    if let Some(t) = p.ty {
                        let ts = ast.ty(t).span;
                        let name = &text[d.span.start..d.span.end];
                        output.push(action(uri, format!("Make '{name}' nullable"), "quickfix.makeVariableNullable", vec![(Span { start: ts.end, end: ts.end }, "?".into())], d));
                    }
                }
            }
            "await_in_wrong_context" => {
                // `AddAsync` with `convertFunctionFromSyncToAsync`; the libraries that
                // `writeType` scheduled come in as imports.
                let cx = crate::refactorings::Context::new(project, unit);
                if let Some((edits, import)) = cx.add_async(d.span) {
                    let mut m = crate::refactorings_exec::Change::default();
                    for (s, x) in edits {
                        m.add(uri, s, x);
                    }
                    crate::refactorings_move::builder_imports(&cx, &mut m, &import);
                    if m.conflict.is_none() {
                        let all: Vec<Edit> = m.files.into_iter().flat_map(|(_, l)| l.into_iter().rev()).collect();
                        output.push(CodeAction {
                            title: "Add 'async' modifier".into(),
                            kind: "quickfix.add.async".into(),
                            edits: all,
                            diagnostic: Some(d.clone()),
                            create_file: None, linked: Default::default(),
                        });
                    }
                }
            }
            "nullable_type_in_extends_clause" | "nullable_type_in_implements_clause" | "nullable_type_in_on_clause" | "nullable_type_in_with_clause" => {
                let snippet = &text[d.span.start..d.span.end];
                if snippet.ends_with('?') {
                    output.push(action(uri, "Remove the '?'".into(), "quickfix.remove.questionMark", vec![(Span { start: d.span.end - 1, end: d.span.end }, String::new())], d));
                }
            }
            "const_instance_field" => {
                let ast = &project.program().unit(unit).ast;
                if let Some(m) = ast.members.iter().find(|m| m.span.start <= d.span.start && d.span.end <= m.span.end && matches!(m.kind, MemberKind::Field(_))) {
                    let start = m.metadata.last().map_or(m.span.start, |a| {
                        let rest = &text[a.span.end..];
                        a.span.end + (rest.len() - rest.trim_start().len())
                    });
                    output.push(action(uri, "Add 'static' modifier".into(), "quickfix.add.static", vec![(Span { start: start, end: start }, "static ".into())], d));
                }
            }
            "non_final_field_in_enum" => {
                let cx = crate::refactorings::Context::new(project, unit);
                if let Some(e) = cx.make_final(d.span) {
                    output.push(action(uri, "Make final".into(), "quickfix.makeFinal", vec![e], d));
                }
            }
            "extension_declares_member_of_object" | "extension_type_declares_member_of_object" => {
                let ast = &project.program().unit(unit).ast;
                if let Some(m) = ast.members.iter().find(|m| m.span.start <= d.span.start && d.span.end <= m.span.end) {
                    let span = with_documentation(&text, whole_lines(&text, m.span));
                    output.push(action(uri, "Remove method declaration".into(), "quickfix.remove.methodDeclaration", vec![(span, String::new())], d));
                }
            }
            "assert_in_redirecting_constructor" => {
                // `RemoveAssertion`.
                let cx = crate::refactorings::Context::new(project, unit);
                if let Some(s) = cx.remove_assertion(d.span) {
                    output.push(action(uri, "Remove the assertion".into(), "quickfix.remove.assertion", vec![(s, String::new())], d));
                }
            }
            "cast_to_non_type" | "not_a_type" => {
                output.extend(project.create_for_diagnostic(unit, d, crate::create::Code::NotType));
            }
            "implements_non_class" | "mixin_of_non_class" | "new_with_non_type" => {
                output.extend(project.create_for_diagnostic(unit, d, crate::create::Code::NonClass));
            }
            "invalid_annotation" | "undefined_annotation" => {
                output.extend(project.create_for_diagnostic(unit, d, crate::create::Code::Annotation));
            }
            "undefined_extension_getter" => output.extend(project.create_for_diagnostic(unit, d, crate::create::Code::ExtensionGetter)),
            "undefined_extension_method" => output.extend(project.create_for_diagnostic(unit, d, crate::create::Code::ExtensionMethod)),
            "undefined_extension_setter" => output.extend(project.create_for_diagnostic(unit, d, crate::create::Code::ExtensionSetter)),
            "implements_repeated" | "implements_super_class" | "mixin_super_class_constraint_non_interface" | "subtype_of_disallowed_type" | "invalid_use_of_type_outside_library" | "extends_non_class" => {
                // `RemoveNameFromDeclarationClause` (on `extends_non_class`, after
                // `ChangeTo.classOrMixin` and `CreateClass`).
                if code == "extends_non_class" {
                    output.extend(project.create_for_diagnostic(unit, d, crate::create::Code::NonClass));
                }
                let cx = crate::refactorings::Context::new(project, unit);
                if let Some((title, s)) = cx.remove_clause_name(d.span) {
                    output.push(action(uri, title, "quickfix.remove.nameFromDeclarationClause", vec![(s, String::new())], d));
                }
            }
            "subtype_of_base_or_final_is_not_base_final_or_sealed" => {
                // `AddClassModifier.baseModifier`, `.finalModifier`, `.sealedModifier`.
                let cx = crate::refactorings::Context::new(project, unit);
                if let Some(o) = cx.modifier_dot(d.span) {
                    for m in ["base", "final", "sealed"] {
                        output.push(action(uri, format!("Add '{m}' modifier"), &format!("quickfix.add.class.modifier.{m}"), vec![(Span { start: o, end: o }, format!("{m} "))], d));
                    }
                }
            }
            "type_check_with_null" => {
                let cx = crate::refactorings::Context::new(project, unit);
                if let Some(s) = cx.is_null_range(d.span) {
                    let negated = d.code.is_some_and(|c| c.info().unique.ends_with("TYPE_CHECK_IS_NOT_NULL"));
                    let (title, kind, new_text) = if negated {
                        ("Use != null instead of 'is! Null'", "quickfix.use.notEqNull", " != null")
                    } else {
                        ("Use == null instead of 'is Null'", "quickfix.use.eqEqNull", " == null")
                    };
                    output.push(action(uri, title.into(), kind, vec![(s, new_text.into())], d));
                }
            }
            "mixin_class_declaration_extends_not_object" => {
                let cx = crate::refactorings::Context::new(project, unit);
                if let Some(s) = cx.remove_extends(d.span) {
                    output.push(action(uri, "Remove the invalid 'extends' clause".into(), "quickfix.remove.extends.clause", vec![(s, String::new())], d));
                }
            }
            "mixin_application_not_implemented_interface" => {
                let cx = crate::refactorings::Context::new(project, unit);
                if let Some((name, o)) = cx.extend_to_mixin(d.span, &d.message) {
                    output.push(action(uri, format!("Extend the class '{name}'"), "quickfix.extendClassForMixin", vec![(Span { start: o, end: o }, format!(" extends {name}"))], d));
                }
            }
            "extension_override_access_to_static_member" => {
                let cx = crate::refactorings::Context::new(project, unit);
                if let Some((name, s)) = cx.extension_name(d.span) {
                    output.push(action(uri, format!("Replace with '{name}'"), "quickfix.replace.withExtensionName", vec![(s, name.clone())], d));
                }
            }
            "invocation_of_non_function_expression" => {
                let cx = crate::refactorings::Context::new(project, unit);
                if let Some(s) = cx.getter_parens(d.span) {
                    output.push(action(uri, "Remove parentheses in getter invocation".into(), "quickfix.remove.parenthesisInGetterInvocation", vec![(s, String::new())], d));
                }
            }
            "super_invocation_not_last" => {
                let cx = crate::refactorings::Context::new(project, unit);
                if let Some(e) = cx.super_last(d.span) {
                    output.push(action(uri, "Move the invocation to the end of the initializer list".into(), "quickfix.makeSuperInvocationLast", e, d));
                }
            }
            "yield_of_invalid_type" => {
                let strict = project
                    .program()
                    .unit(unit)
                    .path
                    .as_deref()
                    .is_some_and(|c| crate::ignore_diagnostic::file_options(c).0.strict_casts);
                let cx = crate::refactorings::Context::new(project, unit);
                if let Some(o) = cx.nullable_return(d.span, strict) {
                    output.push(action(uri, "Make the return type nullable".into(), "quickfix.makeReturnTypeNullable", vec![(Span { start: o, end: o }, "?".into())], d));
                }
            }
            "unreachable_switch_case" | "unreachable_switch_default" => {
                let cx = crate::refactorings::Context::new(project, unit);
                if let Some(s) = cx.remove_dead_member(d.span) {
                    output.push(action(uri, "Remove dead code".into(), "quickfix.remove.deadCode", vec![(s, String::new())], d));
                }
            }
            "unnecessary_null_comparison" => {
                let is_true = d.code.is_some_and(|c| c.info().unique.ends_with("_TRUE"));
                let cx = crate::refactorings::Context::new(project, unit);
                if let Some(e) = cx.remove_comparison(d.span, is_true) {
                    output.push(action(uri, "Remove comparison".into(), "quickfix.remove.comparison", e, d));
                }
            }
            "enum_with_abstract_member" | "extension_type_with_abstract_member" => {
                let cx = crate::refactorings::Context::new(project, unit);
                if let Some(e) = cx.convert_to_block_body(d.span) {
                    output.push(block_body(action(uri, "Convert to block body".into(), "quickfix.convert.bodyToBlock", vec![e], d)));
                }
            }
            "non_constant_map_pattern_key" => {
                let cx = crate::refactorings::Context::new(project, unit);
                if let Some(e) = cx.add_const(d.span) {
                    output.push(action(uri, "Add 'const' modifier".into(), "quickfix.add.const", e, d));
                }
            }
            "non_exhaustive_switch_statement" | "non_exhaustive_switch_expression" => {
                let cx = crate::refactorings::Context::new(project, unit);
                if let Some((s, new_text, import)) = cx.missing_cases(d.span)
                    && let Some(a) = action_with_imports(&cx, uri, "Add missing switch cases".into(), "quickfix.add.missingSwitchCases", vec![(s, new_text)], &import, d)
                {
                    output.push(a);
                }
            }
            "new_with_undefined_constructor" => {
                let cx = crate::refactorings::Context::new(project, unit);
                if let Some(c) = crate::insert::create_constructor(&cx, d.span) {
                    let target = crate::refactorings::Context::new(project, c.unit);
                    let (s, t) = c.edit.clone();
                    if let Some(target_uri) = project.unit_uri(c.unit)
                        && let Some(mut a) = action_with_imports(&target, &target_uri, format!("Create constructor '{}'", c.name), "quickfix.create.constructor", vec![c.edit], &c.import, d)
                    {
                        if let Some((base, e)) = &c.marks
                            && let Some(i) = a.edits.iter().position(|x| x.span == s && x.text == t)
                        {
                            a = a.with_write_at(i, *base, e);
                        }
                        output.push(a);
                    }
                }
            }
            _ => {}
        }
    }
    output
}

/// `span` extended upwards through the `///` lines right above it.
fn with_documentation(text: &str, span: Span) -> Span {
    let mut start = span.start;
    while start > 0 {
        let previous_end = start - 1;
        let start_ = text[..previous_end].rfind('\n').map_or(0, |i| i + 1);
        if text[start_..previous_end].trim_start().starts_with("///") {
            start = start_;
        } else {
            break;
        }
    }
    Span { start: start, end: span.end }
}

/// The parameter (of a function, method or constructor) whose name is at `name`.
fn parameter_with_name(ast: &ast::Ast, name: Span) -> Option<&ast::Parameter> {
    let functions = ast.functions.iter().flat_map(|f| f.parameters.iter().flatten());
    let constructors = ast.members.iter().flat_map(|m| match &m.kind {
        MemberKind::Constructor(k) => k.parameters.iter(),
        _ => [].iter(),
    });
    functions.chain(constructors).find(|p| p.name.is_some_and(|n| n.span == name))
}

/// The class (element and declaration) whose name is at `name`.
fn class_with_name(project: &Project, unit: UnitId, name: Span) -> Option<(ClassId, ast::DeclId)> {
    let p = project.program();
    let ast = &p.unit(unit).ast;
    let (i, _) = ast.decls.iter().enumerate().find(|(_, d)| match &d.kind {
        DeclKind::Class(k) => k.name.span == name,
        DeclKind::Enum(k) => k.name.span == name,
        _ => false,
    })?;
    let decl = ast::DeclId(i as u32);
    let c = (0..p.classes.len()).map(|x| ClassId(x as u32)).find(|c| p.class(*c).decl.is_some_and(|d| d.unit == unit && d.decl == decl))?;
    Some((c, decl))
}

/// The class declaration that contains `offset`.
fn containing_class(project: &Project, unit: UnitId, offset: usize) -> Option<ast::DeclId> {
    let ast = &project.program().unit(unit).ast;
    ast.decls
        .iter()
        .enumerate()
        .find(|(_, d)| matches!(d.kind, DeclKind::Class(_)) && d.span.start <= offset && offset < d.span.end)
        .map(|(i, _)| ast::DeclId(i as u32))
}

/// `abstract ` before the `class` keyword (after the metadata).
fn abstract_class(project: &Project, unit: UnitId, decl: ast::DeclId) -> Option<(String, (Span, String))> {
    let u = project.program().unit(unit);
    let d = u.ast.decl(decl);
    let DeclKind::Class(k) = &d.kind else { return None };
    if k.modifiers.abstract_ || k.modifiers.sealed {
        return None;
    }
    let start = d.metadata.last().map_or(d.span.start, |a| a.span.end);
    let rest = &u.source[start..];
    let start = start + (rest.len() - rest.trim_start().len());
    Some((u.source[k.name.span.start..k.name.span.end].to_string(), (Span { start: start, end: start }, "abstract ".into())))
}

/// `noSuchMethod` before the class's `}`.
fn no_such_method(project: &Project, unit: UnitId, decl: ast::DeclId) -> Option<(Span, String)> {
    let u = project.program().unit(unit);
    let d = u.ast.decl(decl);
    let DeclKind::Class(k) = &d.kind else { return None };
    if !u.source[..d.span.end].ends_with('}') {
        return None;
    }
    let pos = d.span.end - 1;
    let separator = if k.members.is_empty() { "" } else { "\n" };
    Some((Span { start: pos, end: pos }, format!("{separator}  @override\n  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);\n")))
}

/// `Make variable 'x' not final` for the assignment at `usage`.
fn local_not_final(project: &Project, unit: UnitId, usage: Span) -> Option<(String, Vec<(Span, String)>)> {
    let u = project.program().unit(unit);
    let ast = &u.ast;
    let bodies = &project.query.bodies.units[unit.0 as usize];
    let expr = ast.exprs.iter().position(|e| e.span == usage && matches!(e.kind, ast::ExprKind::Identifier(_)))?;
    let decl = bodies.local_declaration(ast::ExprId(expr as u32))?;
    let list = ast.stmts.iter().find_map(|s| match &s.kind {
        StmtKind::Variables(l) if l.variables.len() == 1 && l.variables[0].name.span.start == decl => Some((s.span, l)),
        _ => None,
    })?;
    let (span, l) = list;
    if !l.final_ {
        return None;
    }
    let snippet = &u.source[span.start..];
    let f = span.start + snippet.find("final")?;
    let name = u.source[decl..].split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).next()?.to_string();
    let edit = match l.ty {
        Some(t) => (Span { start: f, end: ast.ty(t).span.start }, String::new()),
        None => (Span { start: f, end: f + 5 }, "var".to_string()),
    };
    Some((name, vec![edit]))
}

/// The title, the insertion and the relative selection of `Create N missing
/// override(s)`.
type Overrides = (String, (Span, String), Option<(usize, usize)>, Vec<Mark>);

/// A member to override: displayed name, whether it is a getter/setter, and
/// the element.
struct Missing {
    name: String,
    getter: bool,
    setter: bool,
    function: FunctionElementId,
    owner: ClassId,
}

/// `Create N missing override(s)`: the abstract or interface members without
/// a concrete implementation in the chain of `c`, sorted by name (getter
/// before setter; a getter/setter pair becomes a field), inserted after the
/// last member (or right after the `{`). The pairs become fields before the
/// other members, as in Dart's loop (`create_missing_overrides.dart:78-114`).
///
/// The selection (relative to the inserted text): with more than one
/// member, at the start of the first (`selectHere`); with a single one,
/// that of `writeOverride(setSelection: true)` (the whole `throw` body, or
/// right after the `// TODO`); a lone field has no selection.
fn missing_overrides(project: &mut Project, c: ClassId, unit: UnitId, decl: ast::DeclId) -> Option<Overrides> {
    let missing = missing(project, c);
    if missing.is_empty() {
        return None;
    }
    let p = project.program();
    let u = p.unit(unit);
    let d = u.ast.decl(decl);
    let (members, enum_) = match &d.kind {
        DeclKind::Class(k) => (k.members.clone(), false),
        DeclKind::Enum(k) => (k.members.clone(), true),
        _ => return None,
    };
    let source = u.source.clone();
    let last = members.iter().map(|m| u.ast.member(*m).span.end).max();
    let opens = source[d.span.start..d.span.end].find('{').map(|i| d.span.start + i + 1);
    let (position, prefix) = match (last, opens) {
        (Some(f), _) => (f, "\n\n  ".to_string()),
        (None, Some(a)) => (a, "\n  ".to_string()),
        _ => return None,
    };
    let single_line = !source[d.span.start..d.span.end].contains('\n');
    let map = mapping(project, c);
    let mut fields = Vec::new();
    let mut texts = Vec::new();
    let mut n = missing.len();
    let mut i = 0;
    while i < missing.len() {
        let a = &missing[i];
        if a.getter && missing.get(i + 1).is_some_and(|b| b.setter && b.name == a.name) {
            let ty = return_(project, a.function, map.get(&a.owner));
            let final_ = if enum_ { "final " } else { "" };
            fields.push(format!("@override\n  {final_}{ty} {};", a.name));
            n -= 1;
            i += 2;
            continue;
        }
        texts.push(override_(project, a, map.get(&a.owner)));
        i += 1;
    }
    let selection = if n > 1 {
        Some((prefix.len(), 0))
    } else if let [(s, _)] = texts.as_slice()
        && fields.is_empty()
    {
        let body = "throw UnimplementedError()";
        if let Some(k) = s.find(&format!("=> {body};")) {
            Some((prefix.len() + k + 3, body.len()))
        } else if let Some(k) = s.find(&format!("{body};")) {
            Some((prefix.len() + k, body.len() + 1))
        } else {
            s.find("// TODO: implement ").map(|k| {
                let end = s[k..].find('\n').map_or(s.len(), |x| k + x);
                (prefix.len() + end, 0)
            })
        }
    } else {
        None
    };
    // The text: the fields, then the overrides; the parameter groups become
    // valid over the whole text.
    let mut text = prefix.clone();
    let mut groups: Vec<Mark> = Vec::new();
    let parts: Vec<(String, Vec<Mark>)> = fields.into_iter().map(|c| (c, Vec::new())).chain(texts).collect();
    for (k, (s, marks)) in parts.into_iter().enumerate() {
        if k > 0 {
            text.push_str("\n\n  ");
        }
        let base = text.len();
        groups.extend(marks.into_iter().map(|(r, n, suggestions)| (base + r, n, suggestions)));
        text.push_str(&s);
    }
    if last.is_none() && single_line {
        text.push('\n');
    }
    let title = format!("Create {n} missing override{}", if n == 1 { "" } else { "s" });
    Some((title, (Span { start: position, end: position }, text), selection, groups))
}

/// The members without a concrete implementation in the chain of `c`.
fn missing(project: &Project, c: ClassId) -> Vec<Missing> {
    let p = project.program();
    // Implementation chain: the class, the mixins (from the last to the first)
    // and the superclass, recursively.
    let mut chain = Vec::new();
    let mut current = Some(c);
    let mut seen = HashSet::new();
    while let Some(x) = current {
        if !seen.insert(x) {
            break;
        }
        chain.push(x);
        for m in p.class(x).mixin_classes.iter().rev() {
            chain.push(*m);
        }
        current = p.class(x).supertype_class;
    }
    let concrete = |name: &str| {
        chain.iter().any(|x| {
            p.class(*x).instance_members.iter().any(|(s, f)| project.name(*s) == name && !p.function(*f).abstract_)
        })
    };
    let mut supers: Vec<ClassId> = project.supertypes(c).into_iter().collect();
    supers.sort();
    let mut by_name: BTreeMap<String, (FunctionElementId, ClassId)> = BTreeMap::new();
    for s in supers {
        for (sym, f) in &p.class(s).instance_members {
            let name = project.name(*sym).to_string();
            if name == "noSuchMethod" {
                continue;
            }
            by_name.entry(name).or_insert((*f, s));
        }
    }
    let mut output: Vec<Missing> = by_name
        .into_iter()
        .filter(|(name, _)| !concrete(name))
        .map(|(name, (f, owner))| {
            let fe = p.function(f);
            let setter = name.ends_with('=') && name != "==" && !matches!(fe.kind, FunctionKind::Operator);
            let getter = matches!(fe.kind, FunctionKind::Getter) || (fe.kind == FunctionKind::ImplicitAccessor && !setter);
            Missing { name: base_name(&name).to_string(), getter, setter, function: f, owner }
        })
        .collect();
    // By displayed name; getter before setter.
    output.sort_by(|a, b| (a.name.as_str(), !a.getter).cmp(&(b.name.as_str(), !b.getter)));
    output
}

/// For each supertype of `c`, the type arguments as `c` sees it.
fn mapping(project: &mut Project, c: ClassId) -> HashMap<ClassId, HashMap<dartforge_types::TypeParamId, TypeId>> {
    let supers: Vec<ClassId> = project.supertypes(c).into_iter().collect();
    let query = &mut project.query;
    let Some(data) = query.outline.classes.get(c.0 as usize) else { return HashMap::new() };
    let args: Box<[TypeId]> = data
        .type_params
        .clone()
        .iter()
        .map(|tp| query.table.intern(Type::TypeParameter { param: *tp, nullable: false }))
        .collect();
    let this = query.table.intern(Type::Interface { class: c, args, nullable: false });
    let mut output = HashMap::new();
    for s in supers {
        let Some(t) = query.outline.hierarchy.supertype_of(this, s, &mut query.table, &query.core) else { continue };
        let Type::Interface { args, .. } = query.table.get(t).clone() else { continue };
        let Some(ds) = query.outline.classes.get(s.0 as usize) else { continue };
        if ds.type_params.len() == args.len() {
            output.insert(s, ds.type_params.iter().copied().zip(args.iter().copied()).collect());
        }
    }
    output
}

fn format(project: &mut Project, t: TypeId, map: Option<&HashMap<dartforge_types::TypeParamId, TypeId>>) -> String {
    let t = match map {
        Some(m) => dartforge_types::ops::substitute(t, m, &mut project.query.table),
        None => t,
    };
    project.query.format(t)
}

/// The return type (or the getter's type) of `f` as `c` sees it.
fn return_(project: &mut Project, f: FunctionElementId, map: Option<&HashMap<dartforge_types::TypeParamId, TypeId>>) -> String {
    let fe = project.program().function(f);
    let t = match (fe.kind, fe.variable) {
        (FunctionKind::ImplicitAccessor, Some(v)) => project.query.variable_type(v).unwrap_or(project.query.outline.functions[f.0 as usize].return_type),
        _ => project.query.outline.functions[f.0 as usize].return_type,
    };
    format(project, t, map)
}

/// The text of an override (Dart's `writeOverride`).
/// The text of `writeOverride` and the linked groups of the parameters (in
/// bytes of the text).
fn override_(project: &mut Project, a: &Missing, map: Option<&HashMap<dartforge_types::TypeParamId, TypeId>>) -> (String, Vec<Mark>) {
    let name = a.name.clone();
    if a.getter {
        let ty = return_(project, a.function, map);
        let ty = if ty == "dynamic" { String::new() } else { format!("{ty} ") };
        return (format!("@override\n  // TODO: implement {name}\n  {ty}get {name} => throw UnimplementedError();"), Vec::new());
    }
    let (parameters, marks) = parameter_list(project, a.function, map);
    let shift = |base: usize, marks: Vec<Mark>| marks.into_iter().map(|(r, n, s)| (base + r, n, s)).collect::<Vec<_>>();
    if a.setter {
        let before = format!("@override\n  set {name}");
        return (format!("{before}{parameters} {{\n    // TODO: implement {name}\n  }}"), shift(before.len(), marks));
    }
    let ty = return_(project, a.function, map);
    let operator = if project.program().function(a.function).kind == FunctionKind::Operator { "operator " } else { "" };
    let body = if ty == "void" { String::new() } else { "\n    throw UnimplementedError();".to_string() };
    // `writeType` does not write a `dynamic` return.
    let before = if ty == "dynamic" { format!("@override\n  {operator}{name}") } else { format!("@override\n  {ty} {operator}{name}") };
    (format!("{before}{parameters} {{\n    // TODO: implement {name}{body}\n  }}"), shift(before.len(), marks))
}

/// `<T>(int a, [int b = 0], {required int c})` of `f`, with the types as the
/// class sees them.
/// A linked group of a generated text: position and length (bytes of the
/// text) and the suggestions.
pub(crate) type Mark = (usize, usize, Vec<String>);

/// `addSuperTypesAsSuggestions(type)`: for an interface type, the type
/// itself and those of `allSupertypes` (in `implementedInterfaces` order),
/// as Dart displays them; for the others, nothing.
pub(crate) fn supertype_suggestions(project: &mut Project, t: TypeId) -> Vec<String> {
    let Type::Interface { class, args, nullable } = project.query.table.get(t).clone() else { return Vec::new() };
    let mut v = vec![project.query.format(t)];
    let base = project.query.table.intern(Type::Interface { class, args, nullable: false });
    for s in crate::create::all_supertypes(project, class) {
        let query = &mut project.query;
        if let Some(mut st) = query.outline.hierarchy.supertype_of(base, s, &mut query.table, &query.core) {
            // The supertypes of `T?` come out nullable (`Object?`, `Pattern?`).
            if nullable && let Type::Interface { class, args, .. } = query.table.get(st).clone() {
                st = query.table.intern(Type::Interface { class, args, nullable: true });
            }
            v.push(query.format(st));
        }
    }
    v
}

/// `writeParameters(parameters, methodBeingCopied: f)` of `writeOverride`:
/// the text and the linked groups `TYPE<i>` (with the supertype
/// suggestions) and `PARAM<i>` (only for unnamed ones), in bytes of the
/// text.
fn parameter_list(project: &mut Project, f: FunctionElementId, map: Option<&HashMap<dartforge_types::TypeParamId, TypeId>>) -> (String, Vec<Mark>) {
    let p = project.program();
    let (unit, ps, tps): (UnitId, Vec<(ParameterKind, bool, Option<String>, Option<String>)>, String) = match p.function(f).node {
        FunctionRef::Function { unit, function } => {
            let u = p.unit(unit);
            let func = u.ast.function(function);
            let ps = func
                .parameters
                .iter()
                .flatten()
                .map(|x| {
                    let name = x.public_name.or(x.name).map(|n| u.source[n.span.start..n.span.end].to_string());
                    let pattern = x.default_value.map(|e| {
                        let s = u.ast.expr(e).span;
                        u.source[s.start..s.end].to_string()
                    });
                    (x.kind, x.required, name, pattern)
                })
                .collect();
            let tps = if func.type_params.is_empty() {
                String::new()
            } else {
                format!("<{}>", func.type_params.iter().map(|t| &u.source[t.span.start..t.span.end]).collect::<Vec<_>>().join(", "))
            };
            (unit, ps, tps)
        }
        _ => {
            // Implicit setter of a field: one parameter with the field's type.
            let ps = vec![(ParameterKind::Required, false, Some("value".to_string()), None)];
            (UnitId(0), ps, String::new())
        }
    };
    let _ = unit;
    let types: Vec<TypeId> = project.query.outline.functions[f.0 as usize].parameters.iter().map(|q| q.ty).collect();
    // Each parameter: (kind, text, marks relative to its text).
    let mut required: Vec<(String, Vec<Mark>)> = Vec::new();
    let mut optional: Vec<(String, Vec<Mark>)> = Vec::new();
    let mut named: Vec<(String, Vec<Mark>)> = Vec::new();
    for (i, (type_p, req, name, pattern)) in ps.into_iter().enumerate() {
        let mut s = String::new();
        let mut marks = Vec::new();
        if type_p == ParameterKind::Named && req {
            s.push_str("required ");
        }
        let ty = types.get(i).map_or_else(|| "dynamic".to_string(), |t| format(project, *t, map));
        let suggestions = match types.get(i) {
            Some(t) => {
                let t = match map {
                    Some(m) => dartforge_types::ops::substitute(*t, m, &mut project.query.table),
                    None => *t,
                };
                supertype_suggestions(project, t)
            }
            None => Vec::new(),
        };
        // `_writeType` does not write `dynamic` (nor the invalid type) for a
        // parameter: without a type, without the group.
        let dynamic = types.get(i).is_some_and(|t| matches!(project.query.table.get(*t), Type::Dynamic));
        if !dynamic {
            marks.push((s.len(), ty.len(), suggestions));
            s.push_str(&ty);
            s.push(' ');
        }
        let name = name.unwrap_or_else(|| format!("p{i}"));
        if type_p != ParameterKind::Named {
            marks.push((s.len(), name.len(), Vec::new()));
        }
        s.push_str(&name);
        if let Some(v) = pattern {
            s.push_str(&format!(" = {v}"));
        }
        match type_p {
            ParameterKind::Required => required.push((s, marks)),
            ParameterKind::Optional => optional.push((s, marks)),
            ParameterKind::Named => named.push((s, marks)),
        }
    }
    let mut text = format!("{tps}(");
    let mut all: Vec<Mark> = Vec::new();
    let mut join = |text: &mut String, list: Vec<(String, Vec<Mark>)>, opens: &str, closes: &str, first: &mut bool| {
        for (k, (s, marks)) in list.into_iter().enumerate() {
            if !*first {
                text.push_str(", ");
            }
            *first = false;
            if k == 0 {
                text.push_str(opens);
            }
            let base = text.len();
            all.extend(marks.into_iter().map(|(r, n, suggestions)| (base + r, n, suggestions)));
            text.push_str(&s);
        }
        text.push_str(closes);
    };
    let mut first = true;
    join(&mut text, required, "", "", &mut first);
    if !optional.is_empty() {
        join(&mut text, optional, "[", "]", &mut first);
    }
    if !named.is_empty() {
        join(&mut text, named, "{", "}", &mut first);
    }
    text.push(')');
    (text, all)
}

