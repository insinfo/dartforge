//! The thirty-second batch of lint rules
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), written directly from the 3.6.2
//! emitters (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//!
//! * `unsafe_html`: the assignment to `href` (of `AnchorElement`), `src` (of
//!   `EmbedElement`, `IFrameElement`, `ScriptElement`) and `srcdoc` (of
//!   `IFrameElement`), the call of `createFragment`/`setInnerHtml` (of
//!   `Element`) and of `open` (of `Window`), also with a `dynamic` target,
//!   and the `html` constructor of `DocumentFragment`/`Element`, along the
//!   superclass chain (`extendsClass(…, 'dart.dom.html')`). The codes are the
//!   emitter's own `SecurityLintCode`s (messages different from those of
//!   `messages.yaml`).
//! * `analyzer_use_new_elements` (internal): only in the package with
//!   `analyzer_use_new_elements.txt` at the root and in the library whose
//!   file starts with one of its prefixes; the named type whose element is a
//!   declaration of `package:analyzer/dart/element/element.dart` (except
//!   `DirectiveUri`, `DirectiveUriWithRelativeUri`, `ElementAnnotation` and
//!   `ElementKind`), and the identifier (outside the name of an invoked
//!   method) or the method call whose static type contains such an interface
//!   type.
//!
//! Written without compiling or running (2026-10-05).

use super::rules::LintReport;
use super::rules16::is_method_invocation;
use super::LintCode;
use crate::Unit;
use dartforge_elements::model::{ClassId, ClassKind};
use dartforge_frontend::ast::{ExprId, ExprKind};
use dartforge_intern::Interner;
use dartforge_types::resolved::Resolved;
use dartforge_types::table::{Type, TypeId};

/// `_Visitor.unsafeAttributeCode`.
pub static UNSAFE_HTML_ATTRIBUTE: LintCode = LintCode {
    name: "unsafe_html",
    unique: "unsafe_html_attribute",
    message: "Avoid unsafe HTML APIs (assigning \"{0}\" attribute).",
    correction: None,
    documented: false,
};

/// `_Visitor.unsafeMethodCode`.
pub static UNSAFE_HTML_METHOD: LintCode = LintCode {
    name: "unsafe_html",
    unique: "unsafe_html_method",
    message: "Avoid unsafe HTML APIs (calling the '{0}' method of {1}).",
    correction: None,
    documented: false,
};

/// `_Visitor.unsafeConstructorCode`.
pub static UNSAFE_HTML_CONSTRUCTOR: LintCode = LintCode {
    name: "unsafe_html",
    unique: "unsafe_html_constructor",
    message: "Avoid unsafe HTML APIs (calling the '{0}' constructor of {1}).",
    correction: None,
    documented: false,
};

/// `AnalyzerUseNewElements.code`.
pub static ANALYZER_USE_NEW_ELEMENTS: LintCode = LintCode {
    name: "analyzer_use_new_elements",
    unique: "analyzer_use_new_elements",
    message: "This code uses the old analyzer element model.",
    correction: Some("Try using the new elements."),
    documented: false,
};

/// The type of a target: `Some(None)` for `dynamic`, `Some(Some(class))` for
/// an interface type.
#[derive(Clone, Copy)]
enum TargetType {
    Dynamic,
    Class(ClassId),
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unit<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let Some(s) = sem else { return out };
    if enabled("analyzer_use_new_elements") {
        out.extend(new_elements_rule(u, interner, s));
    }
    if !enabled("unsafe_html") {
        return out;
    }
    let a = u.ast;
    let program = s.program;
    let table = s.table;
    let body = s.body;
    // `extendsDartHtmlClass`: the class or a superclass is `name` of the
    // `dart.dom.html` library.
    let library_name = |c: ClassId| -> Option<String> {
        program.library(program.class(c).library).name.as_ref().map(|n| n.iter().map(|x| interner.resolve(*x)).collect::<Vec<_>>().join("."))
    };
    let extends_html = |recv_ty: TargetType, name: &str| -> bool {
        let TargetType::Class(mut c) = recv_ty else { return false };
        let mut seen: Vec<ClassId> = Vec::new();
        loop {
            if seen.contains(&c) {
                return false;
            }
            seen.push(c);
            if interner.resolve(program.class(c).name) == name && library_name(c).as_deref() == Some("dart.dom.html") {
                return true;
            }
            match program.class(c).supertype_class {
                Some(x) => c = x,
                None => return false,
            }
        }
    };
    let dynamic_or = |recv_ty: TargetType, name: &str| matches!(recv_ty, TargetType::Dynamic) || extends_html(recv_ty, name);
    let target_type_of = |t: TypeId| -> Option<TargetType> {
        match table.get(t) {
            Type::Dynamic => Some(TargetType::Dynamic),
            Type::Interface { class, .. } => Some(TargetType::Class(*class)),
            _ => None,
        }
    };
    // The cascades: the `realTarget` of a section is the cascade's target.
    let mut cascades: std::collections::HashMap<ExprId, ExprId> = std::collections::HashMap::new();
    for e in a.exprs.iter() {
        if let ExprKind::Cascade { target, sections, .. } = &e.kind {
            for sec in sections.iter() {
                let mut x = *sec;
                loop {
                    match &a.expr(x).kind {
                        ExprKind::CascadeTarget => {
                            cascades.insert(x, *target);
                            break;
                        }
                        ExprKind::Property { target, .. } | ExprKind::Index { target, .. } | ExprKind::Call { target, .. } | ExprKind::TypeArguments { target, .. } => x = *target,
                        ExprKind::Assign { target, .. } => x = *target,
                        _ => break,
                    }
                }
            }
        }
    }
    let real_target_type = |x: ExprId| -> Option<TargetType> {
        let x = cascades.get(&x).copied().unwrap_or(x);
        body.get_type(x).and_then(target_type_of)
    };
    // The `thisType` of the class that contains the resolved member (only a `class`).
    let member_class_type = |e: ExprId| -> Option<TargetType> {
        match body.get_resolved(e)? {
            Resolved::Member { class, .. } if matches!(program.class(*class).kind, ClassKind::Class | ClassKind::MixinApplication) => Some(TargetType::Class(*class)),
            _ => None,
        }
    };
    let mut findings: Vec<(dartforge_diagnostics::Span, &'static LintCode, Vec<String>)> = Vec::new();
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        match &e.kind {
            // `visitAssignmentExpression`.
            ExprKind::Assign { target, .. } => {
                let mut left = *target;
                while let ExprKind::Parenthesized(x) = &a.expr(left).kind {
                    left = *x;
                }
                let (recv_ty, name) = match &a.expr(left).kind {
                    ExprKind::Identifier(n) => (member_class_type(left), n.sym),
                    ExprKind::Property { target: t, name, .. } => (real_target_type(*t), name.sym),
                    _ => continue,
                };
                let Some(recv_ty) = recv_ty else { continue };
                let n = interner.resolve(name);
                let is_unsafe = match n {
                    "href" => dynamic_or(recv_ty, "AnchorElement"),
                    "src" => matches!(recv_ty, TargetType::Dynamic) || extends_html(recv_ty, "EmbedElement") || extends_html(recv_ty, "IFrameElement") || extends_html(recv_ty, "ScriptElement"),
                    "srcdoc" => dynamic_or(recv_ty, "IFrameElement"),
                    _ => false,
                };
                if is_unsafe {
                    findings.push((e.span, &UNSAFE_HTML_ATTRIBUTE, vec![n.to_string()]));
                }
            }
            // `visitInstanceCreationExpression` (with or without `new`).
            ExprKind::InstanceCreation { constructor: Some(c), .. } if interner.resolve(c.sym) == "html" => {
                let Some(recv_ty) = body.get_type(id).and_then(target_type_of) else { continue };
                if extends_html(recv_ty, "DocumentFragment") {
                    findings.push((e.span, &UNSAFE_HTML_CONSTRUCTOR, vec!["html".to_string(), "DocumentFragment".to_string()]));
                } else if extends_html(recv_ty, "Element") {
                    findings.push((e.span, &UNSAFE_HTML_CONSTRUCTOR, vec!["html".to_string(), "Element".to_string()]));
                }
            }
            ExprKind::Call { target, .. } => {
                if let Some(Resolved::Constructor(f)) = body.get_resolved(id) {
                    if interner.resolve(program.function(*f).name) != "html" {
                        continue;
                    }
                    let Some(recv_ty) = body.get_type(id).and_then(target_type_of) else { continue };
                    if extends_html(recv_ty, "DocumentFragment") {
                        findings.push((e.span, &UNSAFE_HTML_CONSTRUCTOR, vec!["html".to_string(), "DocumentFragment".to_string()]));
                    } else if extends_html(recv_ty, "Element") {
                        findings.push((e.span, &UNSAFE_HTML_CONSTRUCTOR, vec!["html".to_string(), "Element".to_string()]));
                    }
                    continue;
                }
                // `visitMethodInvocation`.
                if !is_method_invocation(s, a, *target) {
                    continue;
                }
                let (recv_ty, name) = match &a.expr(*target).kind {
                    ExprKind::Identifier(n) => (member_class_type(*target), n.sym),
                    ExprKind::Property { target: t, name, .. } => (real_target_type(*t), name.sym),
                    _ => continue,
                };
                let Some(recv_ty) = recv_ty else { continue };
                let n = interner.resolve(name);
                let (is_unsafe, owner_class) = match n {
                    "createFragment" | "setInnerHtml" => (dynamic_or(recv_ty, "Element"), "Element"),
                    "open" => (dynamic_or(recv_ty, "Window"), "Window"),
                    _ => (false, ""),
                };
                if is_unsafe {
                    findings.push((e.span, &UNSAFE_HTML_METHOD, vec![n.to_string(), owner_class.to_string()]));
                }
            }
            _ => {}
        }
    }
    for (span, code, args) in findings {
        out.push(LintReport { code, span, args });
    }
    out
}

/// `_isOldModelElement`: the class declared in the unit
/// `package:analyzer/dart/element/element.dart`, except those that do not migrate.
fn is_old_model_element(s: &super::Semantics<'_>, interner: &Interner, c: ClassId) -> bool {
    let k = s.program.class(c);
    let Some(r) = k.decl else { return false };
    if s.program.unit(r.unit).uri != "package:analyzer/dart/element/element.dart" {
        return false;
    }
    !matches!(interner.resolve(k.name), "DirectiveUri" | "DirectiveUriWithRelativeUri" | "ElementAnnotation" | "ElementKind")
}

/// `_isOldModelType`: the `RecursiveTypeVisitor` over interface types.
fn is_old_model_type(s: &super::Semantics<'_>, interner: &Interner, t: TypeId, depth: u32) -> bool {
    if depth > 32 {
        return false;
    }
    let r = |x: TypeId| is_old_model_type(s, interner, x, depth + 1);
    match s.table.get(t) {
        Type::Interface { class, args, .. } => is_old_model_element(s, interner, *class) || args.iter().any(|x| r(*x)),
        Type::ExtensionType { args, .. } => args.iter().any(|x| r(*x)),
        Type::FutureOr { arg, .. } => r(*arg),
        Type::Function { ret, positional, optional, named, .. } => r(*ret) || positional.iter().chain(optional.iter()).any(|x| r(*x)) || named.iter().any(|(_, x, _)| r(*x)),
        Type::Record { positional, named, .. } => positional.iter().any(|x| r(*x)) || named.iter().any(|(_, x)| r(*x)),
        _ => false,
    }
}

/// `analyzer_use_new_elements`.
fn new_elements_rule(u: Unit<'_>, interner: &Interner, s: &super::Semantics<'_>) -> Vec<LintReport> {
    let mut out = Vec::new();
    let program = s.program;
    // `_isEnabledForFile`: the pub package, the option file and the prefixes.
    let lib = program.library(program.unit(s.unit).library);
    let Some(file) = lib.units.first().and_then(|x| program.unit(*x).path.clone()) else { return out };
    let Some(root) = file.ancestors().skip(1).find(|d| d.join("pubspec.yaml").is_file()).map(std::path::Path::to_path_buf) else { return out };
    let Ok(text) = std::fs::read_to_string(root.join("analyzer_use_new_elements.txt")) else { return out };
    let separator = std::path::MAIN_SEPARATOR.to_string();
    let prefixes: Vec<String> = text.trim().split('\n').map(|l| l.trim().replace('/', &separator)).collect();
    let path = file.to_string_lossy().to_string();
    let base = root.to_string_lossy().to_string();
    if !path.starts_with(&base) || path.len() <= base.len() {
        return out;
    }
    let relative = &path[base.len() + 1..];
    if !prefixes.iter().any(|p| relative.starts_with(p.as_str())) {
        return out;
    }
    let a = u.ast;
    // `visitNamedType`.
    for t in a.types.iter() {
        let dartforge_frontend::ast::TypeKind::Named { name, .. } = &t.kind else { continue };
        let b = match &name[..] {
            [n] => program.lookup_in_unit(s.unit, n.sym),
            [p, n] => program.lookup_prefixed_in_unit(s.unit, p.sym, n.sym),
            _ => None,
        };
        if let Some(dartforge_elements::model::Element::Class(c)) = b.and_then(|b| b.getter)
            && is_old_model_element(s, interner, c)
            && let Some(last) = name.last()
        {
            out.push(LintReport { code: &ANALYZER_USE_NEW_ELEMENTS, span: last.span, args: Vec::new() });
        }
    }
    // The invoked method names: only through `visitMethodInvocation`.
    let mut method_names: std::collections::HashSet<ExprId> = std::collections::HashSet::new();
    for (k, e) in a.exprs.iter().enumerate() {
        let ExprKind::Call { target, .. } = &e.kind else { continue };
        if matches!(s.body.get_resolved(ExprId(k as u32)), Some(Resolved::Constructor(_))) || !is_method_invocation(s, a, *target) {
            continue;
        }
        method_names.insert(*target);
        if s.body.get_type(ExprId(k as u32)).is_some_and(|t| is_old_model_type(s, interner, t, 0)) {
            let sp = match &a.expr(*target).kind {
                ExprKind::Property { name, .. } => name.span,
                _ => a.expr(*target).span,
            };
            out.push(LintReport { code: &ANALYZER_USE_NEW_ELEMENTS, span: sp, args: Vec::new() });
        }
    }
    // `visitSimpleIdentifier`: the identifier and the property name.
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        if method_names.contains(&id) {
            continue;
        }
        let sp = match &e.kind {
            ExprKind::Identifier(n) => n.span,
            ExprKind::Property { name, .. } => name.span,
            _ => continue,
        };
        if s.body.get_type(id).is_some_and(|t| is_old_model_type(s, interner, t, 0)) {
            out.push(LintReport { code: &ANALYZER_USE_NEW_ELEMENTS, span: sp, args: Vec::new() });
        }
    }
    out.sort_by_key(|r| (r.span.start, r.span.end));
    out
}
