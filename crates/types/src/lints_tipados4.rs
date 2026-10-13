//! More typed lint rules from the `core` set
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), written directly from the
//! 3.6.2 emitters (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`).
//! As in [`crate::lints_tipados`], they return neutral findings.
//!
//! * `collection_methods_unrelated_type`: the definitions in the emitter's
//!   order (`Iterable`'s `contains`, `remove` of `List`, `Map`,
//!   `dart.collection`'s `Queue` and `Set`, `containsKey`/`containsValue` of
//!   `Map`, `Set`'s `lookup`, and `Map`'s `[]`), `asInstanceOf` through the
//!   instantiated hierarchy and the `typesAreUnrelated` of
//!   [`crate::lints_tipados3`]. Without a target, the type is the
//!   `thisType` of the class, mixin or enum enclosing the call, or the
//!   extension's extended type. Only `MethodInvocation`: a name resolved to
//!   a variable or getter becomes `FunctionExpressionInvocation` in the
//!   analyzer.
//! * `void_checks`: the assignment (through `writeType`), the `return`
//!   (through the `FunctionExpression` or the nearest method; a constructor
//!   does not count), the arguments of instance creations and of
//!   `MethodInvocation` (through `staticParameterElement`,
//!   `UnitBodyTypes::tipos_de_parametros`) and the assigned variable
//!   pattern (through the matched type).
//! * `unnecessary_overrides`: the inherited concrete member
//!   (`lookUp…(concrete: true, inherited: true)`: mixins from last to first,
//!   then the superclass, with access to private names only in the same
//!   library), the signature instantiated in the supertype, the metadata
//!   (`@override`, `@protected`), the documentation comment and a body with
//!   a single statement that forwards to `super`. `isCovariant` compares the
//!   written `covariant` (covariance inherited from further up applies to
//!   both sides).
//! * `no_duplicate_case_values`: only the old `SwitchCase` (library before
//!   3.0), with the constants engine and `DartObjectImpl` equality.
//!
//! Written without compiling or running (2026-10-05).

use crate::constantes::avaliador::{Constante, Ctx, Motor};
use crate::lints_tipados::Finding;
use crate::resolve::OutlineTypes;
use crate::resolved::{BodyTypes, MemberRef, Resolved, UnitBodyTypes};
use crate::table::{CoreTypes, Type, TypeId, TypeTable};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, FunctionElementId, FunctionKind, FunctionRef, LibraryId, Program, UnitId};
use dartforge_frontend::ast::{self, BinaryOp, DeclKind, ExprId, ExprKind, FunctionBody, MemberKind, ParameterKind, PatternKind, StmtKind, UnaryOp};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};

fn without_parentheses(a: &ast::Ast, mut e: ExprId) -> ExprId {
    while let ExprKind::Parenthesized(x) = &a.expr(e).kind {
        e = *x;
    }
    e
}

fn inside(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

/// The `FunctionElementId` of each written function of the unit.
fn function_elements(program: &Program, u: UnitId) -> HashMap<ast::FunctionId, FunctionElementId> {
    let mut m = HashMap::new();
    for (k, f) in program.functions.iter().enumerate() {
        if let FunctionRef::Function { unit, function } = f.node
            && unit == u
        {
            m.insert(function, FunctionElementId(k as u32));
        }
    }
    m
}

/// The `realTarget` of each cascade section.
fn cascade_targets(a: &ast::Ast) -> HashMap<ExprId, ExprId> {
    let mut m = HashMap::new();
    for e in a.exprs.iter() {
        let ExprKind::Cascade { target, sections, .. } = &e.kind else { continue };
        for &s in sections.iter() {
            let mut x = s;
            loop {
                let next_ = match &a.expr(x).kind {
                    ExprKind::Property { target, .. }
                    | ExprKind::Index { target, .. }
                    | ExprKind::Call { target, .. }
                    | ExprKind::TypeArguments { target, .. }
                    | ExprKind::Assign { target, .. } => *target,
                    ExprKind::Unary { operand, .. } => *operand,
                    ExprKind::CascadeTarget => {
                        m.insert(x, *target);
                        break;
                    }
                    _ => break,
                };
                x = next_;
            }
        }
    }
    m
}

/// The call is an analyzer `MethodInvocation` (the name does not resolve to
/// a variable or a getter, which become `FunctionExpressionInvocation`).
pub(crate) fn is_method_invocation(program: &Program, a: &ast::Ast, body: &UnitBodyTypes, tgt: ExprId) -> bool {
    if !matches!(a.expr(tgt).kind, ExprKind::Identifier(_) | ExprKind::Property { .. }) {
        return false;
    }
    let non_getter_function = |f: FunctionElementId| program.function(f).kind != FunctionKind::Getter;
    match body.get_resolved(tgt) {
        Some(Resolved::Member { member: MemberRef::Function(f), .. }) => non_getter_function(*f),
        Some(Resolved::Member { member: MemberRef::Variable(_), .. }) => false,
        Some(Resolved::Element(dartforge_elements::model::Element::Function(f))) => non_getter_function(*f),
        Some(Resolved::Element(_)) => false,
        Some(Resolved::ExtensionMember { member, .. }) => non_getter_function(*member),
        Some(Resolved::Local(_)) => {
            // A local function is a `MethodInvocation`; a local variable is not.
            body.declaracao_local(tgt).is_some_and(|d| {
                a.stmts.iter().any(|s| matches!(&s.kind, StmtKind::Function(g) if a.function(*g).name.is_some_and(|n| n.span.start == d)))
            })
        }
        Some(Resolved::Parameter { .. }) => false,
        _ => true,
    }
}

/// The findings of this module's typed rules in unit `u`.
#[allow(clippy::too_many_arguments)]
pub fn achados(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
    the_inferred: &HashSet<LibraryId>,
    u: UnitId,
) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    let Some(body) = bodies.units.get(u.0 as usize) else { return out };
    the_collections(program, interner, table, core, outline, body, u, &mut out);
    void_checks(program, table, core, outline, body, u, &mut out);
    unnecessary_overrides(program, interner, table, core, outline, body, u, &mut out);
    duplicate_cases(program, interner, table, core, outline, bodies, the_inferred, u, &mut out);
    out
}

/// A `_MethodDefinition` definition.
struct Definition {
    method: &'static str,
    /// The class (`dart:core`), or the interface by name and library.
    class: Option<ClassId>,
    by_name: Option<(&'static str, &'static str)>,
    index: usize,
}

/// `collection_methods_unrelated_type`.
#[allow(clippy::too_many_arguments)]
fn the_collections(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    body: &UnitBodyTypes,
    u: UnitId,
    out: &mut Vec<Finding>,
) {
    let a = &program.unit(u).ast;
    let cascades = cascade_targets(a);
    let def = |method, cls: Option<ClassId>, idx| Definition { method, class: cls, by_name: None, index: idx };
    let methods = [
        def("contains", core.iterable_class, 0),
        def("remove", core.list_class, 0),
        def("containsKey", core.map_class, 0),
        def("containsValue", core.map_class, 1),
        def("remove", core.map_class, 0),
        Definition { method: "remove", class: None, by_name: Some(("dart.collection", "Queue")), index: 0 },
        def("lookup", core.set_class, 0),
        def("remove", core.set_class, 0),
    ];
    let indices = [def("[]", core.map_class, 0)];
    let library_name = |l: LibraryId| program.library(l).name.as_ref().map(|n| n.iter().map(|s| interner.resolve(*s)).collect::<Vec<_>>().join("."));
    // `collectionTypeFor`: o `asInstanceOf`.
    let collection = |table: &mut TypeTable, t: TypeId, d: &Definition| -> Option<TypeId> {
        if let Some(c) = d.class {
            return outline.hierarchy.supertype_of(t, c, table, core);
        }
        let (library, interface) = d.by_name?;
        let class_of = |t: TypeId| match table.get(t) {
            Type::Interface { class, .. } | Type::ExtensionType { decl: class, .. } => Some(*class),
            _ => None,
        };
        let own_ = class_of(t)?;
        let mut candidates = vec![own_];
        if let Some(d) = outline.hierarchy.get(own_) {
            let mut sup: Vec<ClassId> = d.supertypes.keys().copied().collect();
            sup.sort();
            candidates.extend(sup);
        }
        let tgt = candidates.into_iter().find(|c| {
            let k = program.class(*c);
            interner.resolve(k.name) == interface && library_name(k.library).as_deref() == Some(library)
        })?;
        outline.hierarchy.supertype_of(t, tgt, table, core)
    };
    let e_interface = |table: &TypeTable, t: TypeId| matches!(table.get(t), Type::Interface { .. } | Type::ExtensionType { .. } | Type::FutureOr { .. });
    // The type of the implicit target (`this`): the enclosing class, mixin or
    // enum, or the extension's extended type.
    let implicit_target = |table: &mut TypeTable, span: Span| -> Option<TypeId> {
        let mut best: Option<(Span, TypeId)> = None;
        for &did in &program.unit(u).unit.declarations {
            let d = a.decl(did);
            if !inside(span, d.span) {
                continue;
            }
            let t = match &d.kind {
                DeclKind::Class(_) | DeclKind::Mixin(_) | DeclKind::Enum(_) => {
                    let c = (0..program.classes.len()).map(|i| ClassId(i as u32)).find(|c| {
                        program.class(*c).decl.is_some_and(|r| r.unit == u && r.decl == did)
                    })?;
                    let params: Vec<TypeId> = outline
                        .classes
                        .get(c.0 as usize)
                        .map(|x| x.type_params.iter().map(|p| table.intern(Type::TypeParameter { param: *p, nullable: false })).collect())
                        .unwrap_or_default();
                    table.intern(Type::Interface { class: c, args: params.into(), nullable: false })
                }
                DeclKind::Extension(_) => {
                    let e = program.extensions.iter().position(|x| x.decl.unit == u && x.decl.decl == did)?;
                    outline.extensions.get(e)?.on
                }
                _ => return None,
            };
            best = Some((d.span, t));
        }
        best.map(|(_, t)| t)
    };
    let mut the_cases: Vec<(Span, ExprId, TypeId, &Definition)> = Vec::new();
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::Call { target, arguments } => {
                if arguments.args.len() != 1 || !is_method_invocation(program, a, body, *target) {
                    continue;
                }
                let (tgt, the_name) = match &a.expr(*target).kind {
                    ExprKind::Property { target: tgt, name, .. } => (Some(*tgt), *name),
                    ExprKind::Identifier(n) => (None, *n),
                    _ => continue,
                };
                let the_name = interner.resolve(the_name.sym);
                let candidates: Vec<&Definition> = methods.iter().filter(|d| d.method == the_name).collect();
                if candidates.is_empty() {
                    continue;
                }
                let target_type = match tgt {
                    Some(x) => {
                        let x = if matches!(a.expr(x).kind, ExprKind::CascadeTarget) { cascades.get(&x).copied() } else { Some(x) };
                        x.and_then(|x| body.get_type(x))
                    }
                    None => implicit_target(table, e.span),
                };
                let Some(t) = target_type.filter(|t| e_interface(table, *t)) else { continue };
                let arg = &arguments.args[0];
                let span = match arg.name {
                    Some(n) => Span { start: n.span.start, end: a.expr(arg.value).span.end },
                    None => a.expr(arg.value).span,
                };
                for d in candidates {
                    if let Some(col) = collection(table, t, d) {
                        the_cases.push((span, arg.value, col, d));
                        break;
                    }
                }
            }
            ExprKind::Index { target, index, .. } => {
                let x = if matches!(a.expr(*target).kind, ExprKind::CascadeTarget) { cascades.get(target).copied() } else { Some(*target) };
                let Some(t) = x.and_then(|x| body.get_type(x)).filter(|t| e_interface(table, *t)) else { continue };
                for d in &indices {
                    if let Some(col) = collection(table, t, d) {
                        the_cases.push((a.expr(*index).span, *index, col, d));
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    let future_or = interner
        .lookup("FutureOr")
        .and_then(|s| program.library(core.async_library.unwrap_or(LibraryId(0))).declared.get(&s))
        .and_then(|b| b.getter)
        .and_then(|e| match e {
            dartforge_elements::model::Element::Class(c) => Some(c),
            _ => None,
        });
    for (span, arg, col, d) in the_cases {
        let Some(ta) = body.get_type(arg).filter(|t| !core.is_unknown(table, *t)) else { continue };
        let Type::Interface { args, .. } = table.get(col).clone() else { continue };
        let Some(&arg_type) = args.get(d.index) else { continue };
        let mut rel = crate::lints_tipados3::Relation { program, table: &mut *table, core, outline, future_or, call: interner.lookup("call") };
        if rel.unrelated(ta, arg_type) {
            let args_list = vec![crate::despejo::formatar(table, ta, interner, program), crate::despejo::formatar(table, arg_type, interner, program)];
            out.push((span, "collection_methods_unrelated_type", args_list));
        }
    }
}

/// `isTypeAcceptableWhenExpectingVoid`.
fn accepted_as_void(program: &Program, table: &TypeTable, core: &CoreTypes, t: TypeId) -> bool {
    match table.get(t) {
        Type::Void | Type::Null | Type::Never => true,
        Type::Interface { class, args, .. } if Some(*class) == core.future_class => {
            args.first().is_some_and(|x| accepted_as_void(program, table, core, *x))
        }
        _ => false,
    }
}

/// `isTypeAcceptableWhenExpectingFutureOrVoid`.
fn accepted_as_future_or_void(program: &Program, table: &TypeTable, core: &CoreTypes, t: TypeId) -> bool {
    match table.get(t) {
        Type::Dynamic | Type::FutureOr { .. } => true,
        _ if accepted_as_void(program, table, core, t) => true,
        Type::Interface { class, args, .. } if Some(*class) == core.future_class => {
            args.first().is_some_and(|x| accepted_as_future_or_void(program, table, core, *x))
        }
        _ => false,
    }
}

/// `void_checks`'s `_check`: `true` reports. `block_closure`: the checked
/// node is a `FunctionExpression` whose body is not an expression body.
fn check_void(program: &Program, table: &TypeTable, core: &CoreTypes, expected: TypeId, t: TypeId, is_return: bool, block_closure: bool) -> bool {
    if core.is_unknown(table, expected) || core.is_unknown(table, t) {
        return false;
    }
    let expected_void = matches!(table.get(expected), Type::Void);
    if expected_void && !matches!(table.get(t), Type::Dynamic) && is_return {
        return false;
    }
    if expected_void && !accepted_as_void(program, table, core, t) {
        return true;
    }
    if let Type::FutureOr { arg, .. } = table.get(expected) {
        if matches!(table.get(*arg), Type::Void) && !accepted_as_future_or_void(program, table, core, t) {
            return true;
        }
        return false;
    }
    if expected_void {
        return false;
    }
    if block_closure
        && let (Type::Function { ret: r1, .. }, Type::Function { ret: r2, .. }) = (table.get(expected), table.get(t))
    {
        return check_void(program, table, core, *r1, *r2, is_return, block_closure);
    }
    false
}

/// `void_checks`.
fn void_checks(program: &Program, table: &mut TypeTable, core: &CoreTypes, outline: &OutlineTypes, body: &UnitBodyTypes, u: UnitId, out: &mut Vec<Finding>) {
    let table: &TypeTable = table;
    let a = &program.unit(u).ast;
    let ty = |e: ExprId| body.get_type(e).filter(|t| !core.is_unknown(table, *t));
    let block_closure = |e: ExprId| match &a.expr(e).kind {
        ExprKind::FunctionExpression(f) => !matches!(a.function(*f).body, FunctionBody::Expression(_)),
        _ => false,
    };
    let report = |span: Span, out: &mut Vec<Finding>| out.push((span, "void_checks", Vec::new()));
    // Assignments.
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        if let ExprKind::Assign { value, .. } = &e.kind
            && let (Some(&expected), Some(t)) = (body.tipos_de_escrita.get(&id), ty(*value))
            && check_void(program, table, core, expected, t, false, block_closure(*value))
        {
            report(e.span, out);
        }
    }
    // `return`: the nearest function (literal, local, top-level or method).
    let elems = function_elements(program, u);
    let literals: HashMap<ast::FunctionId, ExprId> = a
        .exprs
        .iter()
        .enumerate()
        .filter_map(|(k, e)| match &e.kind {
            ExprKind::FunctionExpression(f) => Some((*f, ExprId(k as u32))),
            _ => None,
        })
        .collect();
    let locals: HashMap<ast::FunctionId, usize> = a
        .stmts
        .iter()
        .filter_map(|s| match &s.kind {
            StmtKind::Function(f) => a.function(*f).name.map(|n| (*f, n.span.start)),
            _ => None,
        })
        .collect();
    // Constructor bodies (a `return` in them has no function).
    let constructors: Vec<Span> = a
        .members
        .iter()
        .filter_map(|m| match &m.kind {
            MemberKind::Constructor(k) => match &k.body {
                FunctionBody::Block(s) => Some(a.stmt(*s).span),
                _ => None,
            },
            _ => None,
        })
        .collect();
    let function_return = |f: ast::FunctionId| -> Option<TypeId> {
        let t = if let Some(&e) = literals.get(&f) {
            ty(e)?
        } else if let Some(&p) = locals.get(&f) {
            body.tipo_local(p)?
        } else {
            return elems.get(&f).and_then(|fe| outline.functions.get(fe.0 as usize)).map(|d| d.return_type);
        };
        match table.get(t) {
            Type::Function { ret, .. } => Some(*ret),
            _ => None,
        }
    };
    for s in a.stmts.iter() {
        let StmtKind::Return(Some(v)) = &s.kind else { continue };
        // The innermost function containing the `return`.
        let innermost = a
            .functions
            .iter()
            .enumerate()
            .filter(|(_, f)| inside(s.span, f.span))
            .min_by_key(|(_, f)| f.span.end - f.span.start)
            .map(|(k, f)| (ast::FunctionId(k as u32), f.span));
        let Some((f, fspan)) = innermost else { continue };
        // A constructor inside the function (does not happen) or the function
        // inside a constructor: the innermost one wins; a constructor more
        // inner than the function removes the check.
        if constructors.iter().any(|c| inside(s.span, *c) && inside(*c, fspan) && *c != fspan) {
            continue;
        }
        if let (Some(expected), Some(t)) = (function_return(f), ty(*v))
            && check_void(program, table, core, expected, t, true, block_closure(*v))
        {
            report(s.span, out);
        }
    }
    // Arguments of instance creations and of `MethodInvocation`.
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        let args_list = match &e.kind {
            ExprKind::InstanceCreation { arguments, .. } => arguments,
            ExprKind::Call { target, arguments } => {
                let constructor = matches!(body.get_resolved(id), Some(Resolved::Constructor(_)));
                if !constructor && !is_method_invocation(program, a, body, *target) {
                    continue;
                }
                arguments
            }
            _ => continue,
        };
        for arg in args_list.args.iter() {
            if let (Some(&p), Some(t)) = (body.tipos_de_parametros.get(&arg.value), ty(arg.value))
                && check_void(program, table, core, p, t, false, block_closure(arg.value))
            {
                report(a.expr(arg.value).span, out);
            }
        }
    }
    // The assigned variable pattern.
    let mut assigned: Vec<ast::PatternId> = Vec::new();
    fn variables(a: &ast::Ast, p: ast::PatternId, v: &mut Vec<ast::PatternId>) {
        match &a.pattern(p).kind {
            PatternKind::Variable { .. } => v.push(p),
            PatternKind::Or(x, y) | PatternKind::And(x, y) => {
                variables(a, *x, v);
                variables(a, *y, v);
            }
            PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) | PatternKind::Cast { pattern: x, .. } => variables(a, *x, v),
            PatternKind::List { elements, .. } => {
                for el in elements.iter() {
                    if let ast::ListPatternElement::Pattern(x) | ast::ListPatternElement::Rest(Some(x)) = el {
                        variables(a, *x, v);
                    }
                }
            }
            PatternKind::Map { entries, .. } => {
                for en in entries.iter() {
                    variables(a, en.value, v);
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                for f in fields.iter() {
                    variables(a, f.pattern, v);
                }
            }
            _ => {}
        }
    }
    for e in a.exprs.iter() {
        if let ExprKind::PatternAssign { pattern, .. } = &e.kind {
            variables(a, *pattern, &mut assigned);
        }
    }
    for p in assigned {
        let (Some(&the_value), Some(&decl)) = (body.tipos_casados.get(&p), body.declaracoes_de_padroes.get(&p)) else { continue };
        let Some(expected) = body.tipo_local(decl) else { continue };
        if check_void(program, table, core, expected, the_value, false, false) {
            report(a.pattern(p).span, out);
        }
    }
}

/// The member kind: method (including operators), getter or setter.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Metodo,
    Getter,
    Setter,
}

/// A parameter of a member: name, kind, written `covariant`, type and the
/// `defaultValueCode`.
struct MemberParameter {
    name: String,
    /// `_sameKind`: 0 required (positional or named `required`), 1 optional
    /// positional, 2 optional named.
    kind: u8,
    covariant: bool,
    ty: TypeId,
    pattern: Option<String>,
}

/// `unnecessary_overrides`.
#[allow(clippy::too_many_arguments)]
fn unnecessary_overrides(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    body: &UnitBodyTypes,
    u: UnitId,
    out: &mut Vec<Finding>,
) {
    let unit_id = program.unit(u);
    let a = &unit_id.ast;
    let source = unit_id.source.as_str();
    let lib = unit_id.library;
    let comments = dartforge_frontend::comentarios::Comentarios::de(source);
    let elems = function_elements(program, u);
    let protected = |u2: UnitId, m: &ast::Annotation| crate::anotacoes::e_getter_de(program, interner, u2, m, "meta", "protected");
    // The annotations of a member (the method's, or the implicit accessor's field's).
    let member_metadata = |f: FunctionElementId| -> Option<(UnitId, Vec<&ast::Annotation>)> {
        let fe = program.function(f);
        match fe.node {
            FunctionRef::Function { unit, function } => {
                let a2 = &program.unit(unit).ast;
                let m = a2.members.iter().find(|m| matches!(m.kind, MemberKind::Method(g) if g == function))?;
                Some((unit, m.metadata.iter().collect()))
            }
            _ => {
                let v = fe.variable?;
                match program.variable(v).node {
                    dartforge_elements::model::VariableRef::Field { unit, member, .. } => {
                        Some((unit, program.unit(unit).ast.member(member).metadata.iter().collect()))
                    }
                    _ => None,
                }
            }
        }
    };
    let has_protected = |f: FunctionElementId| member_metadata(f).is_some_and(|(u2, ms)| ms.iter().any(|m| protected(u2, m)));
    for d in a.decls.iter() {
        let (members, e_interface) = match &d.kind {
            DeclKind::Class(x) => (&x.members, true),
            DeclKind::Mixin(x) => (&x.members, true),
            DeclKind::Enum(x) => (&x.members, true),
            DeclKind::ExtensionType(x) => (&x.members, true),
            _ => continue,
        };
        if !e_interface {
            continue;
        }
        for &mid in members {
            let m = a.member(mid);
            let MemberKind::Method(fid) = &m.kind else { continue };
            let f = a.function(*fid);
            if f.static_ {
                continue;
            }
            let Some(the_name) = f.name else { continue };
            let text = interner.resolve(the_name.sym);
            if text == "noSuchMethod" {
                continue;
            }
            // The documentation comment.
            let after = match m.metadata.last() {
                Some(x) => dartforge_frontend::fonte::pular_brancos(source.as_bytes(), x.span.end),
                None => m.span.start,
            };
            if comments.dart_doc(source, after).is_some() || m.metadata.iter().rev().any(|x| comments.dart_doc(source, x.span.start).is_some()) {
                continue;
            }
            let Some(&own) = elems.get(fid) else { continue };
            let Some(cls) = program.function(own).class else { continue };
            let kind = match f.kind {
                ast::FunctionKind::Getter => Kind::Getter,
                ast::FunctionKind::Setter => Kind::Setter,
                _ => Kind::Metodo,
            };
            let key = match kind {
                Kind::Setter => interner.lookup(&format!("{text}_=")),
                _ => Some(the_name.sym),
            };
            let Some(key) = key else { continue };
            let Some((owner, inherited)) = inherited_concrete(program, cls, key, lib, text.starts_with('_'), kind) else { continue };
            // `_addsMetadata`.
            let inherited_protected = has_protected(inherited);
            let adds = m.metadata.iter().any(|x| {
                let e_override = crate::anotacoes::e_getter_de(program, interner, u, x, "dart.core", "override");
                let is_protected = protected(u, x);
                !(e_override || (is_protected && inherited_protected))
            });
            if adds {
                continue;
            }
            // `_haveSameDeclaration`.
            let Some(the_own) = member_parameters(program, interner, table, outline, own, None) else { continue };
            let this_type = {
                let params: Vec<TypeId> = outline
                    .classes
                    .get(cls.0 as usize)
                    .map(|x| x.type_params.iter().map(|p| table.intern(Type::TypeParameter { param: *p, nullable: false })).collect())
                    .unwrap_or_default();
                table.intern(Type::Interface { class: cls, args: params.into(), nullable: false })
            };
            let Some(instantiated) = outline.hierarchy.supertype_of(this_type, owner, table, core) else { continue };
            let Some(the_inherited) = member_parameters(program, interner, table, outline, inherited, Some((owner, instantiated))) else { continue };
            let (ret_own, ps_own) = the_own;
            let (ret_inherited, ps_inherited) = the_inherited;
            if crate::ops::without_display(ret_own, table) != crate::ops::without_display(ret_inherited, table) || ps_own.len() != ps_inherited.len() {
                continue;
            }
            let same = ps_own.iter().zip(ps_inherited.iter()).all(|(p, q)| {
                crate::ops::without_display(p.ty, table) == crate::ops::without_display(q.ty, table)
                    && p.name == q.name
                    && p.covariant == q.covariant
                    && p.kind == q.kind
                    && p.pattern == q.pattern
            });
            if !same {
                continue;
            }
            // `_makesPublicFromProtected`.
            let own_protected = m.metadata.iter().any(|x| protected(u, x));
            if !own_protected && inherited_protected {
                continue;
            }
            if passes_to_super(a, source, interner, &comments, body, f, kind, text) {
                out.push((the_name.span, "unnecessary_overrides", Vec::new()));
            }
        }
    }
}

/// `lookUp{Method,Getter,Setter}2(concrete: true, inherited: true)`: the
/// declaring class and the member.
fn inherited_concrete(program: &Program, cls: ClassId, key: SymbolId, lib: LibraryId, private: bool, kind: Kind) -> Option<(ClassId, FunctionElementId)> {
    let serve = |c: ClassId| -> Option<FunctionElementId> {
        let e = program.class(c);
        if private && e.library != lib {
            return None;
        }
        let f = *e.instance_members.get(&key)?;
        let m = program.function(f);
        let of_kind = match kind {
            Kind::Metodo => matches!(m.kind, FunctionKind::Function | FunctionKind::Operator),
            Kind::Getter => m.kind == FunctionKind::Getter,
            Kind::Setter => m.kind == FunctionKind::Setter,
        };
        (!m.abstract_ && of_kind).then_some(f)
    };
    let mut seen: HashSet<ClassId> = HashSet::new();
    let mut current = Some(cls);
    let mut first = true;
    while let Some(c) = current {
        if !seen.insert(c) {
            break;
        }
        if !first && let Some(f) = serve(c) {
            return Some((c, f));
        }
        first = false;
        let e = program.class(c);
        for &mx in e.mixin_classes.iter().rev() {
            if let Some(f) = serve(mx) {
                return Some((mx, f));
            }
        }
        current = e.supertype_class;
    }
    None
}

/// The return type and the parameters (in written order) of a member;
/// `instantiated`: the owner class and the instantiated supertype in which the
/// member is seen (the substitution of its type parameters).
fn member_parameters(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    outline: &OutlineTypes,
    f: FunctionElementId,
    instantiated: Option<(ClassId, TypeId)>,
) -> Option<(TypeId, Vec<MemberParameter>)> {
    let data = outline.functions.get(f.0 as usize)?;
    let mut sig = data.signature;
    if let Some((owner, sup)) = instantiated {
        let params = outline.classes.get(owner.0 as usize).map(|x| x.type_params.clone()).unwrap_or_default();
        if let Type::Interface { args, .. } = table.get(sup).clone() {
            let map: HashMap<crate::table::TypeParamId, TypeId> = params.iter().copied().zip(args.iter().copied()).collect();
            sig = crate::ops::substitute(sig, &map, table);
        }
    }
    let Type::Function { positional, optional, named, ret, .. } = table.get(sig).clone() else { return None };
    let fe = program.function(f);
    let mut v = Vec::new();
    match fe.node {
        FunctionRef::Function { unit, function } => {
            let a = &program.unit(unit).ast;
            let source = program.unit(unit).source.as_str();
            let ps = a.function(function).parameters.as_deref().unwrap_or(&[]);
            let (mut i_pos, mut i_opt) = (0usize, 0usize);
            for p in ps {
                let ty = match p.kind {
                    ParameterKind::Required => {
                        i_pos += 1;
                        *positional.get(i_pos - 1)?
                    }
                    ParameterKind::Named => {
                        let n = p.public_name.or(p.name)?.sym;
                        named.iter().find(|(s, _, _)| *s == n).map(|(_, t, _)| *t)?
                    }
                    _ => {
                        i_opt += 1;
                        *optional.get(i_opt - 1)?
                    }
                };
                v.push(MemberParameter {
                    name: p.name.map(|n| interner.resolve(n.sym).to_string()).unwrap_or_default(),
                    kind: match p.kind {
                        ParameterKind::Required => 0,
                        ParameterKind::Named if p.required => 0,
                        ParameterKind::Named => 2,
                        _ => 1,
                    },
                    covariant: p.covariant,
                    ty,
                    pattern: p.default_value.map(|d| dartforge_frontend::fonte::de_expr(a, source, interner, d)),
                });
            }
        }
        _ => {
            // The implicit accessor of a field: the setter has a parameter
            // `_<the_name>`.
            if fe.kind == FunctionKind::Setter {
                let v_id = fe.variable?;
                let the_name = interner.resolve(program.variable(v_id).name);
                let covariant = match program.variable(v_id).node {
                    dartforge_elements::model::VariableRef::Field { unit, member, .. } => match &program.unit(unit).ast.member(member).kind {
                        MemberKind::Field(l) => l.covariant,
                        _ => false,
                    },
                    _ => false,
                };
                v.push(MemberParameter {
                    name: format!("_{the_name}"),
                    kind: 0,
                    covariant,
                    ty: *positional.first()?,
                    pattern: None,
                });
            }
        }
    }
    Some((ret, v))
}

/// The body forwards to `super` (the emitter's visitors).
#[allow(clippy::too_many_arguments)]
fn passes_to_super(
    a: &ast::Ast,
    source: &str,
    interner: &Interner,
    comments: &dartforge_frontend::comentarios::Comentarios,
    body: &UnitBodyTypes,
    f: &ast::Function,
    kind: Kind,
    the_name: &str,
) -> bool {
    let commented = |pos: usize| !comments.antes_de(source, pos).is_empty();
    // `visitSuperExpression`.
    let super_without_comment = |e: ExprId| {
        let e = without_parentheses(a, e);
        matches!(a.expr(e).kind, ExprKind::Super) && !commented(a.expr(e).span.start)
    };
    let ps = f.parameters.as_deref().unwrap_or(&[]);
    // The method's own parameter that the expression cites (`canonicalElement`).
    let parameter = |e: ExprId| -> Option<SymbolId> {
        let e = without_parentheses(a, e);
        match (&a.expr(e).kind, body.get_resolved(e)) {
            (ExprKind::Identifier(n), Some(Resolved::Parameter { .. })) if ps.iter().any(|p| p.name.is_some_and(|q| q.sym == n.sym)) => Some(n.sym),
            _ => None,
        }
    };
    let visit_expr = |e: ExprId| -> bool {
        let e = without_parentheses(a, e);
        match kind {
            Kind::Getter => match &a.expr(e).kind {
                ExprKind::Property { target, name, .. } if interner.resolve(name.sym) == the_name => super_without_comment(*target),
                _ => false,
            },
            Kind::Setter => match &a.expr(e).kind {
                ExprKind::Assign { target, value, .. } => {
                    let [p] = ps else { return false };
                    if p.name.map(|n| n.sym) != parameter(*value) {
                        return false;
                    }
                    match &a.expr(without_parentheses(a, *target)).kind {
                        ExprKind::Property { target, name, .. } if interner.resolve(name.sym) == the_name => super_without_comment(*target),
                        _ => false,
                    }
                }
                _ => false,
            },
            Kind::Metodo if f.kind == ast::FunctionKind::Operator => match &a.expr(e).kind {
                ExprKind::Binary { op, left, right } => {
                    let [p] = ps else { return false };
                    operator_text(*op) == Some(the_name) && p.name.map(|n| n.sym) == parameter(*right) && super_without_comment(*left)
                }
                ExprKind::Unary { op, operand } => {
                    let the_same = match op {
                        UnaryOp::Neg => the_name == "unary-" || the_name == "-",
                        UnaryOp::BitNot => the_name == "~",
                        _ => false,
                    };
                    the_same && ps.is_empty() && super_without_comment(*operand)
                }
                _ => false,
            },
            Kind::Metodo => match &a.expr(e).kind {
                ExprKind::Call { target, arguments } => {
                    let ExprKind::Property { target: tgt, name, .. } = &a.expr(*target).kind else { return false };
                    if interner.resolve(name.sym) != the_name {
                        return false;
                    }
                    // `argumentsMatchParameters`.
                    let positional_args: Vec<SymbolId> = ps.iter().filter(|p| p.kind != ParameterKind::Named).filter_map(|p| p.name.map(|n| n.sym)).collect();
                    let named_args: Vec<SymbolId> = ps.iter().filter(|p| p.kind == ParameterKind::Named).filter_map(|p| p.name.map(|n| n.sym)).collect();
                    let mut args_pos: Vec<SymbolId> = Vec::new();
                    let mut the_named_args: Vec<(SymbolId, SymbolId)> = Vec::new();
                    for x in arguments.args.iter() {
                        let Some(el) = parameter(x.value) else { return false };
                        match x.name {
                            Some(n) => the_named_args.push((n.sym, el)),
                            None => args_pos.push(el),
                        }
                    }
                    if positional_args != args_pos || named_args.len() != the_named_args.len() {
                        return false;
                    }
                    let all_named = named_args.iter().all(|n| {
                        let external = ps.iter().find(|p| p.name.is_some_and(|q| q.sym == *n)).and_then(|p| p.public_name.or(p.name)).map(|q| q.sym);
                        the_named_args.iter().any(|(label, el)| Some(*label) == external && el == n)
                    });
                    all_named && super_without_comment(*tgt)
                }
                _ => false,
            },
        }
    };
    match &f.body {
        FunctionBody::Expression(e) => visit_expr(*e),
        FunctionBody::Block(b) => {
            let StmtKind::Block(the_cmds) = &a.stmt(*b).kind else { return false };
            let [s] = &the_cmds[..] else { return false };
            match &a.stmt(*s).kind {
                StmtKind::Expression(e) => visit_expr(*e),
                StmtKind::Return(v) => {
                    if commented(a.stmt(*s).span.start) {
                        return false;
                    }
                    v.is_some_and(visit_expr)
                }
                _ => false,
            }
        }
        _ => false,
    }
}

/// The binary operator's text (the name of the `operator` it calls).
fn operator_text(op: BinaryOp) -> Option<&'static str> {
    Some(match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::TruncDiv => "~/",
        BinaryOp::Rem => "%",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
        BinaryOp::UShr => ">>>",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Eq => "==",
        BinaryOp::Lt => "<",
        BinaryOp::Gt => ">",
        BinaryOp::LtEq => "<=",
        BinaryOp::GtEq => ">=",
        _ => return None,
    })
}

/// `no_duplicate_case_values`.
#[allow(clippy::too_many_arguments)]
fn duplicate_cases(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
    the_inferred: &HashSet<LibraryId>,
    u: UnitId,
    out: &mut Vec<Finding>,
) {
    let unit_id = program.unit(u);
    let version = program.library(unit_id.library).features.versao();
    if version >= dartforge_frontend::features::LanguageVersion::new(3, 0) {
        return;
    }
    let a = &unit_id.ast;
    let source = unit_id.source.as_str();
    let switches: Vec<&[ast::SwitchCase]> = a
        .stmts
        .iter()
        .filter_map(|s| match &s.kind {
            StmtKind::Switch { cases, .. } => Some(&cases[..]),
            _ => None,
        })
        .collect();
    if switches.is_empty() {
        return;
    }
    let mut engine = Motor::novo(program, interner, table, core, outline, bodies, the_inferred);
    let cx = Ctx::simples(u, unit_id.library);
    for the_cases in switches {
        let mut seen: Vec<(crate::constantes::valor::Valor, ExprId)> = Vec::new();
        for case in the_cases {
            let Some(p) = case.pattern else { continue };
            let PatternKind::Constant(e) = &a.pattern(p).kind else { continue };
            let Constante::Valor(v) = engine.avaliar(&cx, *e, true) else { continue };
            if v.estado.unknown() {
                continue;
            }
            match seen.iter().position(|(w, _)| engine.iguais(&v, w)) {
                Some(i) => {
                    let previous = seen[i].1;
                    let args = vec![dartforge_frontend::fonte::de_expr(a, source, interner, *e), dartforge_frontend::fonte::de_expr(a, source, interner, previous)];
                    out.push((a.expr(*e).span, "no_duplicate_case_values", args));
                }
                None => seen.push((v, *e)),
            }
        }
    }
}
