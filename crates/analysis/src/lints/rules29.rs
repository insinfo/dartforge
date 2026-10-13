//! The twenty-ninth batch of lint rules
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8), written directly from the 3.6.2
//! emitters (`E:\references\dart-sdk-3.6.2\pkg\linter\lib\src\rules`):
//!
//! * `unreachable_from_main`: the library's declarations
//!   (`_DeclarationGatherer`: the top-level ones, each top-level variable, and
//!   the public constructors, fields and methods of public top-level types
//!   that override nothing and are not test methods) that no entry point
//!   (`main` or a top-level `@pragma('vm:entry-point')`) reaches through the
//!   references (`_ReferenceVisitor`). The references of each declaration
//!   are those of its subtree: the identifiers read (the left side of an
//!   assignment and the operand of `++`/`--` come in through the read and
//!   write elements), the constructor names (outside a constant pattern, with
//!   the type), the annotations, the object pattern fields, the named types
//!   only when they are a typedef, an extension type, a type argument or the
//!   type of an `external` member, the supertype clauses of classes and
//!   extension types, the superclass's unnamed constructor (a class without
//!   constructors, or a constructor without `super(…)`), the
//!   `super(…)`/`this(…)`, the target of a redirecting factory and the
//!   instance `toJson`. Each reference adds the top-level declaration that
//!   contains the element and, if the element and the type that contains it
//!   are public, that of the member itself.
//!
//!   Recorded divergence: documentation comment references (`[name]`) do
//!   not count; the `@pragma` only counts written with the literal string.
//!
//! Written without compiling or running (2026-10-05).

use super::codigos_g as c;
use super::rules::LintReport;
use crate::Unidade;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element, ExtensionId, FunctionElementId, FunctionKind, FunctionRef, Program, TypedefId, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, Annotation, Ast, DeclId, DeclKind, ExprId, ExprKind, Initializer, MemberId, MemberKind, PatternKind, TypeKind, UnaryOp};
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::resolved::{MemberRef, Resolved, UnitBodyTypes};
use dartforge_types::table::Type;
use std::collections::{HashMap, HashSet};

/// A declaration the rule can report.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Dec {
    TopLevel(UnitId, DeclId),
    TopLevelVar(UnitId, DeclId, usize),
    Member(UnitId, MemberId),
    Field(UnitId, MemberId, usize),
}

impl Dec {
    fn unit(self) -> UnitId {
        match self {
            Dec::TopLevel(u, ..) | Dec::TopLevelVar(u, ..) | Dec::Member(u, ..) | Dec::Field(u, ..) => u,
        }
    }
}

/// A referenced element.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Ref {
    Class(ClassId),
    Extension(ExtensionId),
    Typedef(TypedefId),
    Function(FunctionElementId),
    Variable(VariableId),
    /// A local element (variable, parameter, type parameter): the top-level
    /// declaration that contains the point.
    Position(UnitId, usize),
}

#[derive(Clone, Copy, Debug)]
enum Action {
    /// `_addDeclaration`.
    Element(Ref),
    /// `_addNamedType`: only the declaration of the element itself.
    Direct(Ref),
}

struct Cx<'a> {
    program: &'a Program,
    interner: &'a Interner,
    s: &'a super::Semantics<'a>,
    decls: HashSet<Dec>,
    /// The top-level regions of each unit: (start, end, declaration).
    top_regions: HashMap<UnitId, Vec<(usize, usize, Dec)>>,
}

impl Cx<'_> {
    fn is_private(&self, n: SymbolId) -> bool {
        self.interner.resolve(n).starts_with('_')
    }

    fn class_of_decl(&self, u: UnitId, d: DeclId) -> Option<ClassId> {
        (0..self.program.classes.len()).map(|i| ClassId(i as u32)).find(|c| self.program.class(*c).decl.is_some_and(|r| r.unit == u && r.decl == d))
    }

    /// `declarationMap[element]`.
    fn decl_of(&self, r: Ref) -> Option<Dec> {
        let program = self.program;
        let d = match r {
            Ref::Class(c) => {
                let x = program.class(c).decl?;
                Dec::TopLevel(x.unit, x.decl)
            }
            Ref::Extension(e) => {
                let x = program.extension(e).decl;
                Dec::TopLevel(x.unit, x.decl)
            }
            Ref::Typedef(t) => {
                let x = program.typedef(t).decl;
                Dec::TopLevel(x.unit, x.decl)
            }
            Ref::Function(f) => {
                let g = program.function(f);
                match g.node {
                    FunctionRef::Constructor { unit, member } => Dec::Member(unit, member),
                    FunctionRef::Function { unit, function } => {
                        let a = &program.unit(unit).ast;
                        if let Some(m) = a.members.iter().position(|m| matches!(m.kind, MemberKind::Method(x) if x == function)) {
                            Dec::Member(unit, MemberId(m as u32))
                        } else {
                            let d = a.decls.iter().position(|d| matches!(d.kind, DeclKind::Function(x) if x == function))?;
                            Dec::TopLevel(unit, DeclId(d as u32))
                        }
                    }
                    FunctionRef::None => return self.decl_of(Ref::Variable(g.variable?)),
                }
            }
            Ref::Variable(v) => match program.variable(v).node {
                VariableRef::TopLevel { unit, decl, index } => Dec::TopLevelVar(unit, decl, index),
                VariableRef::Field { unit, member, index } => Dec::Field(unit, member, index),
                _ => return None,
            },
            Ref::Position(u, p) => {
                let top_regions = self.top_regions.get(&u)?;
                top_regions.iter().find(|(i, f, _)| *i <= p && p < *f)?.2
            }
        };
        self.decls.contains(&d).then_some(d)
    }

    /// The element's name is private (`isPrivate`).
    fn is_private_element(&self, r: Ref) -> bool {
        let program = self.program;
        match r {
            Ref::Class(c) => self.is_private(program.class(c).name),
            Ref::Extension(e) => program.extension(e).name.is_none_or(|n| self.is_private(n)),
            Ref::Typedef(t) => self.is_private(program.typedef(t).name),
            Ref::Function(f) => match program.function(f).node {
                FunctionRef::Constructor { unit, member } => match &program.unit(unit).ast.member(member).kind {
                    MemberKind::Constructor(k) => k.name.is_some_and(|n| self.is_private(n.sym)),
                    _ => false,
                },
                _ => self.is_private(program.function(f).name),
            },
            Ref::Variable(v) => self.is_private(program.variable(v).name),
            Ref::Position(..) => false,
        }
    }

    /// `_addDeclaration`.
    fn add(&self, r: Ref, out: &mut HashSet<Dec>) {
        let program = self.program;
        let (class_id, extension) = match r {
            Ref::Function(f) => (program.function(f).class, program.function(f).extension),
            Ref::Variable(v) => (program.variable(v).class, program.variable(v).extension),
            _ => (None, None),
        };
        let top = match (class_id, extension) {
            (Some(c), _) => Ref::Class(c),
            (None, Some(e)) => Ref::Extension(e),
            _ => r,
        };
        if let Some(d) = self.decl_of(top) {
            out.insert(d);
        }
        if self.is_private_element(r) {
            return;
        }
        let private_owner = match (class_id, extension) {
            (Some(c), _) => self.is_private_element(Ref::Class(c)),
            (None, Some(e)) => self.is_private_element(Ref::Extension(e)),
            _ => return,
        };
        if private_owner {
            return;
        }
        if let Some(d) = self.decl_of(r) {
            out.insert(d);
        }
    }

    fn apply(&self, a: Action, out: &mut HashSet<Dec>) {
        match a {
            Action::Element(r) => self.add(r, out),
            Action::Direct(r) => {
                if let Some(d) = self.decl_of(r) {
                    out.insert(d);
                }
            }
        }
    }

    /// The element of a resolution.
    fn from_resolved(&self, u: UnitId, body: &UnitBodyTypes, e: ExprId, pos: usize) -> Option<Ref> {
        Some(match body.get_resolved(e)? {
            Resolved::Local(_) | Resolved::Parameter { .. } | Resolved::TypeParameter(_) => Ref::Position(u, pos),
            Resolved::Member { member: MemberRef::Variable(v), .. } | Resolved::Element(Element::Variable(v)) => Ref::Variable(*v),
            Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::Element(Element::Function(f)) => Ref::Function(*f),
            Resolved::ExtensionMember { member, .. } => Ref::Function(*member),
            Resolved::Constructor(f) => Ref::Function(*f),
            Resolved::Element(Element::Class(c)) => Ref::Class(*c),
            Resolved::Element(Element::Extension(x)) => Ref::Extension(*x),
            Resolved::Element(Element::Typedef(t)) => Ref::Typedef(*t),
            _ => return None,
        })
    }

    /// The read element that goes with the write one (the getter with the
    /// same name, for a written setter).
    fn read_of(&self, r: Ref) -> Option<Ref> {
        let Ref::Function(f) = r else { return Some(r) };
        let g = self.program.function(f);
        if g.kind != FunctionKind::Setter {
            return Some(r);
        }
        if let Some(v) = g.variable {
            return Some(Ref::Variable(v));
        }
        let text = self.interner.resolve(g.name);
        let name = self.interner.lookup(text.strip_suffix("_=").unwrap_or(text))?;
        let members = match (g.class, g.extension) {
            (Some(c), _) => {
                let x = self.program.class(c);
                x.instance_members.get(&name).or_else(|| x.static_members.get(&name)).copied()
            }
            (None, Some(e)) => {
                let x = self.program.extension(e);
                x.instance_members.get(&name).or_else(|| x.static_members.get(&name)).copied()
            }
            _ => match self.program.lookup_in_unit(self.s.unit, name)?.getter? {
                Element::Function(h) => Some(h),
                Element::Variable(v) => return Some(Ref::Variable(v)),
                _ => None,
            },
        };
        members.map(Ref::Function)
    }

    /// The element of a type name (`NamedType.element`), without a type
    /// parameter.
    fn type_element(&self, u: UnitId, body: &UnitBodyTypes, t: ast::TypeId) -> Option<Ref> {
        let a = &self.program.unit(u).ast;
        let TypeKind::Named { name, .. } = &a.ty(t).kind else { return None };
        let ty = body.tipos_de_anotacoes.get(&t).copied().or_else(|| self.s.outline.tipos_escritos.get(&(u, t)).copied());
        if ty.is_some_and(|x| matches!(self.s.table.get(x), Type::TypeParameter { .. })) {
            return None;
        }
        let b = match &name[..] {
            [n] => self.program.lookup_in_unit(u, n.sym),
            [p, n] => self.program.lookup_prefixed_in_unit(u, p.sym, n.sym),
            _ => None,
        }?;
        match b.getter? {
            Element::Class(c) => Some(Ref::Class(c)),
            Element::Typedef(x) => Some(Ref::Typedef(x)),
            Element::Extension(x) => Some(Ref::Extension(x)),
            _ => None,
        }
    }

    /// `getOverridden2(container, Name(lib, nome)) != null`.
    fn overrides_member(&self, class_id: ClassId, name: SymbolId, setter: bool) -> bool {
        let key = if setter { self.interner.lookup(&format!("{}_=", self.interner.resolve(name))) } else { Some(name) };
        let Some(key) = key else { return false };
        let own = self.program.class(class_id).library;
        let is_private_name = self.is_private(name);
        self.s.outline.hierarchy.get(class_id).is_some_and(|h| {
            h.supertypes.keys().any(|sc| {
                *sc != class_id && self.program.class(*sc).instance_members.get(&key).is_some_and(|f| !is_private_name || self.program.function(*f).library == own)
            })
        })
    }

    /// The constructor `name` (`None`: the unnamed one) of the class.
    fn constructor(&self, c: ClassId, name: Option<SymbolId>) -> Option<FunctionElementId> {
        let key = match name {
            Some(n) => n,
            None => self.interner.lookup("")?,
        };
        self.program.class(c).constructors.get(&key).copied()
    }
}

/// `_DeclarationGatherer.addDeclarations` of a unit.
fn unit_declarations(cx: &Cx<'_>, u: UnitId) -> Vec<Dec> {
    let a = &cx.program.unit(u).ast;
    let mut v = Vec::new();
    for (k, d) in a.decls.iter().enumerate() {
        let id = DeclId(k as u32);
        if let DeclKind::Variables(l) = &d.kind {
            v.extend((0..l.variables.len()).map(|i| Dec::TopLevelVar(u, id, i)));
            continue;
        }
        v.push(Dec::TopLevel(u, id));
        let (name, members, interface): (Option<SymbolId>, &[MemberId], Option<ClassId>) = match &d.kind {
            DeclKind::Class(x) if !x.mixin_application => (Some(x.name.sym), &x.members, cx.class_of_decl(u, id)),
            DeclKind::Enum(x) => (Some(x.name.sym), &x.members, cx.class_of_decl(u, id)),
            DeclKind::Mixin(x) => (Some(x.name.sym), &x.members, cx.class_of_decl(u, id)),
            DeclKind::Extension(x) => (x.name.map(|n| n.sym), &x.members, None),
            DeclKind::ExtensionType(x) => (Some(x.name.sym), &x.members, None),
            _ => continue,
        };
        // A private element (an unnamed extension too) has no members.
        if name.is_none_or(|n| cx.is_private(n)) {
            continue;
        }
        let is_enum = matches!(d.kind, DeclKind::Enum(_));
        let overrides = |n: SymbolId, setter: bool| interface.is_some_and(|c| cx.overrides_member(c, n, setter));
        for &m in members {
            match &a.member(m).kind {
                MemberKind::Constructor(k) => {
                    if !k.name.is_some_and(|n| cx.is_private(n.sym)) && !is_enum {
                        v.push(Dec::Member(u, m));
                    }
                }
                MemberKind::Field(l) => {
                    for (i, x) in l.variables.iter().enumerate() {
                        if !cx.is_private(x.name.sym) && !overrides(x.name.sym, false) {
                            v.push(Dec::Field(u, m, i));
                        }
                    }
                }
                MemberKind::Method(f) => {
                    let f = a.function(*f);
                    let Some(n) = f.name else { continue };
                    if cx.is_private(n.sym) {
                        continue;
                    }
                    let text = cx.interner.resolve(n.sym);
                    let is_test = text.starts_with("test_") || text.starts_with("solo_test_") || text == "setUp" || text == "tearDown";
                    if !overrides(n.sym, f.kind == ast::FunctionKind::Setter) && !is_test {
                        v.push(Dec::Member(u, m));
                    }
                }
            }
        }
    }
    v
}

/// The regions (the subtree) of a declaration.
fn regions(a: &Ast, d: Dec) -> Vec<(usize, usize)> {
    let with_annotations = |sp: Span, metadata: &[Annotation]| {
        let start = metadata.iter().map(|m| m.span.start).min().map_or(sp.start, |x| x.min(sp.start));
        (start, sp.end)
    };
    let variable = |l: &ast::VariableList, i: usize| {
        let x = &l.variables[i];
        let end = x.initializer.map_or(x.name.span.end, |e| a.expr(e).span.end);
        let mut v = vec![(x.name.span.start, end)];
        if let Some(t) = l.ty {
            let sp = a.ty(t).span;
            v.push((sp.start, sp.end));
        }
        v
    };
    match d {
        Dec::TopLevel(_, k) => {
            let x = a.decl(k);
            vec![with_annotations(x.span, &x.metadata)]
        }
        Dec::Member(_, m) => {
            let x = a.member(m);
            vec![with_annotations(x.span, &x.metadata)]
        }
        Dec::TopLevelVar(_, k, i) => match &a.decl(k).kind {
            DeclKind::Variables(l) => variable(l, i),
            _ => Vec::new(),
        },
        Dec::Field(_, m, i) => match &a.member(m).kind {
            MemberKind::Field(l) => variable(l, i),
            _ => Vec::new(),
        },
    }
}

/// The positional references of a unit: (offset, action).
fn references(cx: &Cx<'_>, u: UnitId) -> Vec<(usize, Action)> {
    let program = cx.program;
    let unit_data = program.unit(u);
    let a = &unit_data.ast;
    let body = &cx.s.bodies.units[u.0 as usize];
    let mut out: Vec<(usize, Action)> = Vec::new();
    // The left side of assignments and the operands of `++`/`--`: without a
    // `staticElement`, they come in through the read and write elements.
    let mut written: HashSet<ExprId> = HashSet::new();
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::Assign { target, .. } => {
                written.insert(*target);
            }
            ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } => {
                written.insert(*operand);
            }
            _ => {}
        }
    }
    // The constant patterns (`_patternLevel`).
    let mut constants: Vec<Span> = Vec::new();
    for p in a.patterns.iter() {
        if let PatternKind::Constant(e) = &p.kind {
            constants.push(a.expr(*e).span);
        }
    }
    let in_pattern = |sp: Span| constants.iter().any(|c| c.start <= sp.start && sp.end <= c.end);
    // The parts of a constructor name (`ConstructorName`): the type and the name.
    let mut constructor_names: HashSet<ExprId> = HashSet::new();
    let mark = |mut x: ExprId, names: &mut HashSet<ExprId>| loop {
        names.insert(x);
        match &a.expr(x).kind {
            ExprKind::Property { target, .. } | ExprKind::TypeArguments { target, .. } => x = *target,
            _ => break,
        }
    };
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        if !matches!(body.get_resolved(id), Some(Resolved::Constructor(_))) {
            continue;
        }
        match &e.kind {
            ExprKind::Call { target, .. } => mark(*target, &mut constructor_names),
            ExprKind::Property { .. } => mark(id, &mut constructor_names),
            _ => {}
        }
    }
    let constructor_type = |f: FunctionElementId, callee: Option<ExprId>| -> Option<Ref> {
        // The type as written: the typedef when the name is a typedef's.
        let mut x = callee?;
        loop {
            match &a.expr(x).kind {
                ExprKind::Identifier(_) => break,
                ExprKind::Property { target, .. } | ExprKind::TypeArguments { target, .. } => x = *target,
                _ => return program.function(f).class.map(Ref::Class),
            }
        }
        match body.get_resolved(x) {
            Some(Resolved::Element(Element::Typedef(t))) => Some(Ref::Typedef(*t)),
            Some(Resolved::Element(Element::Class(c))) => Some(Ref::Class(*c)),
            _ => program.function(f).class.map(Ref::Class),
        }
    };
    for (k, e) in a.exprs.iter().enumerate() {
        let id = ExprId(k as u32);
        let pos = e.span.start;
        match &e.kind {
            ExprKind::Identifier(_) | ExprKind::Property { .. } if !written.contains(&id) && !constructor_names.contains(&id) => {
                if let Some(r) = cx.from_resolved(u, body, id, pos) {
                    out.push((pos, Action::Element(r)));
                }
            }
            ExprKind::Assign { op, target, .. } => {
                if matches!(a.expr(*target).kind, ExprKind::Identifier(_) | ExprKind::Property { .. })
                    && let Some(r) = cx.from_resolved(u, body, *target, pos)
                {
                    out.push((pos, Action::Element(r)));
                    if *op != ast::AssignOp::Assign
                        && let Some(l) = cx.read_of(r)
                    {
                        out.push((pos, Action::Element(l)));
                    }
                }
            }
            ExprKind::Unary { op: UnaryOp::PrefixInc | UnaryOp::PrefixDec | UnaryOp::PostfixInc | UnaryOp::PostfixDec, operand } => {
                if matches!(a.expr(*operand).kind, ExprKind::Identifier(_) | ExprKind::Property { .. })
                    && let Some(r) = cx.from_resolved(u, body, *operand, pos)
                {
                    out.push((pos, Action::Element(r)));
                    if let Some(l) = cx.read_of(r) {
                        out.push((pos, Action::Element(l)));
                    }
                }
            }
            _ => {}
        }
        // `visitConstructorName`: the creation and the constructor tear-off.
        if let Some(Resolved::Constructor(f)) = body.get_resolved(id)
            && !in_pattern(e.span)
        {
            let callee = match &e.kind {
                ExprKind::Call { target, .. } => Some(*target),
                ExprKind::Property { .. } => Some(id),
                _ => None,
            };
            out.push((pos, Action::Element(Ref::Function(*f))));
            let ty = match &e.kind {
                ExprKind::InstanceCreation { ty, .. } => cx.type_element(u, body, *ty).or_else(|| program.function(*f).class.map(Ref::Class)),
                _ => constructor_type(*f, callee),
            };
            if let Some(t) = ty {
                out.push((pos, Action::Element(t)));
            }
        }
    }
    // `visitNamedType`: the types in a type argument list and those of an
    // `external` member.
    let mut in_arguments: HashSet<ast::TypeId> = HashSet::new();
    fn mark_type(a: &Ast, t: ast::TypeId, v: &mut HashSet<ast::TypeId>) {
        if !v.insert(t) {
            return;
        }
        match &a.ty(t).kind {
            TypeKind::Named { args, .. } => {
                for x in args.iter() {
                    mark_type(a, *x, v);
                }
            }
            TypeKind::Function { return_type, type_params, parameters } => {
                if let Some(r) = return_type {
                    mark_type(a, *r, v);
                }
                for p in type_params.iter() {
                    if let Some(b) = p.bound {
                        mark_type(a, b, v);
                    }
                }
                fn params(a: &Ast, ps: &[ast::Parameter], v: &mut HashSet<ast::TypeId>) {
                    for p in ps {
                        if let Some(t) = p.ty {
                            mark_type(a, t, v);
                        }
                        if let Some(i) = &p.function_parameters {
                            params(a, i, v);
                        }
                    }
                }
                params(a, parameters, v);
            }
            TypeKind::Record { positional, named } => {
                for x in positional.iter() {
                    mark_type(a, *x, v);
                }
                for (_, x) in named.iter() {
                    mark_type(a, *x, v);
                }
            }
            TypeKind::Void => {}
        }
    }
    let mut roots: Vec<ast::TypeId> = Vec::new();
    for t in a.types.iter() {
        if let TypeKind::Named { args, .. } = &t.kind {
            roots.extend(args.iter().copied());
        }
    }
    for e in a.exprs.iter() {
        match &e.kind {
            ExprKind::List { type_args, .. } | ExprKind::SetOrMap { type_args, .. } | ExprKind::TypeArguments { type_args, .. } => roots.extend(type_args.iter().copied()),
            ExprKind::Call { arguments, .. } | ExprKind::InstanceCreation { arguments, .. } => roots.extend(arguments.type_args.iter().copied()),
            _ => {}
        }
    }
    for p in a.patterns.iter() {
        match &p.kind {
            PatternKind::List { type_args, .. } | PatternKind::Map { type_args, .. } => roots.extend(type_args.iter().copied()),
            _ => {}
        }
    }
    for m in dartforge_frontend::parents::all_annotations(a, &unit_data.unit) {
        roots.extend(m.type_args.iter().copied());
    }
    for d in a.decls.iter() {
        if let DeclKind::Enum(x) = &d.kind {
            for k in x.constants.iter() {
                roots.extend(k.type_args.iter().copied());
            }
        }
    }
    for t in roots {
        mark_type(a, t, &mut in_arguments);
    }
    let mut externals: HashSet<ast::TypeId> = HashSet::new();
    for m in a.members.iter() {
        match &m.kind {
            MemberKind::Method(f) if a.function(*f).external => {
                if let Some(r) = a.function(*f).return_type {
                    mark_type(a, r, &mut externals);
                }
            }
            MemberKind::Field(l) if l.external => {
                if let Some(t) = l.ty {
                    mark_type(a, t, &mut externals);
                }
            }
            _ => {}
        }
    }
    for d in a.decls.iter() {
        if let DeclKind::Variables(l) = &d.kind
            && l.external
            && let Some(t) = l.ty
        {
            mark_type(a, t, &mut externals);
        }
    }
    for (k, t) in a.types.iter().enumerate() {
        let id = ast::TypeId(k as u32);
        if !matches!(t.kind, TypeKind::Named { .. }) {
            continue;
        }
        let Some(r) = cx.type_element(u, body, id) else { continue };
        let is_extension_type = matches!(r, Ref::Class(c) if program.class(c).kind == ClassKind::ExtensionType);
        if matches!(r, Ref::Typedef(_)) || is_extension_type || in_arguments.contains(&id) || externals.contains(&id) {
            out.push((t.span.start, Action::Element(r)));
        }
    }
    // `visitPatternField`: the getter of an object pattern field.
    for p in a.patterns.iter() {
        let PatternKind::Object { ty, fields } = &p.kind else { continue };
        let matched_ty = body.tipos_de_anotacoes.get(ty).copied().or_else(|| cx.s.outline.tipos_escritos.get(&(u, *ty)).copied());
        let class_id = match matched_ty.map(|t| cx.s.table.get(t)) {
            Some(Type::Interface { class, .. }) => Some(*class),
            Some(Type::ExtensionType { decl, .. }) => Some(*decl),
            _ => None,
        };
        for f in fields.iter() {
            let name = f.name.map(|n| n.sym).or_else(|| shorthand_name(a, f.pattern));
            let Some(name) = name else { continue };
            let getter = class_id.and_then(|c| {
                std::iter::once(c)
                    .chain(cx.s.outline.hierarchy.get(c).map(|h| h.supertypes.keys().copied().collect::<Vec<_>>()).unwrap_or_default())
                    .find_map(|x| program.class(x).instance_members.get(&name).copied())
            });
            if let Some(g) = getter {
                out.push((f.span.start, Action::Element(Ref::Function(g))));
            }
        }
    }
    // `visitAnnotation`: the annotation's element and its identifiers.
    for m in dartforge_frontend::parents::all_annotations(a, &unit_data.unit) {
        let pos = m.span.start;
        let mut actions: Vec<Ref> = Vec::new();
        let element = |b: Option<dartforge_elements::model::Binding>| b.and_then(|b| b.getter);
        match &m.name[..] {
            [n] => match element(program.lookup_in_unit(u, n.sym)) {
                Some(Element::Class(c)) => {
                    actions.push(Ref::Class(c));
                    if let Some(k) = cx.constructor(c, None) {
                        actions.push(Ref::Function(k));
                    }
                }
                Some(Element::Variable(v)) => actions.push(Ref::Variable(v)),
                Some(Element::Function(f)) => actions.push(Ref::Function(f)),
                _ => {}
            },
            [p, n] => match element(program.lookup_prefixed_in_unit(u, p.sym, n.sym)) {
                Some(Element::Class(c)) => {
                    actions.push(Ref::Class(c));
                    if let Some(k) = cx.constructor(c, None) {
                        actions.push(Ref::Function(k));
                    }
                }
                Some(Element::Variable(v)) => actions.push(Ref::Variable(v)),
                Some(Element::Function(f)) => actions.push(Ref::Function(f)),
                _ => {
                    // `Class.name`: the named constructor or the static member.
                    if let Some(Element::Class(c)) = element(program.lookup_in_unit(u, p.sym)) {
                        actions.push(Ref::Class(c));
                        if let Some(k) = cx.constructor(c, Some(n.sym)) {
                            actions.push(Ref::Function(k));
                        } else if let Some(f) = program.class(c).static_members.get(&n.sym) {
                            actions.push(Ref::Function(*f));
                        }
                    }
                }
            },
            [p, k, n] => {
                if let Some(Element::Class(c)) = element(program.lookup_prefixed_in_unit(u, p.sym, k.sym)) {
                    actions.push(Ref::Class(c));
                    if let Some(f) = cx.constructor(c, Some(n.sym)) {
                        actions.push(Ref::Function(f));
                    }
                }
            }
            _ => {}
        }
        for r in actions {
            out.push((pos, Action::Element(r)));
        }
    }
    // The supertype clauses, the implicit superclass constructors, the
    // `super(…)`/`this(…)`, the redirection targets and `toJson`.
    for (k, d) in a.decls.iter().enumerate() {
        let id = DeclId(k as u32);
        match &d.kind {
            DeclKind::Class(x) if !x.mixin_application => {
                for t in x.extends.iter().chain(x.with.iter()).chain(x.implements.iter()) {
                    if let Some(r) = cx.type_element(u, body, *t) {
                        out.push((a.ty(*t).span.start, Action::Direct(r)));
                    }
                }
                let Some(class_id) = cx.class_of_decl(u, id) else { continue };
                let outer = program.class(class_id).supertype_class;
                let has_constructors = x.members.iter().any(|m| matches!(a.member(*m).kind, MemberKind::Constructor(_)));
                if !has_constructors
                    && let Some(s) = outer
                    && let Some(k) = cx.constructor(s, None)
                {
                    out.push((x.name.span.start, Action::Element(Ref::Function(k))));
                }
                // `test_reflective_loader`'s `@reflectiveTest`.
                if d.metadata.iter().any(|m| dartforge_types::anotacoes::e_getter_de(program, cx.interner, u, m, "test_reflective_loader", "reflectiveTest"))
                    && let Some(k) = cx.constructor(class_id, None)
                {
                    out.push((x.name.span.start, Action::Element(Ref::Function(k))));
                }
                for &m in x.members.iter() {
                    let MemberKind::Constructor(kc) = &a.member(m).kind else { continue };
                    if !kc.initializers.iter().any(|i| matches!(i, Initializer::Super { .. }))
                        && let Some(s) = outer
                        && let Some(k) = cx.constructor(s, None)
                    {
                        out.push((a.member(m).span.start.max(kc.class_name.span.start), Action::Element(Ref::Function(k))));
                    }
                }
            }
            DeclKind::ExtensionType(x) => {
                for t in x.implements.iter() {
                    if let Some(r) = cx.type_element(u, body, *t) {
                        out.push((a.ty(*t).span.start, Action::Direct(r)));
                    }
                }
            }
            _ => {}
        }
        let members: &[MemberId] = match &d.kind {
            DeclKind::Class(x) => &x.members,
            DeclKind::Mixin(x) => &x.members,
            DeclKind::Enum(x) => &x.members,
            DeclKind::Extension(x) => &x.members,
            DeclKind::ExtensionType(x) => &x.members,
            _ => continue,
        };
        let class_id = cx.class_of_decl(u, id);
        for &m in members {
            match &a.member(m).kind {
                MemberKind::Constructor(kc) => {
                    let outer = class_id.and_then(|c| program.class(c).supertype_class);
                    for i in kc.initializers.iter() {
                        match i {
                            Initializer::Super { span, constructor, .. } => {
                                if let Some(f) = outer.and_then(|s| cx.constructor(s, constructor.map(|n| n.sym))) {
                                    out.push((span.start, Action::Element(Ref::Function(f))));
                                }
                            }
                            Initializer::Redirect { span, constructor, .. } => {
                                if let Some(f) = class_id.and_then(|c| cx.constructor(c, constructor.map(|n| n.sym))) {
                                    out.push((span.start, Action::Element(Ref::Function(f))));
                                }
                            }
                            _ => {}
                        }
                    }
                    // The target of `factory C() = D.name;`.
                    if let Some(r) = &kc.redirect
                        && let Some(Ref::Class(redirect_class)) = cx.type_element(u, body, r.ty)
                    {
                        out.push((r.span.start, Action::Element(Ref::Class(redirect_class))));
                        if let Some(f) = cx.constructor(redirect_class, r.constructor.map(|n| n.sym)) {
                            out.push((r.span.start, Action::Element(Ref::Function(f))));
                        }
                    }
                }
                MemberKind::Method(f) => {
                    let g = a.function(*f);
                    if let Some(n) = g.name
                        && !g.static_
                        && cx.interner.resolve(n.sym) == "toJson"
                    {
                        // The method's own element.
                        let fid = (0..program.functions.len()).map(|i| FunctionElementId(i as u32)).find(|x| matches!(program.function(*x).node, FunctionRef::Function { unit, function } if unit == u && function == *f));
                        if let Some(fid) = fid {
                            out.push((n.span.start, Action::Element(Ref::Function(fid))));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// The name of the `:x` shorthand field (the pattern variable, through `?`,
/// `!` and the cast).
fn shorthand_name(a: &Ast, mut p: ast::PatternId) -> Option<SymbolId> {
    loop {
        match &a.pattern(p).kind {
            PatternKind::Variable { name, .. } => return Some(name.sym),
            PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Cast { pattern: x, .. } => p = *x,
            _ => return None,
        }
    }
}

/// Runs the rules of this batch that are enabled (`enabled(name)`).
pub fn run(u: Unidade<'_>, interner: &Interner, enabled: &dyn Fn(&str) -> bool, sem: Option<&super::Semantics<'_>>) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let Some(s) = sem else { return out };
    if !enabled("unreachable_from_main") {
        return out;
    }
    let program = s.program;
    let library = program.unit(s.unit).library;
    let units: Vec<UnitId> = program.library(library).units.clone();
    let mut cx = Cx { program, interner, s, decls: HashSet::new(), top_regions: HashMap::new() };
    let mut all: Vec<Dec> = Vec::new();
    for &x in units.iter() {
        all.extend(unit_declarations(&cx, x));
    }
    cx.decls = all.iter().copied().collect();
    for &d in all.iter() {
        if let Dec::TopLevel(x, _) | Dec::TopLevelVar(x, ..) = d {
            let a = &program.unit(x).ast;
            for (i, f) in regions(a, d) {
                cx.top_regions.entry(x).or_default().push((i, f, d));
            }
        }
    }
    // The entry points.
    let entry_point_pragma = |x: UnitId, m: &Annotation| -> bool {
        let Some(n) = m.name.last() else { return false };
        if interner.resolve(n.sym) != "pragma" {
            return false;
        }
        let b = match &m.name[..] {
            [n] => program.lookup_in_unit(x, n.sym),
            [p, n] => program.lookup_prefixed_in_unit(x, p.sym, n.sym),
            _ => None,
        };
        let Some(Element::Class(c)) = b.and_then(|b| b.getter) else { return false };
        if program.library(program.class(c).library).uri != "dart:core" {
            return false;
        }
        let Some(args) = &m.arguments else { return false };
        let a = &program.unit(x).ast;
        args.args.iter().find(|y| y.name.is_none()).is_some_and(|y| matches!(&a.expr(y.value).kind, ExprKind::String(sl) if sl.constant_value().is_some_and(|v| v == *"vm:entry-point")))
    };
    let entries: Vec<Dec> = all
        .iter()
        .copied()
        .filter(|d| {
            let Dec::TopLevel(x, k) = *d else { return false };
            let a = &program.unit(x).ast;
            let decl = a.decl(k);
            let DeclKind::Function(f) = &decl.kind else { return false };
            a.function(*f).name.is_some_and(|n| interner.resolve(n.sym) == "main") || decl.metadata.iter().any(|m| entry_point_pragma(x, m))
        })
        .collect();
    if entries.is_empty() {
        return out;
    }
    // The dependencies of each declaration, by scanning the nested regions of
    // each unit.
    let mut dependencies: HashMap<Dec, HashSet<Dec>> = all.iter().map(|d| (*d, HashSet::new())).collect();
    for &x in units.iter() {
        let a = &program.unit(x).ast;
        let mut ranges: Vec<(usize, usize, Dec)> = Vec::new();
        for &d in all.iter().filter(|d| d.unit() == x) {
            for (i, f) in regions(a, d) {
                ranges.push((i, f, d));
            }
        }
        ranges.sort_by(|p, q| p.0.cmp(&q.0).then(q.1.cmp(&p.1)));
        let mut items = references(&cx, x);
        items.sort_by_key(|(p, _)| *p);
        let mut stack: Vec<(usize, usize, Dec)> = Vec::new();
        let mut next = 0;
        for (p, action) in items {
            while next < ranges.len() && ranges[next].0 <= p {
                let iv = ranges[next];
                while stack.last().is_some_and(|t| t.1 <= iv.0) {
                    stack.pop();
                }
                stack.push(iv);
                next += 1;
            }
            // The regions nest, but an inner range may have ended before an
            // outer one: filters by those that contain `p`.
            let mut reached: HashSet<Dec> = HashSet::new();
            cx.apply(action, &mut reached);
            if reached.is_empty() {
                continue;
            }
            for iv in stack.iter().filter(|iv| iv.0 <= p && p < iv.1) {
                if let Some(dep) = dependencies.get_mut(&iv.2) {
                    dep.extend(reached.iter().copied());
                }
            }
        }
    }
    let mut used: HashSet<Dec> = entries.iter().copied().collect();
    let mut queue: Vec<Dec> = entries;
    while let Some(d) = queue.pop() {
        if let Some(deps) = dependencies.get(&d) {
            for &x in deps {
                if used.insert(x) {
                    queue.push(x);
                }
            }
        }
    }
    // The unreached ones of the unit, public and without `@visibleForTesting`.
    let a = u.ast;
    let visible_for_testing = |metadata: &[Annotation]| metadata.iter().any(|m| dartforge_types::anotacoes::e_getter_de(program, interner, s.unit, m, "meta", "visibleForTesting"));
    let mut findings: Vec<(Span, String)> = Vec::new();
    for d in unit_declarations(&cx, s.unit) {
        if used.contains(&d) {
            continue;
        }
        match d {
            Dec::TopLevel(_, k) => {
                let decl = a.decl(k);
                if visible_for_testing(&decl.metadata) {
                    continue;
                }
                let name = match &decl.kind {
                    DeclKind::Class(x) => Some(x.name),
                    DeclKind::Mixin(x) => Some(x.name),
                    DeclKind::Enum(x) => Some(x.name),
                    DeclKind::ExtensionType(x) => Some(x.name),
                    DeclKind::Typedef(x) => Some(x.name),
                    DeclKind::Function(f) => a.function(*f).name,
                    DeclKind::Extension(x) => match x.name {
                        Some(n) => Some(n),
                        // An unnamed extension is private.
                        None => continue,
                    },
                    DeclKind::Variables(_) => None,
                };
                let Some(n) = name else { continue };
                if cx.is_private(n.sym) {
                    continue;
                }
                findings.push((n.span, interner.resolve(n.sym).to_string()));
            }
            Dec::TopLevelVar(_, k, i) => {
                let decl = a.decl(k);
                let DeclKind::Variables(l) = &decl.kind else { continue };
                let n = l.variables[i].name;
                if cx.is_private(n.sym) || visible_for_testing(&decl.metadata) {
                    continue;
                }
                findings.push((n.span, interner.resolve(n.sym).to_string()));
            }
            Dec::Member(_, m) => {
                let member = a.member(m);
                if visible_for_testing(&member.metadata) {
                    continue;
                }
                match &member.kind {
                    MemberKind::Constructor(k) => {
                        let class_id = interner.resolve(k.class_name.sym);
                        match k.name {
                            Some(n) => findings.push((n.span, format!("{class_id}.{}", interner.resolve(n.sym)))),
                            None => findings.push((k.class_name.span, format!("{class_id}.new"))),
                        }
                    }
                    MemberKind::Method(f) => {
                        let Some(n) = a.function(*f).name else { continue };
                        findings.push((n.span, interner.resolve(n.sym).to_string()));
                    }
                    _ => {}
                }
            }
            Dec::Field(_, m, i) => {
                let member = a.member(m);
                let MemberKind::Field(l) = &member.kind else { continue };
                if visible_for_testing(&member.metadata) {
                    continue;
                }
                let n = l.variables[i].name;
                findings.push((n.span, interner.resolve(n.sym).to_string()));
            }
        }
    }
    findings.sort_by_key(|(sp, _)| (sp.start, sp.end));
    for (sp, name) in findings {
        out.push(LintReport { code: &c::UNREACHABLE_FROM_MAIN, span: sp, args: vec![name] });
    }
    out
}
