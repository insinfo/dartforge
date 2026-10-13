//! `UNUSED_ELEMENT` and `UNUSED_FIELD` of library declarations: the 3.6.2
//! `GatherUsedLocalElementsVisitor` and `UnusedLocalElementsVerifier`
//! (`analyzer/lib/src/error/unused_local_elements_verifier.dart`), through
//! the resolved element of each reference (the local part, local variables
//! and functions, and the parameter part live in `analise::locais` and
//! `parametros`).
//!
//! Collection (over all units of the library, without the comment
//! references):
//! * the resolved identifier (and property name): reading a top-level
//!   variable through the synthetic accessor uses the variable; reading an
//!   explicit top-level accessor puts it in the members and in the read ones
//!   (a read setter counts the corresponding getter); any other element of
//!   this library is used (`_useIdentifierElement`), except the class itself
//!   (only from a `ClassDeclaration`) and the executable itself (declared
//!   function or method); a class or extension member goes to the members
//!   and, if read, to the read ones; an enum's `values` reads all the
//!   constants; an unresolved name that is read goes to the unresolved ones;
//! * "read" is `_isReadIdentifier`: in a read context (not the target of
//!   `=`), and, if the parent is the whole expression statement, neither the
//!   operand of a prefix/postfix nor the target of an assignment other than
//!   `??=` (a property name has the property as its parent, and only the
//!   first rule applies);
//! * a type name (`visitNamedType`) uses the element, except the class itself
//!   and the interface in the type of a variable list that is not a field
//!   list or of an `is`;
//! * the annotation name, the constructor of each enum constant, the named
//!   constructor of a creation, of a redirection (`= C.n`, `this.n(…)`) and
//!   of a `super.n(…)`, and the public constructors of an interface type that
//!   a public `typedef` names.
//!
//! Report: the unused private class, mixin, enum and extension type (the
//! class's `@JS` uses it), typedef, function and top-level variable
//! (`_isUsedElement`, with `@pragma('vm:entry-point')`); the method, the
//! accessor and the named constructor (of a class with more than one) not
//! accessible from outside without a use or an override of a used member
//! (`_isUsedMember`); the field and the enum constant that are not read
//! (`_isReadMember`: a public one only counts as not read when it is static
//! in a private class or extension).
//! Written without compiling or running (2026-10-05).

use crate::constantes::avaliador::{Constante, Ctx, Motor};
use crate::resolve::OutlineTypes;
use crate::resolved::{BodyTypes, MemberRef, Resolved};
use crate::table::{CoreTypes, TypeTable};
use dartforge_diagnostics::codes::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{
    ClassId, ClassKind, Element, ExtensionId, FunctionElementId, FunctionKind, LibraryId, Program, TypedefId, UnitId, UnitRole, VariableId,
};
use dartforge_frontend::ast::{self, DeclKind, ExprId, ExprKind, ForInit, Initializer, MemberKind, StmtKind, TypeKind, UnaryOp};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};

/// An element of the verifier.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum El {
    Classe(ClassId),
    Extensao(ExtensionId),
    Typedef(TypedefId),
    Funcao(FunctionElementId),
    /// The top-level variable or the field.
    Variavel(VariableId),
    /// The synthetic read accessor of a variable.
    Getter(VariableId),
    /// The synthetic write accessor of a variable.
    Setter(VariableId),
}

#[derive(Default)]
struct Used {
    elements: HashSet<El>,
    members: HashSet<El>,
    read_values: HashSet<El>,
    unresolved: HashSet<SymbolId>,
}

fn inside(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

fn private(interner: &Interner, s: SymbolId) -> bool {
    interner.resolve(s).starts_with('_')
}

struct Context<'a> {
    program: &'a Program,
    interner: &'a Interner,
    lib: LibraryId,
}

impl Context<'_> {
    /// The element of a function member (the synthetic accessor counts through
    /// the variable).
    fn of_function(&self, f: FunctionElementId) -> El {
        let g = self.program.function(f);
        match g.variable {
            Some(v) => {
                if self.program.variable(v).setter == Some(f) {
                    El::Setter(v)
                } else {
                    El::Getter(v)
                }
            }
            None => El::Funcao(f),
        }
    }

    fn of_element(&self, e: Element) -> Option<El> {
        Some(match e {
            Element::Class(c) => El::Classe(c),
            Element::Extension(x) => El::Extensao(x),
            Element::Typedef(t) => El::Typedef(t),
            Element::Function(f) => self.of_function(f),
            Element::Variable(v) => El::Variavel(v),
            Element::Prefix(..) => return None,
        })
    }

    /// The library of the element.
    fn library(&self, e: El) -> LibraryId {
        let p = self.program;
        match e {
            El::Classe(c) => p.class(c).library,
            El::Extensao(x) => p.extension(x).library,
            El::Typedef(t) => p.typedefs[t.0 as usize].library,
            El::Funcao(f) => p.function(f).library,
            El::Variavel(v) | El::Getter(v) | El::Setter(v) => p.variable(v).library,
        }
    }

    /// The container (class or extension) and the member key.
    fn member_key(&self, e: El) -> Option<(Option<ClassId>, Option<ExtensionId>, String)> {
        let p = self.program;
        match e {
            El::Funcao(f) => {
                let g = p.function(f);
                let mut the_name = self.interner.resolve(g.name).to_string();
                if g.kind == FunctionKind::Setter && !the_name.ends_with("_=") {
                    the_name.push_str("_=");
                }
                Some((g.class, g.extension, the_name))
            }
            El::Getter(v) => {
                let x = p.variable(v);
                Some((x.class, x.extension, self.interner.resolve(x.name).to_string()))
            }
            El::Setter(v) => {
                let x = p.variable(v);
                Some((x.class, x.extension, format!("{}_=", self.interner.resolve(x.name))))
            }
            _ => None,
        }
    }

    /// The getter corresponding to a setter (`correspondingGetter`).
    fn getter_of(&self, e: El) -> Option<El> {
        match e {
            El::Setter(v) => Some(El::Getter(v)),
            El::Funcao(f) if self.program.function(f).kind == FunctionKind::Setter => {
                let (cls, extension, the_name) = self.member_key(e)?;
                let base = the_name.strip_suffix("_=").unwrap_or(&the_name).to_string();
                let s = self.interner.lookup(&base)?;
                let tgt = match (cls, extension) {
                    (Some(c), _) => {
                        let k = self.program.class(c);
                        k.instance_members.get(&s).or_else(|| k.static_members.get(&s)).copied()
                    }
                    (None, Some(x)) => {
                        let k = self.program.extension(x);
                        k.instance_members.get(&s).or_else(|| k.static_members.get(&s)).copied()
                    }
                    (None, None) => match self.program.library(self.program.function(f).library).declared.get(&s)?.getter? {
                        Element::Function(g) => Some(g),
                        _ => None,
                    },
                };
                tgt.map(|g| self.of_function(g))
            }
            _ => None,
        }
    }

    /// `_isPubliclyAccessible` of a method, accessor or constructor.
    fn accessible(&self, e: El, the_name: SymbolId) -> bool {
        if private(self.interner, the_name) {
            return false;
        }
        let p = self.program;
        let (cls, extension, static_, ctor, factory) = match e {
            El::Funcao(f) => {
                let g = p.function(f);
                let ctor = matches!(g.kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor);
                (g.class, g.extension, g.static_, ctor, g.factory)
            }
            El::Getter(v) | El::Setter(v) => {
                let x = p.variable(v);
                (x.class, x.extension, x.static_, false, false)
            }
            _ => return true,
        };
        if let Some(c) = cls {
            let k = p.class(c);
            if k.kind == ClassKind::Enum && ctor && !factory {
                return false;
            }
            if private(self.interner, k.name) && (static_ || ctor) {
                return false;
            }
        }
        if let Some(x) = extension {
            return p.extension(x).name.is_some_and(|n| !private(self.interner, n));
        }
        true
    }
}

/// `_overriddenElements`: the `getOverridden2` in the member's class, by the
/// `Name` of the verified library (`e.declaration` of each one).
fn overridden(cx: &Context<'_>, h: &mut crate::heranca::Heranca, p: &mut crate::heranca::ProvedorDoOutline<'_, '_>, e: El) -> Vec<El> {
    let Some((Some(c), _, text)) = cx.member_key(e) else { return Vec::new() };
    let Some(s) = cx.interner.lookup(&text) else { return Vec::new() };
    let the_name = crate::heranca::Nome::novo(cx.interner, cx.lib, s);
    h.overridden(p, c, the_name).unwrap_or_default().into_iter().map(|m| cx.of_function(m.funcao)).collect()
}

/// `_overridesUsedElement`.
fn overrides_used(
    cx: &Context<'_>,
    h: &mut crate::heranca::Heranca,
    p: &mut crate::heranca::ProvedorDoOutline<'_, '_>,
    e: El,
    used: &Used,
    seen: &mut HashSet<El>,
) -> bool {
    if !seen.insert(e) {
        return false;
    }
    for o in overridden(cx, h, p, e) {
        if used.members.contains(&o) || overrides_used(cx, h, p, o, used, seen) {
            return true;
        }
    }
    false
}

/// The types written in a variable list that is not a field list and in an
/// `is`, with the whole subtree (type arguments, parts of function and
/// record types): in them, the named interface type does not count as a use.
/// `visitNamedType` passes the `NamedType` itself as `parent`, so the rule
/// applies while `_enclosingVariableDeclaration` (only during the list's
/// type, `visitVariableDeclarationList`) or `_enclosingIsExpression` (only
/// during the type of the `is`) is set (3.6.2,
/// `unused_local_elements_verifier.dart:189-198`, `:336-347`, `:386-400`).
fn types_without_interface_use(a: &ast::Ast) -> HashSet<ast::TypeId> {
    let roots = types_without_interface_use_roots(a);
    let mut v = HashSet::new();
    fn descend(a: &ast::Ast, t: ast::TypeId, v: &mut HashSet<ast::TypeId>) {
        if !v.insert(t) {
            return;
        }
        match &a.ty(t).kind {
            TypeKind::Named { args, .. } => {
                for &x in args.iter() {
                    descend(a, x, v);
                }
            }
            TypeKind::Void => {}
            TypeKind::Function { return_type, type_params, parameters } => {
                if let Some(r) = return_type {
                    descend(a, *r, v);
                }
                for tp in type_params.iter() {
                    if let Some(b) = tp.bound {
                        descend(a, b, v);
                    }
                }
                params_list(a, parameters, v);
            }
            TypeKind::Record { positional, named } => {
                for &x in positional.iter() {
                    descend(a, x, v);
                }
                for (_, x) in named.iter() {
                    descend(a, *x, v);
                }
            }
        }
    }
    fn params_list(a: &ast::Ast, ps: &[ast::Parameter], v: &mut HashSet<ast::TypeId>) {
        for p in ps.iter() {
            if let Some(x) = p.ty {
                descend(a, x, v);
            }
            for tp in p.function_type_params.iter() {
                if let Some(b) = tp.bound {
                    descend(a, b, v);
                }
            }
            if let Some(f) = &p.function_parameters {
                params_list(a, f, v);
            }
        }
    }
    for t in roots {
        descend(a, t, &mut v);
    }
    v
}

/// The roots of [`types_without_interface_use`]: the written type of the list
/// and that of the `is`.
fn types_without_interface_use_roots(a: &ast::Ast) -> HashSet<ast::TypeId> {
    let mut v = HashSet::new();
    fn list(l: &ast::VariableList, v: &mut HashSet<ast::TypeId>) {
        if let Some(t) = l.ty {
            v.insert(t);
        }
    }
    fn of_collection(el: &ast::CollectionElement, v: &mut HashSet<ast::TypeId>) {
        match el {
            ast::CollectionElement::For { init, body, .. } => {
                if let Some(ForInit::Variables(l)) = init {
                    list(l, v);
                }
                of_collection(body, v);
            }
            ast::CollectionElement::ForIn { body, .. } => of_collection(body, v),
            ast::CollectionElement::If { then, else_, .. } => {
                of_collection(then, v);
                if let Some(x) = else_ {
                    of_collection(x, v);
                }
            }
            _ => {}
        }
    }
    for d in a.decls.iter() {
        if let DeclKind::Variables(l) = &d.kind {
            list(l, &mut v);
        }
    }
    for s in a.stmts.iter() {
        match &s.kind {
            StmtKind::Variables(l) | StmtKind::For { init: Some(ForInit::Variables(l)), .. } => list(l, &mut v),
            _ => {}
        }
    }
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::Is { ty, .. } => {
                v.insert(*ty);
            }
            ExprKind::List { elements, .. } | ExprKind::SetOrMap { elements, .. } => {
                for el in elements.iter() {
                    of_collection(el, &mut v);
                }
            }
            _ => {}
        }
    }
    v
}

/// `@pragma('vm:entry-point')` (`isPragmaVmEntryPoint`: the `pragma`
/// constructor of `dart.core` with the `name` field equal to
/// `vm:entry-point`) or, with `js`, `@JS(…)` (the `JS` constructor of the
/// `_js_annotations` library) in a list of annotations of unit `u`.
fn marked(cx: &Context<'_>, engine: &mut Option<Motor<'_>>, u: UnitId, metadata: &[ast::Annotation], js: bool) -> bool {
    let p = cx.program;
    let library_name = |l: LibraryId| {
        p.library(l).name.as_ref().map(|n| n.iter().map(|s| cx.interner.resolve(*s)).collect::<Vec<_>>().join(".")).unwrap_or_default()
    };
    for m in metadata {
        let Some(args) = &m.arguments else { continue };
        let Some(Element::Class(c)) = crate::anotacoes::annotation_element(p, cx.interner, u, m) else { continue };
        let k = p.class(c);
        let the_name = cx.interner.resolve(k.name);
        if js && the_name == "JS" && library_name(k.library) == "_js_annotations" {
            return true;
        }
        if the_name == "pragma" && library_name(k.library) == "dart.core" {
            // The `name` of the `pragma` is the first positional argument.
            let Some(first) = args.args.iter().find(|x| x.name.is_none()) else { continue };
            let a = &p.unit(u).ast;
            let value = match &a.expr(first.value).kind {
                ExprKind::String(s) => s.constant_value().map(|t| t.to_string_lossy()),
                _ => engine.as_mut().and_then(|mm| {
                    let cxm = Ctx::simples(u, p.unit(u).library);
                    match mm.avaliar(&cxm, first.value, true) {
                        Constante::Valor(v) => match v.estado {
                            crate::constantes::valor::Estado::Str(Some(t)) => Some(String::from_utf16_lossy(&t)),
                            _ => None,
                        },
                        Constante::Invalid(_) => None,
                    }
                }),
            };
            if value.as_deref() == Some("vm:entry-point") {
                return true;
            }
        }
    }
    false
}

/// The reports of library `lib`.
#[allow(clippy::too_many_arguments)]
pub fn elementos_nao_usados(
    program: &Program,
    interner: &Interner,
    table: &mut TypeTable,
    core: &CoreTypes,
    outline: &OutlineTypes,
    bodies: &BodyTypes,
    the_inferred: &HashSet<LibraryId>,
    lib: LibraryId,
) -> Vec<(UnitId, Diagnostic)> {
    let cx = Context { program, interner, lib };
    let mut used = Used::default();
    let units: Vec<UnitId> = program.library(lib).units.iter().copied().filter(|u| program.unit(*u).role != UnitRole::Patch).collect();
    let decl_class = |u: UnitId, d: ast::DeclId| {
        program.classes.iter().position(|c| c.decl.is_some_and(|r| r.unit == u && r.decl == d)).map(|i| ClassId(i as u32))
    };
    let node_function = |u: UnitId, f: ast::FunctionId| {
        program
            .functions
            .iter()
            .position(|e| matches!(e.node, dartforge_elements::model::FunctionRef::Function { unit, function } if unit == u && function == f))
            .map(|i| FunctionElementId(i as u32))
    };
    let member_constructor = |u: UnitId, m: ast::MemberId| {
        program
            .functions
            .iter()
            .position(|e| matches!(e.node, dartforge_elements::model::FunctionRef::Constructor { unit, member } if unit == u && member == m))
            .map(|i| FunctionElementId(i as u32))
    };

    for &u in &units {
        let a = &program.unit(u).ast;
        let Some(the_body) = bodies.units.get(u.0 as usize) else { continue };
        let pais = crate::lints_tipados::unit_parents(program, u);
        // The classes (`ClassDeclaration`) and the executables (declared functions
        // and methods) that contain each place.
        let classes: Vec<(Span, ClassId)> = a
            .decls
            .iter()
            .enumerate()
            .filter(|(_, d)| matches!(d.kind, DeclKind::Class(_)))
            .filter_map(|(i, d)| decl_class(u, ast::DeclId(i as u32)).map(|c| (d.span, c)))
            .collect();
        let mut executables: Vec<(Span, FunctionElementId)> = Vec::new();
        for d in a.decls.iter() {
            if let DeclKind::Function(f) = &d.kind
                && let Some(e) = node_function(u, *f)
            {
                executables.push((a.function(*f).span, e));
            }
        }
        for m in a.members.iter() {
            if let MemberKind::Method(f) = &m.kind
                && let Some(e) = node_function(u, *f)
            {
                executables.push((a.function(*f).span, e));
            }
        }
        let class_in = |s: Span| classes.iter().filter(|(r, _)| inside(s, *r)).min_by_key(|(r, _)| r.end - r.start).map(|x| x.1);
        let executable_in = |s: Span| executables.iter().filter(|(r, _)| inside(s, *r)).min_by_key(|(r, _)| r.end - r.start).map(|x| x.1);
        // `_useIdentifierElement`.
        let use_it = |used: &mut Used, e: El, where_: Span| {
            if cx.library(e) != lib {
                return;
            }
            if let El::Classe(c) = e
                && class_in(where_) == Some(c)
            {
                return;
            }
            if let El::Funcao(f) = e
                && executable_in(where_) == Some(f)
            {
                return;
            }
            used.elements.insert(e);
        };
        // `_addMemberAndCorrespondingGetter`.
        let read_member = |used: &mut Used, e: El| match cx.getter_of(e) {
            Some(g) => {
                used.members.insert(g);
                used.read_values.insert(g);
            }
            None if matches!(e, El::Setter(_)) => {}
            None => {
                used.read_values.insert(e);
            }
        };
        // The expression statements.
        let commands: HashSet<ExprId> = a
            .stmts
            .iter()
            .filter_map(|s| match &s.kind {
                StmtKind::Expression(e) => Some(*e),
                _ => None,
            })
            .collect();
        let parent_of = |e: ExprId| match pais.pai(e) {
            dartforge_frontend::pais::Pai::Expr(p) => Some(p),
            _ => None,
        };

        // The executable of an operator or `call` resolution (the node's
        // `staticElement`).
        let executable = |r: Option<&Resolved>| match r {
            Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::ExtensionMember { member: f, .. }) => {
                Some(cx.of_function(*f))
            }
            _ => None,
        };
        for (k, ex) in a.exprs.iter().enumerate() {
            let id = ExprId(k as u32);
            let (the_name, property) = match &ex.kind {
                ExprKind::Identifier(n) => (*n, false),
                ExprKind::Property { name, .. } => (*name, true),
                // `visitBinaryExpression`, `visitPrefixExpression`,
                // `visitPostfixExpression`, `visitIndexExpression` and
                // `visitAssignmentExpression`: the resolved operator is a used member
                // (`addMember`).
                ExprKind::Binary { .. } | ExprKind::Unary { .. } | ExprKind::Index { .. } | ExprKind::Assign { .. } => {
                    // The index that is the target of an assignment (`a[i] = v`, `a[i] ??= v`):
                    // the `writeOrReadElement` (3.6.2
                    // `unused_local_elements_verifier.dart:176-180`) is the `[]=`; the `[]`
                    // read by the compound assignment does not count.
                    let assignment_target = matches!(ex.kind, ExprKind::Index { .. })
                        && parent_of(id).is_some_and(|p| matches!(&a.expr(p).kind, ExprKind::Assign { target, .. } if *target == id));
                    let write = if assignment_target {
                        match the_body.get_resolved(id) {
                            Some(Resolved::Member { member: MemberRef::Function(f), .. }) | Some(Resolved::ExtensionMember { member: f, .. }) => {
                                let g = program.function(*f);
                                let the_name = interner.lookup("[]=");
                                let sibling = the_name.and_then(|n| match (g.class, g.extension) {
                                    (Some(c), _) => program.class(c).instance_members.get(&n).copied(),
                                    (None, Some(x)) => program.extension(x).instance_members.get(&n).copied(),
                                    _ => None,
                                });
                                if interner.resolve(g.name) == "[]" { sibling } else { Some(*f) }
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };
                    match write {
                        Some(f) => {
                            used.members.insert(cx.of_function(f));
                        }
                        None if !assignment_target => {
                            if let Some(el) = executable(the_body.get_resolved(id)) {
                                used.members.insert(el);
                            }
                        }
                        None => {}
                    }
                    continue;
                }
                // `visitDotShorthandConstructorInvocation`,
                // `visitDotShorthandInvocation` and
                // `visitDotShorthandPropertyAccess` (3.13, main checkout
                // `unused_local_elements_verifier.dart:203-238`): the context's
                // declaration is used (`addElement`, without the class-itself
                // rule); the static member by name (`visitSimpleIdentifier` of
                // the `memberName`/`propertyName`). The constructor is handled at the
                // call.
                ExprKind::DotShorthand { name, .. } => {
                    if let Some(Resolved::Element(Element::Class(d))) = the_body.get_resolved(id) {
                        if cx.library(El::Classe(*d)) == lib {
                            used.elements.insert(El::Classe(*d));
                        }
                        if let Some(&f) = program.class(*d).static_members.get(&name.sym) {
                            let el = cx.of_function(f);
                            use_it(&mut used, el, ex.span);
                            used.members.insert(el);
                            read_member(&mut used, el);
                        } else if let Some(&v) = program.class(*d).enum_constants.iter().find(|&&v| program.variable(v).name == name.sym) {
                            // The enum constant (`.v`) has no getter in the model: read like that of
                            // `_E.v`.
                            let el = El::Getter(v);
                            use_it(&mut used, el, ex.span);
                            used.members.insert(el);
                            read_member(&mut used, el);
                        }
                    }
                    continue;
                }
                // The creation: the named constructor (its identifier; in `const C.n()`
                // the parser leaves `C.n` as a two-part type, without `constructor`).
                ExprKind::InstanceCreation { .. } | ExprKind::Call { .. } => {
                    // `visitFunctionExpressionInvocation`: the implicit `call` (`addElement`).
                    if matches!(ex.kind, ExprKind::Call { .. })
                        && let Some(el) = executable(the_body.get_resolved(id))
                    {
                        used.elements.insert(el);
                    }
                    if let Some(Resolved::Constructor(f)) = the_body.get_resolved(id) {
                        let written = match &ex.kind {
                            ExprKind::InstanceCreation { .. } => true,
                            ExprKind::Call { target, .. } => matches!(a.expr(*target).kind, ExprKind::Property { .. } | ExprKind::DotShorthand { .. }),
                            _ => false,
                        };
                        let named_one = !interner.resolve(program.function(*f).name).is_empty();
                        if written && named_one {
                            use_it(&mut used, El::Funcao(*f), ex.span);
                            used.members.insert(El::Funcao(*f));
                        }
                    }
                    continue;
                }
                _ => continue,
            };
            let parent = parent_of(id);
            // `inGetterContext` and `_isReadIdentifier`.
            let target_of = |p: ExprId| matches!(&a.expr(p).kind, ExprKind::Assign { target, .. } if *target == id);
            let simple_assignment = parent.is_some_and(|p| matches!(&a.expr(p).kind, ExprKind::Assign { op: ast::AssignOp::Assign, target, .. } if *target == id));
            let mut read_value = !simple_assignment;
            if read_value
                && !property
                && let Some(p) = parent
                && commands.contains(&p)
            {
                match &a.expr(p).kind {
                    ExprKind::Unary { operand, .. } if *operand == id => read_value = false,
                    ExprKind::Assign { op, target, .. } if *target == id => read_value = matches!(op, ast::AssignOp::Compound(ast::BinaryOp::IfNull)),
                    _ => {}
                }
            }
            let written = parent.is_some_and(|p| {
                target_of(p)
                    || matches!(&a.expr(p).kind, ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } if *operand == id)
            });
            match the_body.get_resolved(id) {
                None | Some(Resolved::Dynamic) => {
                    if read_value {
                        used.unresolved.insert(the_name.sym);
                    }
                }
                Some(Resolved::Local(_) | Resolved::Parameter { .. } | Resolved::TypeParameter(_) | Resolved::Prefix(_)) => {}
                Some(Resolved::Element(e)) => {
                    let Some(el) = cx.of_element(*e) else { continue };
                    match el {
                        // The static extension field (in the model, read through the variable):
                        // the field's synthetic getter is the member read (`_A.f1`, `_baz` inside
                        // the extension).
                        El::Variavel(v) | El::Getter(v) | El::Setter(v)
                            if program.variable(v).class.is_none() && program.variable(v).extension.is_some() =>
                        {
                            if read_value {
                                used.members.insert(El::Getter(v));
                                read_member(&mut used, El::Getter(v));
                            }
                            if written {
                                use_it(&mut used, El::Setter(v), ex.span);
                            }
                        }
                        // The top-level variable (synthetic accessors).
                        El::Variavel(v) | El::Getter(v) | El::Setter(v) if program.variable(v).class.is_none() => {
                            if read_value {
                                used.elements.insert(El::Variavel(v));
                            }
                        }
                        // The explicit top-level accessor.
                        El::Funcao(f) if matches!(program.function(f).kind, FunctionKind::Getter | FunctionKind::Setter) => {
                            if read_value {
                                used.members.insert(el);
                                read_member(&mut used, el);
                            }
                            if written {
                                // The `writeElement`: the setter of the same name, or the getter of the
                                // recovery (without a setter).
                                let setter = program.library(program.function(f).library).declared.get(&program.function(f).name).and_then(|b| b.setter);
                                match setter {
                                    Some(Element::Function(g)) => use_it(&mut used, El::Funcao(g), ex.span),
                                    _ => use_it(&mut used, el, ex.span),
                                }
                                // The `readElement` of the compound assignment and of `++`/`--`: the getter
                                // (`_useIdentifierElement`, 3.6.2
                                // `unused_local_elements_verifier.dart:284`).
                                if !simple_assignment {
                                    let getter = if program.function(f).kind == FunctionKind::Getter { Some(el) } else { cx.getter_of(el) };
                                    if let Some(g) = getter {
                                        use_it(&mut used, g, ex.span);
                                    }
                                }
                            }
                            if !read_value && !written {
                                use_it(&mut used, el, ex.span);
                            }
                        }
                        _ => use_it(&mut used, el, ex.span),
                    }
                }
                Some(Resolved::Member { class, member, .. }) => {
                    let (el_read, el_write) = match member {
                        MemberRef::Variable(v) => (El::Getter(*v), El::Setter(*v)),
                        MemberRef::Function(f) => {
                            let el = cx.of_function(*f);
                            match el {
                                El::Getter(v) | El::Setter(v) => (El::Getter(v), El::Setter(v)),
                                other => (other, other),
                            }
                        }
                    };
                    // Enum `values`: all constants are read.
                    let k = program.class(*class);
                    if k.kind == ClassKind::Enum && interner.resolve(the_name.sym) == "values" {
                        for &c in k.enum_constants.iter() {
                            used.read_values.insert(El::Getter(c));
                        }
                        continue;
                    }
                    let mut targets: Vec<El> = Vec::new();
                    if read_value || !written {
                        targets.push(el_read);
                    }
                    if written {
                        targets.push(el_write);
                        // The `readElement` of the compound assignment and of `++`/`--`.
                        if !simple_assignment && !(read_value || !written) {
                            targets.push(cx.getter_of(el_write).unwrap_or(el_read));
                        }
                    }
                    for el in targets {
                        use_it(&mut used, el, ex.span);
                        if matches!(el, El::Funcao(f) if executable_in(ex.span) == Some(f)) {
                            continue;
                        }
                        used.members.insert(el);
                        if read_value {
                            read_member(&mut used, el);
                        }
                    }
                }
                Some(Resolved::ExtensionMember { member, .. }) => {
                    let el = cx.of_function(*member);
                    use_it(&mut used, el, ex.span);
                    if !matches!(el, El::Funcao(f) if executable_in(ex.span) == Some(f)) {
                        used.members.insert(el);
                        if read_value {
                            read_member(&mut used, el);
                        }
                    }
                }
                Some(Resolved::Constructor(f)) => {
                    use_it(&mut used, El::Funcao(*f), ex.span);
                    used.members.insert(El::Funcao(*f));
                }
            }
        }

        // The bare name of a refutable pattern (`case _limite:`): the
        // `SimpleIdentifier` of the `ConstantPattern`, read.
        for (p, r) in the_body.pattern_names.iter() {
            let where_ = a.pattern(*p).span;
            match r {
                Resolved::Element(e) => {
                    let Some(el) = cx.of_element(*e) else { continue };
                    match el {
                        El::Variavel(v) | El::Getter(v) | El::Setter(v) if program.variable(v).class.is_none() && program.variable(v).extension.is_some() => {
                            used.members.insert(El::Getter(v));
                            read_member(&mut used, El::Getter(v));
                        }
                        El::Variavel(v) | El::Getter(v) | El::Setter(v) if program.variable(v).class.is_none() => {
                            used.elements.insert(El::Variavel(v));
                        }
                        El::Funcao(f) if matches!(program.function(f).kind, FunctionKind::Getter | FunctionKind::Setter) => {
                            used.members.insert(el);
                            read_member(&mut used, el);
                        }
                        _ => use_it(&mut used, el, where_),
                    }
                }
                Resolved::Member { member, .. } => {
                    let el = match member {
                        MemberRef::Variable(v) => El::Getter(*v),
                        MemberRef::Function(f) => match cx.of_function(*f) {
                            El::Getter(v) | El::Setter(v) => El::Getter(v),
                            other => other,
                        },
                    };
                    use_it(&mut used, el, where_);
                    used.members.insert(el);
                    read_member(&mut used, el);
                }
                Resolved::ExtensionMember { member, .. } => {
                    let el = cx.of_function(*member);
                    use_it(&mut used, el, where_);
                    used.members.insert(el);
                    read_member(&mut used, el);
                }
                _ => {}
            }
        }

        // `visitPatternField`: `addMember` and `addReadMember` of the field's
        // element.
        for p in a.patterns.iter() {
            let ast::PatternKind::Object { fields, .. } = &p.kind else { continue };
            for f in fields.iter() {
                let el = match the_body.field_elements.get(&f.pattern) {
                    Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => El::Getter(*v),
                    Some(Resolved::Member { member: MemberRef::Function(g), .. }) | Some(Resolved::ExtensionMember { member: g, .. }) => cx.of_function(*g),
                    _ => continue,
                };
                used.members.insert(el);
                read_member(&mut used, el);
            }
        }

        // `visitRelationalPattern` (`unused_local_elements_verifier.dart:247-251`):
        // `addMember` and `addReadMember` of the operator.
        for el in the_body.relational_operators.values() {
            let el = match el {
                Resolved::Member { member: MemberRef::Function(g), .. } | Resolved::ExtensionMember { member: g, .. } => cx.of_function(*g),
                _ => continue,
            };
            used.members.insert(el);
            read_member(&mut used, el);
        }

        // `visitNamedType`.
        let without_interface = types_without_interface_use(a);
        for (ti, t) in a.types.iter().enumerate() {
            let TypeKind::Named { name, .. } = &t.kind else { continue };
            let element = match &name[..] {
                [n] => {
                    if crate::anotacoes::sombreado(a, interner, n.span.start, n.sym) {
                        continue;
                    }
                    program.lookup_na_unidade(u, n.sym).and_then(|b| b.getter)
                }
                [p, n, ..] => program.lookup_prefixed_na_unidade(u, p.sym, n.sym).and_then(|b| b.getter),
                [] => None,
            };
            let Some(el) = element.and_then(|e| cx.of_element(e)) else { continue };
            if matches!(el, El::Classe(_)) && without_interface.contains(&ast::TypeId(ti as u32)) {
                continue;
            }
            use_it(&mut used, el, t.span);
        }
        // The annotation names.
        for m in dartforge_frontend::pais::todas_as_anotacoes(a, &program.unit(u).unit) {
            if let Some(el) = crate::anotacoes::annotation_element(program, interner, u, m).and_then(|e| cx.of_element(e)) {
                use_it(&mut used, el, m.span);
            }
        }
        // The constructors of the enum constants, the redirection ones and those
        // of `super.n(…)`.
        for (di, d) in a.decls.iter().enumerate() {
            let DeclKind::Enum(x) = &d.kind else { continue };
            let Some(c) = decl_class(u, ast::DeclId(di as u32)) else { continue };
            for k in x.constants.iter() {
                let the_name = k.constructor.map(|n| n.sym).or_else(|| interner.lookup(""));
                if let Some(f) = the_name.and_then(|n| program.class(c).constructors.get(&n)) {
                    used.elements.insert(El::Funcao(*f));
                }
            }
        }
        for (mi, m) in a.members.iter().enumerate() {
            let MemberKind::Constructor(k) = &m.kind else { continue };
            // The `returnType` of the `ConstructorDeclaration` is a `SimpleIdentifier`
            // with the owner type: outside a `ClassDeclaration` (`_enclosingClass` is
            // only set there), it uses the enum or the extension type (3.6.2,
            // `unused_local_elements_verifier.dart:255-286`, `:380-382`). The primary
            // one (3.13) does not have that identifier.
            let primary = a.decls.iter().any(|d| match &d.kind {
                DeclKind::Class(x) => x.primary_constructor == Some(ast::MemberId(mi as u32)),
                DeclKind::Enum(x) => x.primary_constructor == Some(ast::MemberId(mi as u32)),
                _ => false,
            });
            if !primary
                && let Some(eu) = member_constructor(u, ast::MemberId(mi as u32))
                && let Some(cls) = program.function(eu).class
                && matches!(program.class(cls).kind, ClassKind::Enum | ClassKind::ExtensionType)
            {
                used.elements.insert(El::Classe(cls));
            }
            let mut mark = |c: ClassId, n: Option<ast::Name>| {
                let Some(n) = n else { return };
                if let Some(&f) = program.class(c).constructors.get(&n.sym) {
                    use_it(&mut used, El::Funcao(f), m.span);
                    used.members.insert(El::Funcao(f));
                }
            };
            // The redirection is resolved even in a duplicate constructor (which also
            // has an element in the analyzer, but not in our model).
            if let Some(r) = &k.redirect
                && let Some((c, ctor)) = crate::redirection::class_and_constructor(program, u, r)
            {
                mark(c, ctor);
            }
            let Some(eu) = member_constructor(u, ast::MemberId(mi as u32)) else { continue };
            let Some(cls) = program.function(eu).class else { continue };
            for i in k.initializers.iter() {
                match i {
                    Initializer::Redirect { constructor, .. } => mark(cls, *constructor),
                    Initializer::Super { constructor, .. } => {
                        if let Some(s) = program.class(cls).supertype_class {
                            mark(s, *constructor);
                        }
                    }
                    _ => {}
                }
            }
        }
        // `visitGenericTypeAlias`: a public typedef of an interface type uses its
        // public constructors.
        for d in a.decls.iter() {
            let DeclKind::Typedef(t) = &d.kind else { continue };
            let ast::TypedefKind::Alias(ty) = &t.kind else { continue };
            if private(interner, t.name.sym) {
                continue;
            }
            let Some(tgt) = outline.tipos_escritos.get(&(u, *ty)) else { continue };
            if let crate::table::Type::Interface { class, .. } | crate::table::Type::ExtensionType { decl: class, .. } = table.get(*tgt) {
                for (n, f) in program.class(*class).constructors.iter() {
                    if !private(interner, *n) {
                        used.elements.insert(El::Funcao(*f));
                    }
                }
            }
        }
    }

    // `_overridesUsedElement` of each instance member and constructor of the
    // library's classes, through `InheritanceManager3`, before the engine
    // (which takes the table).
    let mut override_used: HashSet<El> = HashSet::new();
    {
        let mut inheritance = crate::heranca::Heranca::default();
        let mut prov = crate::heranca::ProvedorDoOutline { program, interner, core, outline, table: &mut *table };
        let mut candidates: Vec<El> = Vec::new();
        for (i, k) in program.classes.iter().enumerate() {
            if k.library != lib || k.decl.is_none() {
                continue;
            }
            let c = ClassId(i as u32);
            for &f in k.instance_members.values().chain(k.constructors.values()) {
                candidates.push(cx.of_function(f));
            }
            for &v in &k.fields {
                candidates.push(El::Getter(v));
            }
            let _ = c;
        }
        for el in candidates {
            if overrides_used(&cx, &mut inheritance, &mut prov, el, &used, &mut HashSet::new()) {
                override_used.insert(el);
            }
        }
    }
    // O relato.
    let mut engine: Option<Motor<'_>> = None;
    let needs_engine = units.iter().any(|&u| {
        dartforge_frontend::pais::todas_as_anotacoes(&program.unit(u).ast, &program.unit(u).unit)
            .iter()
            .any(|m| m.name.last().is_some_and(|n| interner.resolve(n.sym) == "pragma"))
    });
    if needs_engine {
        engine = Some(Motor::novo(program, interner, table, core, outline, bodies, the_inferred));
    }
    let mut output: Vec<(UnitId, Diagnostic)> = Vec::new();
    // The fields declared by a primary constructor parameter (3.13), with two
    // arguments: joined into `output` at the end.
    let mut of_primary: Vec<(UnitId, Diagnostic)> = Vec::new();
    let mut report = |u: UnitId, code, n: ast::Name, displayed: String| {
        output.push((u, Diagnostic::with_code(code, n.span, [displayed.as_str()])));
    };
    let used_element = |el: El, metadata: &[ast::Annotation], u: UnitId, engine: &mut Option<Motor<'_>>| {
        used.elements.contains(&el) || marked(&cx, engine, u, metadata, false)
    };
    let used_member = |el: El, the_name: SymbolId, metadata: &[ast::Annotation], u: UnitId, engine: &mut Option<Motor<'_>>| {
        cx.accessible(el, the_name)
            || marked(&cx, engine, u, metadata, false)
            || used.members.contains(&el)
            || used.elements.contains(&el)
            || override_used.contains(&el)
    };
    // `_isReadMember` of a field or enum constant.
    let read_value = |v: VariableId| {
        let x = program.variable(v);
        let static_ = x.static_ || x.const_ && x.class.is_some_and(|c| program.class(c).kind == ClassKind::Enum);
        let private_container = x.class.is_some_and(|c| private(interner, program.class(c).name))
            || x.extension.is_some_and(|e| program.extension(e).name.is_none_or(|n| private(interner, n)));
        if !private(interner, x.name) && !(private_container && static_) {
            return true;
        }
        if used.read_values.contains(&El::Getter(v)) || used.unresolved.contains(&x.name) {
            return true;
        }
        if static_ {
            return false;
        }
        override_used.contains(&El::Getter(v))
    };
    let variables_of = |u: UnitId, decl: Option<ast::DeclId>, mem: Option<ast::MemberId>| -> HashMap<usize, VariableId> {
        let mut m = HashMap::new();
        for (i, x) in program.variables.iter().enumerate() {
            match x.node {
                dartforge_elements::model::VariableRef::TopLevel { unit, decl: d, index } if unit == u && Some(d) == decl => {
                    m.insert(index, VariableId(i as u32));
                }
                dartforge_elements::model::VariableRef::Field { unit, member, index } if unit == u && Some(member) == mem => {
                    m.insert(index, VariableId(i as u32));
                }
                dartforge_elements::model::VariableRef::EnumConstant { unit, decl: d, index } if unit == u && Some(d) == decl => {
                    m.insert(index, VariableId(i as u32));
                }
                _ => {}
            }
        }
        m
    };
    for &u in &units {
        let a = &program.unit(u).ast;
        for (di, d) in a.decls.iter().enumerate() {
            let did = ast::DeclId(di as u32);
            let mut members: &[ast::MemberId] = &[];
            let mut primary: Option<ast::MemberId> = None;
            match &d.kind {
                DeclKind::Class(x) => {
                    members = &x.members;
                    // A class alias (`class _A = S with M;`) is not visited by the 3.6.2
                    // verifier (there is no `visitClassTypeAlias`): it is never reported.
                    if let Some(c) = decl_class(u, did)
                        && !x.mixin_application
                        && private(interner, x.name.sym)
                        && !used_element(El::Classe(c), &d.metadata, u, &mut engine)
                        && !marked(&cx, &mut engine, u, &d.metadata, true)
                    {
                        report(u, w::UNUSED_ELEMENT, x.name, interner.resolve(x.name.sym).to_string());
                    }
                    primary = x.primary_constructor;
                }
                DeclKind::Mixin(x) => {
                    members = &x.members;
                    if let Some(c) = decl_class(u, did)
                        && private(interner, x.name.sym)
                        && !used_element(El::Classe(c), &d.metadata, u, &mut engine)
                    {
                        report(u, w::UNUSED_ELEMENT, x.name, interner.resolve(x.name.sym).to_string());
                    }
                }
                DeclKind::Enum(x) => {
                    members = &x.members;
                    if let Some(c) = decl_class(u, did)
                        && private(interner, x.name.sym)
                        && !used_element(El::Classe(c), &d.metadata, u, &mut engine)
                    {
                        report(u, w::UNUSED_ELEMENT, x.name, interner.resolve(x.name.sym).to_string());
                    }
                    // The constants.
                    let vs = variables_of(u, Some(did), None);
                    for (i, k) in x.constants.iter().enumerate() {
                        if let Some(&v) = vs.get(&i)
                            && !read_value(v)
                        {
                            report(u, w::UNUSED_FIELD, k.name, interner.resolve(k.name.sym).to_string());
                        }
                    }
                    primary = x.primary_constructor;
                }
                DeclKind::ExtensionType(x) => {
                    members = &x.members;
                    if let Some(c) = decl_class(u, did)
                        && private(interner, x.name.sym)
                        && !used_element(El::Classe(c), &d.metadata, u, &mut engine)
                    {
                        report(u, w::UNUSED_ELEMENT, x.name, interner.resolve(x.name.sym).to_string());
                    }
                }
                DeclKind::Extension(x) => members = &x.members,
                DeclKind::Typedef(t) => {
                    let tgt = program.typedefs.iter().position(|e| e.decl.unit == u && e.decl.decl == did).map(|i| TypedefId(i as u32));
                    if let Some(tid) = tgt
                        && private(interner, t.name.sym)
                        && !used_element(El::Typedef(tid), &d.metadata, u, &mut engine)
                    {
                        report(u, w::UNUSED_ELEMENT, t.name, interner.resolve(t.name.sym).to_string());
                    }
                }
                DeclKind::Function(f) => {
                    let g = a.function(*f);
                    let (Some(n), Some(e)) = (g.name, node_function(u, *f)) else { continue };
                    let el = cx.of_function(e);
                    let the_used = match g.kind {
                        ast::FunctionKind::Getter | ast::FunctionKind::Setter => used_member(el, n.sym, &d.metadata, u, &mut engine),
                        _ => !private(interner, n.sym) || used_element(el, &d.metadata, u, &mut engine),
                    };
                    if !the_used {
                        report(u, w::UNUSED_ELEMENT, n, interner.resolve(n.sym).to_string());
                    }
                }
                DeclKind::Variables(l) => {
                    let vs = variables_of(u, Some(did), None);
                    for (i, v) in l.variables.iter().enumerate() {
                        if let Some(&id) = vs.get(&i)
                            && private(interner, v.name.sym)
                            && !used_element(El::Variavel(id), &d.metadata, u, &mut engine)
                        {
                            report(u, w::UNUSED_ELEMENT, v.name, interner.resolve(v.name.sym).to_string());
                        }
                    }
                }
            }
            // The members.
            let type_name = match &d.kind {
                DeclKind::Class(x) => interner.resolve(x.name.sym).to_string(),
                DeclKind::Mixin(x) => interner.resolve(x.name.sym).to_string(),
                DeclKind::Enum(x) => interner.resolve(x.name.sym).to_string(),
                DeclKind::ExtensionType(x) => interner.resolve(x.name.sym).to_string(),
                DeclKind::Extension(x) => x.name.map(|n| interner.resolve(n.sym).to_string()).unwrap_or_default(),
                _ => String::new(),
            };
            // `visitPrimaryConstructorDeclaration` (3.13,
            // `unused_local_elements_verifier.dart:1022-1057` of the main checkout):
            // the field of a declaring parameter (`final`/`var`) of a class or enum
            // that is not read is `UNUSED_FIELD_FROM_PRIMARY_CONSTRUCTOR` at the
            // parameter name, with the name and the keyword; an extension type never
            // reports. The parser elaborates the field with the parameter's name.
            let declaring_names: HashSet<usize> = match primary.map(|m| &a.member(m).kind) {
                Some(MemberKind::Constructor(k)) if matches!(d.kind, DeclKind::Class(_) | DeclKind::Enum(_)) => {
                    k.parameters.iter().filter(|p| p.this_).filter_map(|p| p.name).map(|n| n.span.start).collect()
                }
                _ => HashSet::new(),
            };
            for &mid in members {
                if Some(mid) == primary {
                    continue;
                }
                let mem = a.member(mid);
                match &mem.kind {
                    MemberKind::Method(f) => {
                        let g = a.function(*f);
                        let (Some(n), Some(e)) = (g.name, node_function(u, *f)) else { continue };
                        let el = cx.of_function(e);
                        if !used_member(el, n.sym, &mem.metadata, u, &mut engine) {
                            report(u, w::UNUSED_ELEMENT, n, interner.resolve(n.sym).to_string());
                        }
                    }
                    MemberKind::Field(l) => {
                        let vs = variables_of(u, None, Some(mid));
                        for (i, v) in l.variables.iter().enumerate() {
                            // The field of a declaring parameter `_` is a wildcard: it has no getter,
                            // and its `_isReadMember` is false (`field.getter == null`) even with reads
                            // of `_`.
                            let wildcard = declaring_names.contains(&v.name.span.start) && interner.resolve(v.name.sym) == "_";
                            if let Some(&id) = vs.get(&i)
                                && (wildcard || !read_value(id))
                            {
                                if declaring_names.contains(&v.name.span.start) {
                                    let keyword = if l.final_ { "final" } else { "var" };
                                    of_primary.push((
                                        u,
                                        Diagnostic::with_code(w::UNUSED_FIELD_FROM_PRIMARY_CONSTRUCTOR, v.name.span, [interner.resolve(v.name.sym), keyword]),
                                    ));
                                } else {
                                    report(u, w::UNUSED_FIELD, v.name, interner.resolve(v.name.sym).to_string());
                                }
                            }
                        }
                    }
                    MemberKind::Constructor(k) => {
                        let (Some(n), Some(e)) = (k.name, member_constructor(u, mid)) else { continue };
                        if interner.resolve(n.sym) == "new" || k.parte_primaria {
                            continue;
                        }
                        let Some(c) = program.function(e).class else { continue };
                        // The representation constructor of an extension type is not in the model,
                        // but it counts.
                        let representation = usize::from(program.class(c).kind == ClassKind::ExtensionType);
                        if program.class(c).constructors.len() + representation > 1 && !used_member(El::Funcao(e), n.sym, &mem.metadata, u, &mut engine) {
                            report(u, w::UNUSED_ELEMENT, n, format!("{type_name}.{}", interner.resolve(n.sym)));
                        }
                    }
                }
            }
        }
    }
    output.extend(of_primary);
    output.sort_by_key(|(u, d)| (u.0, d.span.start));
    output.dedup_by(|x, y| x.0 == y.0 && x.1.span == y.1.span && x.1.code == y.1.code);
    output
}
