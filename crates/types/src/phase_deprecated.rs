//! The analyzer's `DeprecatedMemberUseVerifier`
//! (`analyzer/lib/src/error/deprecated_member_use_verifier.dart`, read in
//! full in 3.6.2; docs/ANALYZER-ESPECIFICACAO-INFRA.md, batch II.8):
//! `deprecated_member_use` and `deprecated_member_use_from_same_package`,
//! with and without a message, on the use of an element annotated with the
//! `deprecated` or the `Deprecated(…)` of `dart:core`; and the lint
//! `deprecated_member_use_from_same_package`, which is the same verifier
//! with the linter's visitor (without the pattern fields, and with the
//! simple and field required parameters also opening a deprecated region),
//! only for the package's elements.
//!
//! Covers, like `BaseDeprecatedMemberUseVerifier`: identifiers (outside a
//! declaration, the name of a `ConstructorName`, the name of
//! `super.the_name(…)` and of `hide`; those of `show` too), the left-hand side
//! of an assignment and the operand of `++`/`--` (through the read and
//! write elements), operators (binary, index, prefix, postfix, compound
//! assignment), the `ConstructorName` (creation, also without `new`, and
//! tear-off), named types, object pattern fields (only in the warning), the
//! implicit `call`, the `super(…)`/`this(…)`, the arguments passed to
//! non-required deprecated parameters (outside the function itself), and
//! imports and exports of a deprecated library. A use inside a deprecated
//! declaration is not reported; in a deprecated library, nothing is.
//! The message is the `message` of the `Deprecated` (the string literal, or
//! that of a top-level constant that holds it).
//!
//! Written without compiling or running (2026-10-04; redone on 2026-10-05).

use crate::typed_lints::Finding;
use crate::resolved::{MemberRef, Resolved, UnitBodyTypes};
use crate::table::{Type, TypeTable};
use dartforge_diagnostics::codes::hint as h;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{Element, FunctionElementId, FunctionKind, FunctionRef, LibraryId, Program, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, Combinator, DeclKind, DirectiveKind, ExprId, ExprKind, Initializer, MemberKind, ParameterKind, PatternKind, TypeKind, UnaryOp};
use dartforge_intern::Interner;

/// The text of a constant string expression: the literal, or the literal
/// initializer of a top-level constant.
fn constant_text(program: &Program, u: UnitId, e: ExprId, prof: u32) -> Option<String> {
    let a = &program.unit(u).ast;
    match &a.expr(e).kind {
        ExprKind::String(lit) => dartforge_elements::load::string_lit_value(lit),
        ExprKind::Parenthesized(x) => constant_text(program, u, *x, prof),
        ExprKind::Identifier(n) if prof < 8 => {
            let Some(Element::Variable(v)) = program.lookup_in_unit(u, n.sym).and_then(|b| b.getter) else { return None };
            let x = program.variable(v);
            if !x.const_ {
                return None;
            }
            let VariableRef::TopLevel { unit, decl, index } = x.node else { return None };
            let DeclKind::Variables(l) = &program.unit(unit).ast.decl(decl).kind else { return None };
            constant_text(program, unit, l.variables.get(index)?.initializer?, prof + 1)
        }
        _ => None,
    }
}

/// `ElementAnnotation.isDeprecated` of annotation `m` (written in unit
/// `u`): `Some(message)` when it is the `deprecated` getter (without a
/// message) or the constructor of `Deprecated` of `dart:core`.
fn deprecated_annotation(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation) -> Option<Option<String>> {
    let from_core = |b: Option<dartforge_elements::model::Binding>| -> Option<Element> {
        let e = b?.getter?;
        let lib = match e {
            Element::Variable(v) => program.variable(v).library,
            Element::Class(c) => program.class(c).library,
            _ => return None,
        };
        (program.library(lib).uri == "dart:core").then_some(e)
    };
    // The named constructor (`Deprecated.the_name(…)`): it only counts if it exists
    // in the class; otherwise the annotation does not resolve
    // (`INVALID_ANNOTATION`, without `isDeprecated`), like
    // `@Deprecated.extend()` in 3.6.2.
    let (element, the_name, the_constructor) = match &m.name[..] {
        [n] => (from_core(program.lookup_in_unit(u, n.sym)), n.sym, None),
        [p, n] => match from_core(program.lookup_prefixed_in_unit(u, p.sym, n.sym)) {
            Some(e) => (Some(e), n.sym, None),
            // `Deprecated.new(…)`, `Deprecated.the_name(…)`.
            None => (from_core(program.lookup_in_unit(u, p.sym)), p.sym, Some(n.sym)),
        },
        [p, c, k] => (from_core(program.lookup_prefixed_in_unit(u, p.sym, c.sym)), c.sym, Some(k.sym)),
        _ => return None,
    };
    if let (Some(Element::Class(c)), Some(k)) = (element, the_constructor) {
        let key = if interner.resolve(k) == "new" { interner.lookup("") } else { Some(k) };
        if !key.is_some_and(|ch| program.class(c).constructors.contains_key(&ch)) {
            return None;
        }
    }
    let text = interner.resolve(the_name);
    match element? {
        Element::Variable(_) if text == "deprecated" && m.arguments.is_none() => Some(None),
        Element::Class(_) if text == "Deprecated" => {
            let args = m.arguments.as_ref()?;
            let message = args
                .args
                .iter()
                .find(|x| x.name.is_none())
                .or_else(|| args.args.iter().find(|x| x.name.is_some_and(|n| interner.resolve(n.sym) == "message")))
                .and_then(|x| constant_text(program, u, x.value, 0));
            Some(message)
        }
        _ => None,
    }
}

/// `Some(message)` if the list has a deprecation annotation (the message of
/// the first one).
fn deprecated(program: &Program, interner: &Interner, u: UnitId, metadata: &[ast::Annotation]) -> Option<Option<String>> {
    metadata.iter().find_map(|m| deprecated_annotation(program, interner, u, m))
}

/// The annotations of a function, method or constructor, with their unit.
pub(crate) fn of_function(program: &Program, f: FunctionElementId) -> Option<(&[ast::Annotation], UnitId)> {
    let e = program.function(f);
    // Implicit accessor: the annotations are those of the variable.
    if let Some(v) = e.variable
        && matches!(e.node, FunctionRef::None)
    {
        return of_variable(program, v);
    }
    match e.node {
        FunctionRef::Constructor { unit, member } => Some((&program.unit(unit).ast.member(member).metadata[..], unit)),
        FunctionRef::Function { unit, function } => {
            let a = &program.unit(unit).ast;
            if let Some(m) = a.members.iter().find(|m| matches!(&m.kind, MemberKind::Method(g) if *g == function)) {
                return Some((&m.metadata[..], unit));
            }
            a.decls.iter().find(|d| matches!(&d.kind, DeclKind::Function(g) if *g == function)).map(|d| (&d.metadata[..], unit))
        }
        FunctionRef::None => e.variable.and_then(|v| of_variable(program, v)),
    }
}

pub(crate) fn of_variable(program: &Program, v: VariableId) -> Option<(&[ast::Annotation], UnitId)> {
    match program.variable(v).node {
        VariableRef::TopLevel { unit, decl, .. } => Some((&program.unit(unit).ast.decl(decl).metadata[..], unit)),
        VariableRef::Field { unit, member, .. } => Some((&program.unit(unit).ast.member(member).metadata[..], unit)),
        VariableRef::EnumConstant { unit, decl, index } => match &program.unit(unit).ast.decl(decl).kind {
            DeclKind::Enum(x) => x.constants.get(index).map(|k| (&k.metadata[..], unit)),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn of_element(program: &Program, e: Element) -> Option<(&[ast::Annotation], UnitId)> {
    let of_declaration = |d: dartforge_elements::model::DeclRef| Some((&program.unit(d.unit).ast.decl(d.decl).metadata[..], d.unit));
    match e {
        Element::Class(c) => program.class(c).decl.and_then(of_declaration),
        Element::Typedef(t) => of_declaration(program.typedef(t).decl),
        Element::Extension(x) => of_declaration(program.extension(x).decl),
        Element::Function(f) => of_function(program, f),
        Element::Variable(v) => of_variable(program, v),
        Element::Prefix(..) => None,
    }
}

/// The library is deprecated: its `library` directive has the annotation.
fn deprecated_library(program: &Program, interner: &Interner, lib: LibraryId) -> Option<Option<String>> {
    let first = *program.library(lib).units.first()?;
    let unit_id = program.unit(first);
    let directive = unit_id.unit.directives.iter().find(|d| matches!(d.kind, DirectiveKind::Library { .. }))?;
    deprecated(program, interner, first, &directive.metadata)
}

/// Who visits: the `BestPracticesVerifier` (the warning) or the lint visitor.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Warning,
    Lint,
}

/// A reportable use: the node, the display name, the message and the
/// element's library.
struct Use {
    span: Span,
    name: String,
    message: Option<String>,
    lib: LibraryId,
}

/// A deprecatable element: its metadata, the unit, the library and the
/// display name.
struct Target<'p> {
    metadata: &'p [ast::Annotation],
    unit: UnitId,
    name: String,
}

/// The core of the verifier over unit `u`.
fn verify(program: &Program, interner: &Interner, table: &TypeTable, body: Option<&UnitBodyTypes>, u: UnitId, mode: Mode) -> Vec<Use> {
    let mut out: Vec<Use> = Vec::new();
    // Without either of the two interned annotations, nothing to do.
    if interner.lookup("deprecated").is_none() && interner.lookup("Deprecated").is_none() {
        return out;
    }
    let unit_id = program.unit(u);
    let a = &unit_id.ast;
    // `pushInDeprecatedValue(library.hasDeprecated)`.
    if deprecated_library(program, interner, unit_id.library).is_some() {
        return out;
    }
    let dep = |metadata: &[ast::Annotation]| deprecated(program, interner, u, metadata).is_some();
    // `_inDeprecatedMemberStack`: the deprecated declarations, members and
    // parameters of this unit.
    let mut regions: Vec<Span> = Vec::new();
    regions.extend(a.decls.iter().filter(|d| dep(&d.metadata)).map(|d| d.span));
    regions.extend(a.members.iter().filter(|m| dep(&m.metadata)).map(|m| m.span));
    {
        let mut lists: Vec<&[ast::Parameter]> = Vec::new();
        for f in a.functions.iter() {
            if let Some(ps) = f.parameters.as_deref() {
                lists.push(ps);
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Constructor(k) = &m.kind {
                lists.push(&k.parameters);
            }
        }
        for ps in lists {
            for p in ps {
                // The `DefaultFormalParameter` in both; the required simple and field
                // parameters only in the lint.
                let optional = p.kind != ParameterKind::Required;
                let simple_or_field = !p.super_ && p.function_parameters.is_none();
                if (optional || (mode == Mode::Lint && simple_or_field)) && dep(&p.metadata) {
                    regions.push(p.span);
                }
            }
        }
    }
    let in_deprecated = |s: Span| regions.iter().any(|r| r.start <= s.start && s.end <= r.end);
    let mut report = |span: Span, tgt: Target<'_>| {
        if in_deprecated(span) {
            return;
        }
        let Some(message) = deprecated(program, interner, tgt.unit, tgt.metadata) else { return };
        let lib = program.unit(tgt.unit).library;
        // The analyzer's listener is a set: one report per position.
        if !out.iter().any(|x| x.span == span && x.name == tgt.name) {
            out.push(Use { span, name: tgt.name, message, lib });
        }
    };
    let text = |s: dartforge_intern::SymbolId| interner.resolve(s).to_string();
    // The display name of a function (that of a setter without `=`).
    let function_name = |f: FunctionElementId| -> String {
        let k = program.function(f);
        if matches!(k.kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor) {
            let cls = k.class.map_or(String::new(), |c| text(program.class(c).name));
            let ctor = interner.resolve(k.name);
            return if ctor.is_empty() { format!("{cls}.new") } else { format!("{cls}.{ctor}") };
        }
        let t = interner.resolve(k.name);
        t.strip_suffix("_=").or_else(|| t.strip_suffix('=')).unwrap_or(t).to_string()
    };
    let function_target = |f: FunctionElementId| of_function(program, f).map(|(metadata, unit_id)| Target { metadata, unit: unit_id, name: function_name(f) });
    let element_target = |e: Element| -> Option<Target<'_>> {
        let the_name = match e {
            Element::Class(c) => text(program.class(c).name),
            Element::Typedef(t) => text(program.typedef(t).name),
            Element::Extension(x) => program.extension(x).name.map_or(String::new(), text),
            Element::Function(f) => return function_target(f),
            Element::Variable(v) => text(program.variable(v).name),
            Element::Prefix(..) => return None,
        };
        of_element(program, e).map(|(metadata, unit_id)| Target { metadata, unit: unit_id, name: the_name })
    };
    let variable_target = |v: VariableId| of_variable(program, v).map(|(metadata, unit_id)| Target { metadata, unit: unit_id, name: text(program.variable(v).name) });
    // The element of a resolution (locals and parameters are not included:
    // `_isLocalParameter` silences the parameter inside its own function).
    let resolved_target = |r: &Resolved| -> Option<Target<'_>> {
        match r {
            Resolved::Constructor(f) => function_target(*f),
            Resolved::Element(e) => element_target(*e),
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => function_target(*f),
            Resolved::Member { member: MemberRef::Variable(v), .. } => variable_target(*v),
            _ => None,
        }
    };
    // The getter that accompanies a written setter (the `readElement`).
    let read = |r: &Resolved| -> Option<Target<'_>> {
        let f = match r {
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } | Resolved::Element(Element::Function(f)) => *f,
            _ => return None,
        };
        let g = program.function(f);
        if g.kind != FunctionKind::Setter || g.variable.is_some() {
            return None;
        }
        let t = interner.resolve(g.name);
        let the_name = interner.lookup(t.strip_suffix("_=").unwrap_or(t))?;
        let getter = match (g.class, g.extension) {
            (Some(c), _) => program.class(c).instance_members.get(&the_name).or_else(|| program.class(c).static_members.get(&the_name)).copied(),
            (None, Some(e)) => program.extension(e).instance_members.get(&the_name).or_else(|| program.extension(e).static_members.get(&the_name)).copied(),
            _ => match program.lookup(g.library, the_name).and_then(|b| b.getter) {
                Some(Element::Function(h)) => Some(h),
                _ => None,
            },
        }?;
        function_target(getter)
    };

    // 1. Named types (`namedType`): at the last name.
    for (k, t) in a.types.iter().enumerate() {
        let TypeKind::Named { name, .. } = &t.kind else { continue };
        let binding = match &name[..] {
            [n] => program.lookup_in_unit(u, n.sym),
            [p, n] => program.lookup_prefixed_in_unit(u, p.sym, n.sym),
            _ => None,
        };
        // A type parameter of the same name hides the top-level declaration.
        if body
            .and_then(|c| c.annotation_types.get(&ast::TypeId(k as u32)).copied())
            .is_some_and(|x| matches!(table.get(x), Type::TypeParameter { .. }))
        {
            continue;
        }
        let (Some(element), Some(last)) = (binding.and_then(|b| b.getter), name.last()) else { continue };
        if matches!(element, Element::Class(_) | Element::Typedef(_))
            && let Some(tgt) = element_target(element)
        {
            report(last.span, tgt);
        }
    }

    // 2. Imports and exports of a deprecated library (the whole directive, with
    // the URI as the name) and the names in `show`.
    let library = program.library(unit_id.library);
    let targets = library
        .imports
        .iter()
        .filter(|i| i.unit == u)
        .map(|i| (i.directive, i.library))
        .chain(library.exports.iter().filter(|e| e.unit == u).map(|e| (e.directive, e.library)));
    for (idx, tgt) in targets {
        let Some(directive) = unit_id.unit.directives.get(idx) else { continue };
        // The deprecated library: the metadata of its `library` directive.
        if let Some(first) = program.library(tgt).units.first().copied()
            && let Some(d) = program.unit(first).unit.directives.iter().find(|d| matches!(d.kind, DirectiveKind::Library { .. }))
        {
            report(directive.span, Target { metadata: &d.metadata, unit: first, name: program.library(tgt).uri.clone() });
        }
        let the_combinators = match &directive.kind {
            DirectiveKind::Import { combinators, .. } | DirectiveKind::Export { combinators, .. } => combinators,
            _ => continue,
        };
        for c in the_combinators.iter() {
            let Combinator::Show(names) = c else { continue };
            for n in names {
                let Some(b) = program.library(tgt).exported.get(&n.sym) else { continue };
                if let Some(e) = b.getter.or(b.setter)
                    && let Some(x) = element_target(e)
                {
                    report(n.span, x);
                }
            }
        }
    }

    let Some(body) = body else { return out };

    // The left-hand sides of assignments and the operands of `++`/`--`
    // (without `staticElement`).
    let mut written_ones: std::collections::HashSet<ExprId> = std::collections::HashSet::new();
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::Assign { target, .. } => {
                written_ones.insert(*target);
            }
            ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } => {
                written_ones.insert(*operand);
            }
            _ => {}
        }
    }
    // The name of a write node: the identifier, or the property name.
    let node_name = |x: ExprId| match &a.expr(x).kind {
        ExprKind::Property { name, .. } => name.span,
        _ => a.expr(x).span,
    };
    // The operator resolved on the node itself (`staticElement`).
    let operator = |e: ExprId| -> Option<Target<'_>> {
        match body.get_resolved(e)? {
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => function_target(*f),
            _ => None,
        }
    };
    // `_invocationArguments`: the arguments passed to non-required deprecated
    // parameters, outside the function that declares them.
    let args_list = |f: FunctionElementId, args: &ast::Arguments, the_call: Span, report: &mut dyn FnMut(Span, Target<'_>)| {
        let (ps, pu, owner) = match program.function(f).node {
            FunctionRef::Function { unit, function } => {
                let x = program.unit(unit).ast.function(function);
                (x.parameters.as_deref().unwrap_or(&[]), unit, x.span)
            }
            FunctionRef::Constructor { unit, member } => match &program.unit(unit).ast.member(member).kind {
                MemberKind::Constructor(k) => (&k.parameters[..], unit, program.unit(unit).ast.member(member).span),
                _ => return,
            },
            FunctionRef::None => return,
        };
        let local = pu == u && owner.start <= the_call.start && the_call.end <= owner.end;
        if local {
            return;
        }
        let mut positional_one = 0;
        for x in args.args.iter() {
            let p = match x.name {
                Some(n) => ps.iter().find(|p| p.kind == ParameterKind::Named && p.external_name().is_some_and(|m| m.sym == n.sym)),
                None => {
                    let p = ps.get(positional_one);
                    positional_one += 1;
                    p.filter(|p| p.kind != ParameterKind::Named)
                }
            };
            let Some(p) = p else { continue };
            if p.kind == ParameterKind::Required || p.required {
                continue;
            }
            let Some(the_name) = p.name else { continue };
            let span = match x.name {
                Some(n) => n.span,
                None => a.expr(x.value).span,
            };
            report(span, Target { metadata: &p.metadata, unit: pu, name: text(the_name.sym) });
        }
    };
    // The element invoked by a method call.
    let invoked = |tgt: ExprId| -> Option<FunctionElementId> {
        match body.get_resolved(tgt)? {
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } | Resolved::Element(Element::Function(f)) => {
                (program.function(*f).kind != FunctionKind::Getter).then_some(*f)
            }
            _ => None,
        }
    };

    // 3. The expressions.
    for (i, expr) in a.exprs.iter().enumerate() {
        let id = ExprId(i as u32);
        match &expr.kind {
            // `simpleIdentifier` (the property name too).
            ExprKind::Identifier(_) | ExprKind::Property { .. } if !written_ones.contains(&id) => {
                let Some(r) = body.get_resolved(id) else { continue };
                if matches!(r, Resolved::Constructor(_)) {
                    // O tear-off `C.the_name`/`C.new`: o `ConstructorName`.
                    if let Some(tgt) = resolved_target(r) {
                        report(expr.span, tgt);
                    }
                    continue;
                }
                if let Some(tgt) = resolved_target(r) {
                    report(node_name(id), tgt);
                }
            }
            ExprKind::Assign { target, op, .. } => {
                // `readElement`/`writeElement` at the name, and the operator at the node.
                // A plain `=` has no `readElement`: only the compound ones read.
                if let Some(r) = body.get_resolved(*target) {
                    if let Some(tgt) = resolved_target(r) {
                        report(node_name(*target), tgt);
                    }
                    if *op != ast::AssignOp::Assign
                        && let Some(tgt) = read(r)
                    {
                        report(node_name(*target), tgt);
                    }
                }
                if let Some(tgt) = operator(id) {
                    report(expr.span, tgt);
                }
            }
            ExprKind::Unary { op, operand } => {
                if matches!(op, UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec)
                    && let Some(r) = body.get_resolved(*operand)
                {
                    // On the whole operand.
                    let sp = a.expr(*operand).span;
                    if let Some(tgt) = resolved_target(r) {
                        report(sp, tgt);
                    }
                    if let Some(tgt) = read(r) {
                        report(sp, tgt);
                    }
                }
                if !matches!(op, UnaryOp::NullAssert)
                    && let Some(tgt) = operator(id)
                {
                    report(expr.span, tgt);
                }
            }
            ExprKind::Binary { .. } | ExprKind::Index { .. } => {
                if let Some(tgt) = operator(id) {
                    report(expr.span, tgt);
                }
            }
            ExprKind::InstanceCreation { ty, constructor, arguments, .. } => {
                if let Some(Resolved::Constructor(f)) = body.get_resolved(id) {
                    // The `ConstructorName`: from the type to the constructor name.
                    let end = constructor.map_or(a.ty(*ty).span.end, |n| n.span.end);
                    if let Some(tgt) = function_target(*f) {
                        report(Span { start: a.ty(*ty).span.start, end: end }, tgt);
                    }
                    args_list(*f, arguments, expr.span, &mut report);
                }
            }
            ExprKind::Call { target, arguments } => {
                if let Some(Resolved::Constructor(f)) = body.get_resolved(id) {
                    // Creation without `new`: the `ConstructorName` is the target; its type
                    // is the identifier of the class (or of the typedef).
                    if let Some(tgt) = function_target(*f) {
                        report(a.expr(*target).span, tgt);
                    }
                    let mut x = *target;
                    loop {
                        match &a.expr(x).kind {
                            ExprKind::TypeArguments { target: t, .. } => x = *t,
                            ExprKind::Property { target: t, .. } if !matches!(body.get_resolved(x), Some(Resolved::Element(_))) => x = *t,
                            _ => break,
                        }
                    }
                    let typ = match body.get_resolved(x) {
                        Some(Resolved::Element(e @ (Element::Class(_) | Element::Typedef(_)))) => Some(*e),
                        _ => program.function(*f).class.map(Element::Class),
                    };
                    if let Some(e) = typ
                        && let Some(tgt) = element_target(e)
                    {
                        report(node_name(x), tgt);
                    }
                    args_list(*f, arguments, expr.span, &mut report);
                    continue;
                }
                if crate::typed_lints4::is_method_invocation(program, a, body, *target) {
                    if let Some(f) = invoked(*target) {
                        args_list(f, arguments, expr.span, &mut report);
                    }
                    continue;
                }
                // `functionExpressionInvocation`: the `call` of a class object called as a
                // function.
                if let Some(t) = body.get_type(*target)
                    && let Type::Interface { class, nullable: false, .. } = table.get(t)
                    && let Some(call) = interner.lookup("call")
                    && let Some(f) = class_member(program, *class, call)
                    && program.function(f).kind == FunctionKind::Function
                    && let Some(tgt) = function_target(f)
                {
                    report(expr.span, tgt);
                }
            }
            _ => {}
        }
    }

    // 4. `super(…)` and `this(…)`: the whole node, the arguments, and the name
    // of `this.the_name(…)` (its identifier has the constructor).
    for m in a.members.iter() {
        let MemberKind::Constructor(k) = &m.kind else { continue };
        let cls = (0..program.functions.len())
            .map(|i| FunctionElementId(i as u32))
            .find(|f| matches!(program.function(*f).node, FunctionRef::Constructor { unit, member } if unit == u && std::ptr::eq(a.member(member), m)))
            .and_then(|f| program.function(f).class);
        let Some(cls) = cls else { continue };
        for i in k.initializers.iter() {
            let (span, owner, the_name, args, redirection) = match i {
                Initializer::Super { span, constructor, arguments } => (*span, program.class(cls).supertype_class, *constructor, arguments, false),
                Initializer::Redirect { span, constructor, arguments } => (*span, Some(cls), *constructor, arguments, true),
                _ => continue,
            };
            let Some(owner) = owner else { continue };
            let key = match the_name {
                Some(n) => Some(n.sym),
                None => interner.lookup(""),
            };
            let Some(f) = key.and_then(|c| program.class(owner).constructors.get(&c).copied()) else { continue };
            if let Some(tgt) = function_target(f) {
                report(span, tgt);
            }
            if redirection
                && let Some(n) = the_name
                && let Some(tgt) = function_target(f)
            {
                report(n.span, tgt);
            }
            args_list(f, args, span, &mut report);
        }
    }

    // 5. Object pattern fields (only in the warning): at the field name, or at
    // that of the shorthand's variable.
    if mode == Mode::Warning {
        for p in a.patterns.iter() {
            let PatternKind::Object { ty, fields } = &p.kind else { continue };
            let cls = match body.annotation_types.get(ty).map(|x| table.get(*x)) {
                Some(Type::Interface { class, .. }) => Some(*class),
                Some(Type::ExtensionType { decl, .. }) => Some(*decl),
                _ => None,
            };
            let Some(cls) = cls else { continue };
            for f in fields.iter() {
                let the_name = match f.name {
                    Some(n) => Some((n.sym, n.span)),
                    None => {
                        let mut q = f.pattern;
                        loop {
                            match &a.pattern(q).kind {
                                PatternKind::Variable { name, .. } => break Some((name.sym, name.span)),
                                PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Cast { pattern: x, .. } => q = *x,
                                _ => break None,
                            }
                        }
                    }
                };
                let Some((n, sp)) = the_name else { continue };
                if let Some(getter) = class_member(program, cls, n)
                    && let Some(tgt) = function_target(getter)
                {
                    report(sp, tgt);
                }
            }
        }
    }
    out
}

/// The instance member `the_name` of the class or of one of its supertypes.
fn class_member(program: &Program, cls: dartforge_elements::model::ClassId, the_name: dartforge_intern::SymbolId) -> Option<FunctionElementId> {
    let mut seen: Vec<dartforge_elements::model::ClassId> = Vec::new();
    let mut stack = vec![cls];
    while let Some(c) = stack.pop() {
        if seen.contains(&c) {
            continue;
        }
        seen.push(c);
        let x = program.class(c);
        if let Some(f) = x.instance_members.get(&the_name) {
            return Some(*f);
        }
        stack.extend(x.interface_classes.iter().copied());
        stack.extend(x.on_classes.iter().copied());
        stack.extend(x.mixin_classes.iter().copied());
        stack.extend(x.supertype_class);
    }
    None
}

/// The uses of deprecated elements in unit `u` (the warning). `same_package`
/// says whether a library is in the package of the analyzed library
/// (`_isLibraryInWorkspacePackage`).
pub fn deprecated_uses(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    body: Option<&UnitBodyTypes>,
    u: UnitId,
    same_package: &dyn Fn(LibraryId) -> bool,
) -> Vec<Diagnostic> {
    let mut out: Vec<Diagnostic> = Vec::new();
    for x in verify(program, interner, table, body, u, Mode::Warning) {
        let the_same = same_package(x.lib);
        let d = match x.message.as_deref().map(str::trim).filter(|m| !m.is_empty() && *m != ".") {
            None => {
                let code = if the_same { h::DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE } else { h::DEPRECATED_MEMBER_USE };
                Diagnostic::with_code(code, x.span, [x.name.as_str()])
            }
            Some(m) => {
                let dotted = if m.ends_with(['.', '?', '!']) { m.to_string() } else { format!("{m}.") };
                let code =
                    if the_same { h::DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE_WITH_MESSAGE } else { h::DEPRECATED_MEMBER_USE_WITH_MESSAGE };
                Diagnostic::with_code(code, x.span, [x.name.as_str(), dotted.as_str()])
            }
        };
        if !out.iter().any(|y| y.code == d.code && y.span == d.span) {
            out.push(d);
        }
    }
    out
}

/// The lint `deprecated_member_use_from_same_package`: the uses of the
/// package's elements (the analyzer already reports outside ones as a
/// warning).
pub fn same_package_lint(
    program: &Program,
    interner: &Interner,
    table: &TypeTable,
    body: Option<&UnitBodyTypes>,
    u: UnitId,
    same_package: &dyn Fn(LibraryId) -> bool,
) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    for x in verify(program, interner, table, body, u, Mode::Lint) {
        if !same_package(x.lib) {
            continue;
        }
        let normalized = x.message.as_deref().map(str::trim);
        match normalized.filter(|m| !m.is_empty() && *m != ".") {
            None => out.push((x.span, "deprecated_member_use_from_same_package_without_message", vec![x.name])),
            Some(_) => {
                // The lint emitter appends the period to the original message (without the
                // `trim`).
                let original = x.message.clone().unwrap_or_default();
                let m = normalized.unwrap_or_default();
                let dotted = if m.ends_with(['.', '?', '!']) { m.to_string() } else { format!("{original}.") };
                out.push((x.span, "deprecated_member_use_from_same_package_with_message", vec![x.name, dotted]));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// `DEPRECATED_OPTIONAL` (the 3.13.4 `DeprecatedFunctionalityVerifier`,
// `analyzer/lib/src/error/deprecated_functionality_verifier.dart`, read in
// full at the tag; runs in the `BestPracticesVerifier`).

/// The `deprecationKind` of an annotation (3.13.4 `ElementAnnotationImpl`):
/// `use` for the `deprecated` getter and `Deprecated(…)`/`Deprecated.new(…)`;
/// the constructor name for `Deprecated.extend(…)`, `.implement(…)`,
/// `.subclass(…)`, `.instantiate(…)`, `.mixin(…)` and `.optional(…)` (the
/// `_DeprecationKind` of the 3.13.4 `dart:core`, `annotations.dart:222-230`).
///
/// Our `dart:core` is 3.6.2, which does not have these constructors: the
/// rule only applies to the library judged by 3.13.4
/// ([`Program::library_reference`]), in which the oracle resolves the
/// annotation against its own SDK.
fn deprecation_type(program: &Program, interner: &Interner, u: UnitId, m: &ast::Annotation) -> Option<String> {
    let from_core = |b: Option<dartforge_elements::model::Binding>| -> Option<Element> {
        let e = b?.getter?;
        let lib = match e {
            Element::Variable(v) => program.variable(v).library,
            Element::Class(c) => program.class(c).library,
            _ => return None,
        };
        (program.library(lib).uri == "dart:core").then_some(e)
    };
    let (element, the_name, the_constructor) = match &m.name[..] {
        [n] => (from_core(program.lookup_in_unit(u, n.sym)), n.sym, None),
        [p, n] => match from_core(program.lookup_prefixed_in_unit(u, p.sym, n.sym)) {
            Some(e) => (Some(e), n.sym, None),
            None => (from_core(program.lookup_in_unit(u, p.sym)), p.sym, Some(n.sym)),
        },
        [p, c, k] => (from_core(program.lookup_prefixed_in_unit(u, p.sym, c.sym)), c.sym, Some(k.sym)),
        _ => return None,
    };
    let text = interner.resolve(the_name);
    match element? {
        Element::Variable(_) if text == "deprecated" && m.arguments.is_none() && the_constructor.is_none() => Some("use".to_string()),
        Element::Class(_) if text == "Deprecated" => {
            m.arguments.as_ref()?;
            match the_constructor.map(|k| interner.resolve(k)) {
                None | Some("new") => Some("use".to_string()),
                Some(k @ ("extend" | "implement" | "subclass" | "instantiate" | "mixin" | "optional")) => Some(k.to_string()),
                Some(_) => None,
            }
        }
        _ => None,
    }
}

/// `Element.isDeprecatedWithKind(kind)`: some deprecation annotation in the
/// list has the kind `kind`.
fn deprecated_with_type(program: &Program, interner: &Interner, u: UnitId, metadata: &[ast::Annotation], kind: &str) -> bool {
    metadata.iter().any(|m| deprecation_type(program, interner, u, m).as_deref() == Some(kind))
}

/// The formal parameters of a function or constructor, with their unit.
fn formal_parameters(program: &Program, f: FunctionElementId) -> Option<(&[ast::Parameter], UnitId)> {
    match program.function(f).node {
        FunctionRef::Function { unit, function } => Some((program.unit(unit).ast.function(function).parameters.as_deref().unwrap_or(&[]), unit)),
        FunctionRef::Constructor { unit, member } => match &program.unit(unit).ast.member(member).kind {
            MemberKind::Constructor(k) => Some((&k.parameters[..], unit)),
            _ => None,
        },
        FunctionRef::None => None,
    }
}

/// `ParameterElement.isOptional`: optional positional, or named non-`required`.
fn the_optional(p: &ast::Parameter) -> bool {
    match p.kind {
        ParameterKind::Required => false,
        ParameterKind::Named => !p.required,
        _ => true,
    }
}

/// The `DEPRECATED_OPTIONAL` warnings of unit `u`: the omitted argument for
/// a parameter annotated with `@Deprecated.optional()`, in invocations (of a
/// method, of a constructor, of a dot shorthand and of `this(…)`), in the
/// constructors that go through the superclass constructor without it (the
/// `super.` parameters and the `super(…)`) and in redirected constructors.
pub fn deprecated_optionals(program: &Program, interner: &Interner, body: Option<&UnitBodyTypes>, u: UnitId) -> Vec<Diagnostic> {
    use dartforge_diagnostics::codes::warning::DEPRECATED_OPTIONAL;
    let mut out: Vec<Diagnostic> = Vec::new();
    let unit_id = program.unit(u);
    if program.library_reference(unit_id.library) != dartforge_diagnostics::AnalyzerReference::V3_13 {
        return out;
    }
    if interner.lookup("Deprecated").is_none() {
        return out;
    }
    let a = &unit_id.ast;
    let text = |s: dartforge_intern::SymbolId| interner.resolve(s).to_string();
    let report = |out: &mut Vec<Diagnostic>, span: Span, the_name: String| {
        let d = Diagnostic::with_code(DEPRECATED_OPTIONAL, span, [the_name.as_str()]);
        if !out.iter().any(|y| y.code == d.code && y.span == d.span && y.message == d.message) {
            out.push(d);
        }
    };
    // `_checkForDeprecatedOptional`: the parameters that no argument fills
    // (the `correspondingParameter`), with the kind `optional`.
    let omitted = |out: &mut Vec<Diagnostic>, f: FunctionElementId, args: &ast::Arguments, entity: Span| {
        let Some((ps, pu)) = formal_parameters(program, f) else { return };
        let mut filled = vec![false; ps.len()];
        let positional: Vec<usize> = (0..ps.len()).filter(|&i| ps[i].kind != ParameterKind::Named).collect();
        let mut k = 0;
        for x in args.args.iter() {
            match x.name {
                Some(n) => {
                    if let Some(i) = (0..ps.len()).find(|&i| ps[i].kind == ParameterKind::Named && ps[i].external_name().is_some_and(|m| m.sym == n.sym)) {
                        filled[i] = true;
                    }
                }
                None => {
                    if let Some(&i) = positional.get(k) {
                        filled[i] = true;
                    }
                    k += 1;
                }
            }
        }
        for (i, p) in ps.iter().enumerate() {
            if !filled[i] && deprecated_with_type(program, interner, pu, &p.metadata, "optional") {
                let the_name = p.name.map_or_else(|| "<unknown>".to_string(), |n| text(n.sym));
                report(out, entity, the_name);
            }
        }
    };
    if let Some(body) = body {
        for (i, expr) in a.exprs.iter().enumerate() {
            let id = ExprId(i as u32);
            match &expr.kind {
                // `instanceCreationExpression`: at the `ConstructorName`.
                ExprKind::InstanceCreation { ty, constructor, arguments, .. } => {
                    if let Some(Resolved::Constructor(f)) = body.get_resolved(id) {
                        let end = constructor.map_or(a.ty(*ty).span.end, |n| n.span.end);
                        omitted(&mut out, *f, arguments, Span { start: a.ty(*ty).span.start, end: end });
                    }
                }
                ExprKind::Call { target, arguments } => {
                    // The dot shorthand: `.id(…)`/`.new(…)` of a constructor
                    // (`dotShorthandConstructorInvocation`) and `.m(…)` of a static method
                    // (`dotShorthandInvocation`), at the name.
                    if let ExprKind::DotShorthand { name, .. } = &a.expr(*target).kind {
                        let f = match body.get_resolved(id) {
                            Some(Resolved::Constructor(f)) => Some(*f),
                            _ => match body.get_resolved(*target) {
                                Some(Resolved::Element(Element::Class(d))) => program
                                    .class(*d)
                                    .static_members
                                    .get(&name.sym)
                                    .copied()
                                    .filter(|f| program.function(*f).kind == FunctionKind::Function),
                                _ => None,
                            },
                        };
                        if let Some(f) = f {
                            omitted(&mut out, f, arguments, name.span);
                        }
                        continue;
                    }
                    // Creation without `new`: at the `ConstructorName` (the target).
                    if let Some(Resolved::Constructor(f)) = body.get_resolved(id) {
                        omitted(&mut out, *f, arguments, a.expr(*target).span);
                        continue;
                    }
                    // `methodInvocation`: at the method name; a local function
                    // (`LocalFunctionElement`) does not count.
                    if crate::typed_lints4::is_method_invocation(program, a, body, *target) {
                        let f = match body.get_resolved(*target) {
                            Some(Resolved::Member { member: MemberRef::Function(f), .. })
                            | Some(Resolved::ExtensionMember { member: f, .. })
                            | Some(Resolved::Element(Element::Function(f))) => Some(*f),
                            _ => None,
                        };
                        if let Some(f) = f {
                            let the_name = match &a.expr(*target).kind {
                                ExprKind::Property { name, .. } => name.span,
                                _ => a.expr(*target).span,
                            };
                            omitted(&mut out, f, arguments, the_name);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    // The declared constructors (and the elaborated primary ones).
    for (mi, m) in a.members.iter().enumerate() {
        let MemberKind::Constructor(k) = &m.kind else { continue };
        let Some(f) = (0..program.functions.len())
            .map(|i| FunctionElementId(i as u32))
            .find(|f| matches!(program.function(*f).node, FunctionRef::Constructor { unit, member } if unit == u && member.0 as usize == mi))
        else {
            continue;
        };
        let Some(cls) = program.function(f).class else { continue };
        // `ConstructorDeclaration.errorRange`: from the class name to the
        // constructor name; in a primary one with `const`, from the `const`
        // (`PrimaryConstructorDeclaration.errorRange`, the `beginToken`).
        let primary = a.decls.iter().any(|d| match &d.kind {
            DeclKind::Class(c) => c.primary_constructor.is_some_and(|p| p.0 as usize == mi),
            DeclKind::Enum(e) => e.primary_constructor.is_some_and(|p| p.0 as usize == mi),
            _ => false,
        });
        let start = if primary && k.const_ { m.span.start.min(k.class_name.span.start) } else { k.class_name.span.start };
        let error_range = Span { start: start, end: k.name.map_or(k.class_name.span.end, |n| n.span.end) };

        // `this(…)`/`this.the_name(…)` in the initializer list: at the constructor
        // name, or at the `this`.
        for i in k.initializers.iter() {
            if let Initializer::Redirect { span, constructor, arguments } = i {
                let key = match constructor {
                    Some(n) => Some(n.sym),
                    None => interner.lookup(""),
                };
                if let Some(g) = key.and_then(|c| program.class(cls).constructors.get(&c).copied()) {
                    let entity = constructor.map_or(Span { start: span.start, end: span.start + 4 }, |n| n.span);
                    omitted(&mut out, g, arguments, entity);
                }
            }
        }

        // `_checkForDeprecatedOptionalRedirectedParameters`: the redirecting
        // factory, against the parameters of the target constructor.
        if let Some(r) = &k.redirect
            && let Some(g) = redirect_target(program, interner, u, r)
            && let Some((ps, pu)) = formal_parameters(program, g)
        {
            let positional = k.parameters.iter().filter(|p| p.kind != ParameterKind::Named).count();
            let named: Vec<dartforge_intern::SymbolId> =
                k.parameters.iter().filter(|p| p.kind == ParameterKind::Named).filter_map(|p| p.name.map(|n| n.sym)).collect();
            let mut count = 0;
            for p in ps.iter() {
                if p.kind != ParameterKind::Named {
                    count += 1;
                }
                if !the_optional(p) || !deprecated_with_type(program, interner, pu, &p.metadata, "optional") {
                    continue;
                }
                if p.kind != ParameterKind::Named {
                    if count <= positional {
                        continue;
                    }
                } else if p.name.is_some_and(|n| named.contains(&n.sym)) {
                    continue;
                }
                let the_name = p.name.map_or_else(|| "<unknown>".to_string(), |n| text(n.sym));
                report(&mut out, error_range, the_name);
            }
        }

        // `_checkForDeprecatedOptionalSuperParameters`: the superclass constructor
        // that this one invokes (the `super(…)`, or the implicit unnamed one of a
        // non-redirecting generative constructor).
        if k.factory || k.redirect.is_some() || k.initializers.iter().any(|i| matches!(i, Initializer::Redirect { .. })) {
            continue;
        }
        let supers: Vec<&Initializer> = k.initializers.iter().filter(|i| matches!(i, Initializer::Super { .. })).collect();
        if supers.len() > 1 {
            continue;
        }
        let Some(superclass) = program.class(cls).supertype_class else { continue };
        let (super_name, super_arguments, range) = match supers.first() {
            Some(Initializer::Super { span, constructor, arguments }) => {
                let entity = constructor.map_or(Span { start: span.start, end: span.start + 5 }, |n| n.span);
                (constructor.map(|n| n.sym), Some(arguments), entity)
            }
            _ => (None, None, error_range),
        };
        let key = match super_name {
            Some(n) => Some(n),
            None => interner.lookup(""),
        };
        let Some(g) = key.and_then(|c| program.class(superclass).constructors.get(&c).copied()) else { continue };
        let Some((ps, pu)) = formal_parameters(program, g) else { continue };
        // `verifySuperFormalParameters`: the positional `super.` ones and the
        // names of the named `super.` ones.
        let super_positional = k.parameters.iter().filter(|p| p.super_ && p.kind != ParameterKind::Named).count();
        let super_named: Vec<dartforge_intern::SymbolId> =
            k.parameters.iter().filter(|p| p.super_ && p.kind == ParameterKind::Named).filter_map(|p| p.name.map(|n| n.sym)).collect();
        let (the_super_named, the_super_positional) = match super_arguments {
            Some(args) => (
                args.args.iter().filter_map(|x| x.name.map(|n| n.sym)).collect::<Vec<_>>(),
                args.args.iter().filter(|x| x.name.is_none()).count(),
            ),
            None => (Vec::new(), 0),
        };
        let mut count = 0;
        for p in ps.iter() {
            if p.kind != ParameterKind::Named {
                count += 1;
            }
            if !the_optional(p) || !deprecated_with_type(program, interner, pu, &p.metadata, "optional") {
                continue;
            }
            if p.kind != ParameterKind::Named {
                if count <= super_positional + the_super_positional {
                    continue;
                }
            } else {
                let n = p.name.map(|n| n.sym);
                if n.is_some_and(|n| super_named.contains(&n) || the_super_named.contains(&n)) {
                    continue;
                }
            }
            let the_name = p.name.map_or_else(|| "<unknown>".to_string(), |n| text(n.sym));
            report(&mut out, range, the_name);
        }
    }
    out
}

/// The target constructor of `factory C(…) = D.the_name;` (the `redirectedConstructor`).
fn redirect_target(program: &Program, interner: &Interner, u: UnitId, r: &ast::RedirectTarget) -> Option<FunctionElementId> {
    let a = &program.unit(u).ast;
    let TypeKind::Named { name, .. } = &a.ty(r.ty).kind else { return None };
    let binding = match &name[..] {
        [n] => program.lookup_in_unit(u, n.sym),
        [p, n] => program.lookup_prefixed_in_unit(u, p.sym, n.sym),
        _ => None,
    }?;
    let Element::Class(c) = binding.getter? else { return None };
    let key = match r.constructor {
        Some(n) => Some(n.sym),
        None => interner.lookup(""),
    }?;
    program.class(c).constructors.get(&key).copied()
}
