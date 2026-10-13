//! The constant cycles of a library, as the analyzer's `computeConstants`
//! finds them (`compute.dart:13-96`): the targets of `ConstantFinder`
//! (`utilities.dart:107-210`) and the loose dependencies of
//! `ConstantExpressionsDependenciesFinder` (`:20-101`), walked in order by
//! the walker of [`super::graph`], with the dependencies of
//! `computeDependencies` (`evaluation.dart:223-326`) and of `ReferenceFinder`
//! (`utilities.dart:214-273`).
//!
//! The graph only **finds the components** (docs/ANALYZER-ESPECIFICACAO.md
//! §G, T8.5): the value of each constant is still computed on demand by the
//! [`Motor`]. What comes out of here is who is in a cycle:
//!
//! * each variable of a component gets `recursive_compile_time_constant`
//!   at its name ([`Estado::variables_in_cycle`], [`Estado::locals_in_cycle`]);
//! * each constructor of a component stops being cycle-free
//!   ([`Estado::constructors_in_cycle`]): every creation through it is worth
//!   an unknown of the type, and the verifier reports
//!   `recursive_constant_constructor` at the name of the header's class.
//!
//! The walker's state (the node indices) persists across the libraries of a
//! single verification, as elements persist in the analyzer's session.
//! Elements of libraries whose bodies were not inferred count as already
//! evaluated: they do not become nodes.

use super::avaliador::{constant_locals, Motor};
use super::graph::{Walker, Grafo};
use crate::resolved::{MemberRef, Resolved};
use crate::table::Type;
use dartforge_elements::model::{
    ClassId, ClassKind, Element, FunctionElementId, FunctionKind, FunctionRef, LibraryId, Program, UnitId, UnitRole,
    VariableId, VariableRef,
};
use dartforge_frontend::ast::{
    self, CollectionElement, DeclKind, ExprId, ExprKind, MemberKind, PatternId, PatternKind, StmtId, StmtKind,
};
use dartforge_frontend::features::LanguageVersion;
use dartforge_intern::SymbolId;
use std::collections::{HashMap, HashSet};

/// A constant evaluation target (`ConstantEvaluationTarget`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Alvo {
    /// Top-level variable, field or enum constant.
    Var(VariableId),
    Ctor(FunctionElementId),
    /// Optional parameter (the analyzer's `DefaultFormalParameter`, with or
    /// without a default value): the unit, the start of the parameter and the
    /// default. A required parameter is always evaluated and does not become a
    /// target.
    Param(UnitId, usize, Option<ExprId>),
    /// Local constant: the unit and the name's offset.
    Local(UnitId, usize),
    /// Annotation: the index in `Estado::anotacoes`.
    Annot(u32),
    /// The synthetic `const` field `values` of an enum, whose initializer is the
    /// list of the constants (`LibraryBuilder.buildEnumChildren`): it depends on
    /// all of them, and `e1(values)` closes a cycle.
    Values(ClassId),
}

/// The indices of the program's declarations, by tree node.
#[derive(Debug, Default)]
struct Indices {
    topo: HashMap<(UnitId, ast::DeclId, usize), VariableId>,
    campos: HashMap<(UnitId, ast::MemberId, usize), VariableId>,
    construtores: HashMap<(UnitId, ast::MemberId), FunctionElementId>,
    classes: HashMap<(UnitId, ast::DeclId), ClassId>,
}

impl Indices {
    fn novo(program: &Program) -> Self {
        let mut x = Indices::default();
        for (i, v) in program.variables.iter().enumerate() {
            match v.node {
                VariableRef::TopLevel { unit, decl, index } => {
                    x.topo.insert((unit, decl, index), VariableId(i as u32));
                }
                VariableRef::Field { unit, member, index } => {
                    x.campos.insert((unit, member, index), VariableId(i as u32));
                }
                _ => {}
            }
        }
        for (i, f) in program.functions.iter().enumerate() {
            if let FunctionRef::Constructor { unit, member } = f.node {
                x.construtores.insert((unit, member), FunctionElementId(i as u32));
            }
        }
        for (i, k) in program.classes.iter().enumerate() {
            if let Some(d) = k.decl {
                x.classes.insert((d.unit, d.decl), ClassId(i as u32));
            }
        }
        x
    }
}

/// What persists in the [`Motor`] across libraries.
#[derive(Debug, Default)]
pub struct Estado {
    walker: Walker,
    nos: Vec<Alvo>,
    by_target: HashMap<Alvo, usize>,
    /// `isConstantEvaluated` of each node.
    avaliados: Vec<bool>,
    /// The dependencies of each annotation, computed when it is found (nothing
    /// depends on an annotation: it only starts walks).
    anotacoes: Vec<Vec<Alvo>>,
    /// The local constants of each unit, by the name's offset.
    locais: HashMap<UnitId, HashMap<usize, (StmtId, usize)>>,
    indices: Option<Indices>,
    /// Constructors with `isCycleFree == false`.
    pub constructors_in_cycle: HashSet<FunctionElementId>,
    /// Variables whose result is the cycle's invalid value.
    pub variables_in_cycle: HashSet<VariableId>,
    /// Same, local constants (unit and name offset).
    pub locals_in_cycle: HashSet<(UnitId, usize)>,
}

impl Estado {
    /// The constructor declared in member `mid` of unit `u`.
    pub fn construtor_de(&self, u: UnitId, mid: ast::MemberId) -> Option<FunctionElementId> {
        self.indices.as_ref()?.construtores.get(&(u, mid)).copied()
    }

    /// The initializer of the local constant declared at `offset`.
    fn local_initializer(&mut self, m: &Motor<'_>, u: UnitId, offset: usize) -> Option<Option<ExprId>> {
        let a = m.ast(u);
        let map = self.locais.entry(u).or_insert_with(|| constant_locals(a));
        let &(stmt, i) = map.get(&offset)?;
        let list = match &a.stmt(stmt).kind {
            StmtKind::Variables(l) => l,
            StmtKind::For { init: Some(ast::ForInit::Variables(l)), .. } => l,
            _ => return None,
        };
        Some(list.variables.get(i).and_then(|x| x.initializer))
    }
}

/// Finds the constant cycles of `lib` and stores them in `m.grafo`. Runs
/// before the verifier, which reads the result.
pub fn compute(m: &mut Motor<'_>, lib: LibraryId) {
    if !m.the_inferred.contains(&lib) {
        return;
    }
    let mut st = std::mem::take(&mut m.grafo);
    if st.indices.is_none() {
        st.indices = Some(Indices::novo(m.program));
    }
    let mut walker = std::mem::take(&mut st.walker);
    {
        let engine: &Motor<'_> = m;
        let program = engine.program;
        // The targets, unit by unit: those of `ConstantFinder` and then the
        // dependencies of the loose constant expressions.
        let mut order: Vec<Alvo> = Vec::new();
        for &u in &program.library(lib).units {
            if program.unit(u).role == UnitRole::Patch {
                continue;
            }
            let mut p = Walk::novo(engine, &mut st, u, false);
            p.unidade();
            let Walk { alvos: targets, loose, .. } = p;
            order.extend(targets);
            order.extend(loose);
        }
        let mut g = G { m: engine, est: &mut st };
        for tgt in order {
            if let Some(no) = g.no(tgt) {
                walker.percorrer(&mut g, no);
            }
        }
    }
    st.walker = walker;
    m.grafo = st;
}

/// The written initializer of a top-level variable or a field.
fn inicializador(m: &Motor<'_>, v: VariableId) -> Option<(UnitId, ExprId)> {
    match m.program.variable(v).node {
        VariableRef::TopLevel { unit, decl, index } => match &m.ast(unit).decl(decl).kind {
            DeclKind::Variables(l) => l.variables.get(index).and_then(|x| x.initializer).map(|x| (unit, x)),
            _ => None,
        },
        VariableRef::Field { unit, member, index } => match &m.ast(unit).member(member).kind {
            MemberKind::Field(l) => l.variables.get(index).and_then(|x| x.initializer).map(|x| (unit, x)),
            _ => None,
        },
        _ => None,
    }
}

/// The variable is a `ConstVariableElement`: `const`, an enum constant, or an
/// instance `final` of an enum or of a class with some `const` constructor
/// (`constFieldsForFinalInstance`). The others are always evaluated.
fn constant_element(m: &Motor<'_>, v: VariableId) -> bool {
    let program = m.program;
    let var = program.variable(v);
    if var.const_ || matches!(var.node, VariableRef::EnumConstant { .. }) {
        return true;
    }
    if !var.final_ || var.static_ {
        return false;
    }
    let Some(k) = var.class else { return false };
    let cls = program.class(k);
    match cls.kind {
        ClassKind::Enum => true,
        ClassKind::Class => cls.constructors.values().any(|f| program.function(program.publico(*f)).const_),
        _ => false,
    }
}

/// The declared superclass of `k`: the synthetic classes `S&M` of a `with`
/// clause do not count (the analyzer's `superclass` is `S`).
pub(super) fn declared_superclass(program: &Program, k: ClassId) -> Option<ClassId> {
    let mut sup = program.class(k).supertype_class?;
    let mut steps = 0;
    while program.class(sup).kind == ClassKind::MixinApplication && program.class(sup).decl.is_none() && steps < 64 {
        sup = program.class(sup).supertype_class?;
        steps += 1;
    }
    Some(sup)
}

// -- The graph -----------------------------------------------------------------

struct G<'p, 'a> {
    m: &'p Motor<'a>,
    est: &'p mut Estado,
}

impl<'a> G<'_, 'a> {
    /// The node of `tgt`; `None` when the element is from an opaque library
    /// (counts as evaluated).
    fn no(&mut self, tgt: Alvo) -> Option<usize> {
        let program = self.m.program;
        match tgt {
            Alvo::Var(v) if !self.m.the_inferred.contains(&program.variable(v).library) => return None,
            Alvo::Ctor(f) if !self.m.the_inferred.contains(&program.function(f).library) => return None,
            _ => {}
        }
        if let Some(&i) = self.est.by_target.get(&tgt) {
            return Some(i);
        }
        let always = match tgt {
            Alvo::Var(v) => !constant_element(self.m, v),
            _ => false,
        };
        let i = self.est.nos.len();
        self.est.nos.push(tgt);
        self.est.by_target.insert(tgt, i);
        self.est.avaliados.push(always);
        Some(i)
    }

    /// The `ReferenceFinder` over what `f` visits, in unit `u`.
    fn referencias(&mut self, u: UnitId, f: impl FnOnce(&mut Walk<'_, 'a>)) -> Vec<Alvo> {
        let mut p = Walk::novo(self.m, &mut *self.est, u, true);
        f(&mut p);
        p.saida
    }

    fn of_variable(&mut self, v: VariableId) -> Vec<Alvo> {
        let m = self.m;
        let program = m.program;
        let var = program.variable(v);
        if let VariableRef::EnumConstant { unit, decl, index } = var.node {
            let DeclKind::Enum(en) = &m.ast(unit).decl(decl).kind else { return Vec::new() };
            let Some(cst) = en.constants.get(index) else { return Vec::new() };
            // An enum called `values`, or a constant with the enum's name: the
            // error was already reported elsewhere.
            if m.interner.resolve(en.name.sym) == "values" || cst.name.sym == en.name.sym {
                return Vec::new();
            }
            let mut deps = Vec::new();
            if let Some(k) = var.class {
                let the_key = cst.constructor.map(|n| n.sym).or_else(|| m.interner.lookup(""));
                if let Some(g) = the_key.and_then(|c| program.class(k).constructors.get(&c).copied()) {
                    if program.function(program.publico(g)).const_ {
                        deps.push(Alvo::Ctor(g));
                    }
                }
            }
            if let Some(args) = &cst.arguments {
                deps.extend(self.referencias(unit, |p| {
                    for x in args.args.iter() {
                        p.expr(x.value, true);
                    }
                }));
            }
            return deps;
        }
        match inicializador(m, v) {
            Some((unit, init)) => self.referencias(unit, |p| p.expr(init, var.const_)),
            None => Vec::new(),
        }
    }

    fn of_constructor(&mut self, f: FunctionElementId) -> Vec<Alvo> {
        let m = self.m;
        let program = m.program;
        let public = program.function(program.publico(f));
        if !public.const_ {
            return Vec::new();
        }
        let fe = program.function(f);
        let Some(cls) = fe.class else { return Vec::new() };
        let empty = m.interner.lookup("");
        let no = match fe.node {
            FunctionRef::Constructor { unit, member } => match &m.ast(unit).member(member).kind {
                MemberKind::Constructor(k) => Some((unit, k)),
                _ => None,
            },
            _ => None,
        };
        if public.factory || no.is_some_and(|(_, k)| k.factory) {
            // `getConstRedirectedConstructor`: only the `const` target of the
            // redirection; `dart:core`'s `Symbol` is an exception.
            let e_symbol = m.interner.resolve(program.class(cls).name) == "Symbol" && Some(fe.library) == m.core.core_library;
            if e_symbol {
                return Vec::new();
            }
            let tgt = no.and_then(|(unit, k)| redirection_target(m, unit, k));
            return match tgt {
                Some(g) if program.function(program.publico(g)).const_ => vec![Alvo::Ctor(g)],
                _ => Vec::new(),
            };
        }
        let mut deps = Vec::new();
        let mut implicit_super = true;
        if let Some((unit, k)) = no {
            for init in k.initializers.iter() {
                match init {
                    ast::Initializer::Field { value, .. } => {
                        let value = *value;
                        deps.extend(self.referencias(unit, |p| p.expr(value, false)));
                    }
                    ast::Initializer::Assert { condition, message, .. } => {
                        let (condition, message) = (*condition, *message);
                        deps.extend(self.referencias(unit, |p| {
                            p.expr(condition, false);
                            if let Some(x) = message {
                                p.expr(x, false);
                            }
                        }));
                    }
                    ast::Initializer::Super { constructor, arguments, .. } => {
                        implicit_super = false;
                        // The target, `const` or not.
                        let the_key = constructor.map(|n| n.sym).or(empty);
                        let sup = program.class(cls).supertype_class;
                        if let Some(g) = the_key.and_then(|c| sup.and_then(|s| program.class(s).constructors.get(&c).copied())) {
                            deps.push(Alvo::Ctor(g));
                        }
                        deps.extend(self.referencias(unit, |p| {
                            for x in arguments.args.iter() {
                                p.expr(x.value, false);
                            }
                        }));
                    }
                    ast::Initializer::Redirect { constructor, arguments, .. } => {
                        implicit_super = false;
                        let the_key = constructor.map(|n| n.sym).or(empty);
                        if let Some(g) = the_key.and_then(|c| program.class(cls).constructors.get(&c).copied()) {
                            deps.push(Alvo::Ctor(g));
                        }
                        deps.extend(self.referencias(unit, |p| {
                            for x in arguments.args.iter() {
                                p.expr(x.value, false);
                            }
                        }));
                    }
                }
            }
        }
        if implicit_super {
            // Without a written `super(...)`/`this(...)`: the superclass's unnamed
            // constructor, if it is `const` and the superclass is not `Object`.
            if let Some(sup) = declared_superclass(program, cls) {
                if Some(sup) != m.core.object_class {
                    if let Some(g) = empty.and_then(|c| program.class(sup).constructors.get(&c).copied()) {
                        if program.function(program.publico(g)).const_ {
                            deps.push(Alvo::Ctor(g));
                        }
                    }
                }
            }
        }
        // Every `final`/`const` instance field with an initializer.
        for &v in program.class(cls).fields.iter() {
            let var = program.variable(v);
            if (var.final_ || var.const_) && !var.static_ && inicializador(m, v).is_some() {
                deps.push(Alvo::Var(v));
            }
        }
        // All the parameters.
        if let Some((unit, k)) = no {
            for p in k.parameters.iter() {
                if p.kind != ast::ParameterKind::Required {
                    deps.push(Alvo::Param(unit, p.span.start, p.default_value));
                }
            }
        }
        deps
    }
}

/// The constructor that factory `k` redirects to (`= C.the_name`).
fn redirection_target(m: &Motor<'_>, unit: UnitId, k: &ast::Constructor) -> Option<FunctionElementId> {
    let program = m.program;
    let r = k.redirect.as_ref()?;
    let (tgt, ctor) = crate::redirection::class_and_constructor(program, unit, r)?;
    let the_key = match ctor {
        Some(n) => n.sym,
        None => m.interner.lookup("")?,
    };
    program.class(tgt).constructors.get(&the_key).copied()
}

impl Grafo for G<'_, '_> {
    fn dependencias(&mut self, no: usize) -> Vec<usize> {
        let tgt = self.est.nos[no];
        let deps: Vec<Alvo> = match tgt {
            Alvo::Var(v) => self.of_variable(v),
            Alvo::Ctor(f) => self.of_constructor(f),
            Alvo::Param(u, _, Some(d)) => self.referencias(u, |p| p.expr(d, false)),
            Alvo::Param(_, _, None) => Vec::new(),
            Alvo::Local(u, offset) => match self.est.local_initializer(self.m, u, offset) {
                Some(Some(init)) => self.referencias(u, |p| p.expr(init, true)),
                _ => Vec::new(),
            },
            Alvo::Annot(i) => self.est.anotacoes[i as usize].clone(),
            Alvo::Values(k) => self.m.program.class(k).enum_constants.iter().map(|&v| Alvo::Var(v)).collect(),
        };
        deps.into_iter().filter_map(|d| self.no(d)).collect()
    }

    fn evaluated(&self, no: usize) -> bool {
        self.est.avaliados[no]
    }

    fn avaliar(&mut self, no: usize) {
        let m = self.m;
        let tgt = self.est.nos[no];
        let ready = match tgt {
            // Without an initializer there is no result to store: the node stays
            // unevaluated, as in the analyzer.
            Alvo::Var(v) => {
                matches!(m.program.variable(v).node, VariableRef::EnumConstant { .. }) || inicializador(m, v).is_some()
            }
            // Only a `const` constructor becomes computed.
            Alvo::Ctor(f) => m.program.function(m.program.publico(f)).const_,
            Alvo::Param(..) | Alvo::Annot(_) | Alvo::Values(_) => true,
            Alvo::Local(u, offset) => matches!(self.est.local_initializer(m, u, offset), Some(Some(_))),
        };
        if ready {
            self.est.avaliados[no] = true;
        }
    }

    fn evaluate_component(&mut self, nodes: &[usize]) {
        for &no in nodes {
            let tgt = self.est.nos[no];
            match tgt {
                Alvo::Var(v) => {
                    self.est.variables_in_cycle.insert(v);
                    self.est.avaliados[no] = true;
                }
                // A constructor in a cycle never becomes evaluated.
                Alvo::Ctor(f) => {
                    self.est.constructors_in_cycle.insert(f);
                }
                Alvo::Local(u, offset) => {
                    self.est.locals_in_cycle.insert((u, offset));
                    self.est.avaliados[no] = true;
                }
                // A parameter is a variable for `generateCycleError`: it keeps the
                // stored invalid value.
                // The synthetic `values` has no name to report.
                Alvo::Param(..) | Alvo::Annot(_) | Alvo::Values(_) => self.est.avaliados[no] = true,
            }
        }
    }
}

// -- The tree walk ---------------------------------------------------------------

/// A pre-order walk over the tree of a unit, in two roles: with
/// `references`, it is `ReferenceFinder` (what it finds goes to `saida`);
/// without, it is `ConstantFinder` (`targets`) together with
/// `ConstantExpressionsDependenciesFinder` (`loose`).
struct Walk<'p, 'a> {
    m: &'p Motor<'a>,
    est: &'p mut Estado,
    u: UnitId,
    a: &'a ast::Ast,
    referencias: bool,
    saida: Vec<Alvo>,
    alvos: Vec<Alvo>,
    loose: Vec<Alvo>,
    /// The loose dependencies already seen (the original's set is ordered by
    /// insertion).
    the_seen: HashSet<Alvo>,
    /// The class (or mixin, enum, extension type) of the visited member.
    classe: Option<ClassId>,
    /// `treatFinalInstanceVarAsConst`: inside a class with some `const`
    /// constructor.
    finals_as_const: bool,
    patterns_enabled: bool,
}

impl<'p, 'a> Walk<'p, 'a> {
    fn novo(m: &'p Motor<'a>, st: &'p mut Estado, u: UnitId, references: bool) -> Self {
        let program = m.program;
        let lib = program.unit(u).library;
        let patterns_enabled = program.library(lib).features.versao() >= LanguageVersion::new(3, 0);
        Walk {
            m,
            est: st,
            u,
            a: m.ast(u),
            referencias: references,
            saida: Vec::new(),
            alvos: Vec::new(),
            loose: Vec::new(),
            the_seen: HashSet::new(),
            classe: None,
            finals_as_const: false,
            patterns_enabled,
        }
    }

    /// `_find`: the references of what `f` visits become loose dependencies.
    fn release(&mut self, f: impl FnOnce(&mut Self)) {
        let stored = std::mem::take(&mut self.saida);
        let before = std::mem::replace(&mut self.referencias, true);
        f(self);
        self.referencias = before;
        let found = std::mem::replace(&mut self.saida, stored);
        for x in found {
            if self.the_seen.insert(x) {
                self.loose.push(x);
            }
        }
    }

    // -- Declarations ------------------------------------------------------------

    fn unidade(&mut self) {
        let program = self.m.program;
        let cu = &program.unit(self.u).unit;
        for d in cu.directives.iter() {
            // The annotations of `part` and `part of` have no element.
            if matches!(d.kind, ast::DirectiveKind::Part { .. } | ast::DirectiveKind::PartOf { .. }) {
                continue;
            }
            self.anotacoes(&d.metadata);
        }
        for &d in cu.declarations.iter() {
            self.declaracao(d);
        }
    }

    fn declaracao(&mut self, d: ast::DeclId) {
        let a = self.a;
        let program = self.m.program;
        let decl = a.decl(d);
        self.anotacoes(&decl.metadata);
        let cls = self.est.indices.as_ref().and_then(|x| x.classes.get(&(self.u, d)).copied());
        match &decl.kind {
            DeclKind::Variables(l) => {
                for (i, var) in l.variables.iter().enumerate() {
                    let Some(init) = var.initializer else { continue };
                    self.expr(init, l.const_);
                    if l.const_ {
                        let v = self.est.indices.as_ref().and_then(|x| x.topo.get(&(self.u, d, i)).copied());
                        if let Some(v) = v {
                            self.alvos.push(Alvo::Var(v));
                        }
                    }
                }
            }
            DeclKind::Function(f) => self.funcao(*f),
            DeclKind::Class(k) => {
                self.classe = cls;
                self.finals_as_const =
                    k.members.iter().any(|mid| matches!(&a.member(*mid).kind, MemberKind::Constructor(c) if c.const_));
                self.membros(&k.members);
                self.finals_as_const = false;
                self.classe = None;
            }
            DeclKind::Mixin(k) => {
                self.classe = cls;
                self.membros(&k.members);
                self.classe = None;
            }
            DeclKind::Enum(k) => {
                self.classe = cls;
                for (i, cst) in k.constants.iter().enumerate() {
                    if let Some(args) = &cst.arguments {
                        for x in args.args.iter() {
                            self.expr(x.value, true);
                        }
                    }
                    if let Some(v) = cls.and_then(|c| program.class(c).enum_constants.get(i).copied()) {
                        self.alvos.push(Alvo::Var(v));
                    }
                }
                self.membros(&k.members);
                self.classe = None;
            }
            DeclKind::Extension(k) => self.membros(&k.members),
            DeclKind::ExtensionType(k) => {
                self.classe = cls;
                self.membros(&k.members);
                self.classe = None;
            }
            DeclKind::Typedef(_) => {}
        }
    }

    fn membros(&mut self, members: &'a [ast::MemberId]) {
        let a = self.a;
        for &mid in members {
            let mem = a.member(mid);
            self.anotacoes(&mem.metadata);
            match &mem.kind {
                MemberKind::Field(l) => {
                    for (i, var) in l.variables.iter().enumerate() {
                        let Some(init) = var.initializer else { continue };
                        self.expr(init, l.const_);
                        if l.const_ || self.finals_as_const && l.final_ && !l.static_ {
                            let v = self.est.indices.as_ref().and_then(|x| x.campos.get(&(self.u, mid, i)).copied());
                            if let Some(v) = v {
                                self.alvos.push(Alvo::Var(v));
                            }
                        }
                    }
                }
                MemberKind::Method(f) => self.funcao(*f),
                MemberKind::Constructor(k) => {
                    self.parametros(&k.parameters);
                    for init in k.initializers.iter() {
                        match init {
                            ast::Initializer::Field { value, .. } => self.expr(*value, false),
                            ast::Initializer::Assert { condition, message, .. } => {
                                self.expr(*condition, false);
                                if let Some(x) = message {
                                    self.expr(*x, false);
                                }
                            }
                            ast::Initializer::Super { arguments, .. } | ast::Initializer::Redirect { arguments, .. } => {
                                for x in arguments.args.iter() {
                                    self.expr(x.value, false);
                                }
                            }
                        }
                    }
                    self.corpo(&k.body);
                    // The `const` constructor and all its parameters, after the
                    // children.
                    if k.const_ {
                        let f = self.est.indices.as_ref().and_then(|x| x.construtores.get(&(self.u, mid)).copied());
                        if let Some(f) = f {
                            self.alvos.push(Alvo::Ctor(f));
                            for p in k.parameters.iter() {
                                if p.kind != ast::ParameterKind::Required {
                                    self.alvos.push(Alvo::Param(self.u, p.span.start, p.default_value));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn funcao(&mut self, f: ast::FunctionId) {
        let a = self.a;
        let func = a.function(f);
        if let Some(ps) = &func.parameters {
            self.parametros(ps);
        }
        self.corpo(&func.body);
    }

    fn parametros(&mut self, ps: &'a [ast::Parameter]) {
        for p in ps {
            self.anotacoes(&p.metadata);
            if let Some(fp) = &p.function_parameters {
                self.parametros(fp);
            }
            if let Some(d) = p.default_value {
                self.expr(d, false);
                if !self.referencias {
                    self.alvos.push(Alvo::Param(self.u, p.span.start, Some(d)));
                }
            }
        }
    }

    fn corpo(&mut self, b: &ast::FunctionBody) {
        match b {
            ast::FunctionBody::Block(s) => self.stmt(*s),
            ast::FunctionBody::Expression(e) => self.expr(*e, false),
            _ => {}
        }
    }

    /// The annotations of a declaration: each one is a target, with the element
    /// it resolves to (the getter's variable, or the constructor) and the
    /// references of the arguments as dependencies.
    fn anotacoes(&mut self, ms: &'a [ast::Annotation]) {
        if self.referencias {
            return;
        }
        for an in ms {
            let mut deps = Vec::new();
            if let Some(x) = self.annotation_element(an) {
                deps.push(x);
            }
            if let Some(args) = &an.arguments {
                let stored = std::mem::take(&mut self.saida);
                self.referencias = true;
                for x in args.args.iter() {
                    self.expr(x.value, true);
                }
                self.referencias = false;
                deps.extend(std::mem::replace(&mut self.saida, stored));
            }
            let i = self.est.anotacoes.len() as u32;
            self.est.anotacoes.push(deps);
            self.alvos.push(Alvo::Annot(i));
        }
    }

    fn annotation_element(&self, an: &ast::Annotation) -> Option<Alvo> {
        let program = self.m.program;
        let u = self.u;
        let empty = self.m.interner.lookup("");
        let static_ = |k: ClassId, the_name: SymbolId| -> Option<VariableId> {
            let cls = program.class(k);
            cls.enum_constants.iter().chain(cls.fields.iter()).copied().find(|v| {
                let x = program.variable(*v);
                x.name == the_name && x.static_
            })
        };
        let of_class = |k: ClassId, the_name: Option<SymbolId>| -> Option<Alvo> {
            if let Some(v) = the_name.and_then(|n| static_(k, n)) {
                return Some(Alvo::Var(v));
            }
            let the_key = the_name.or(empty)?;
            program.class(k).constructors.get(&the_key).map(|f| Alvo::Ctor(*f))
        };
        let of_element = |el: Option<Element>, the_name: Option<SymbolId>| -> Option<Alvo> {
            match el? {
                Element::Variable(v) if the_name.is_none() => Some(Alvo::Var(v)),
                Element::Function(f) if the_name.is_none() => program.function(f).variable.map(Alvo::Var),
                Element::Class(k) => of_class(k, the_name),
                _ => None,
            }
        };
        match &an.name[..] {
            [n] => {
                if let Some(v) = self.classe.and_then(|k| static_(k, n.sym)) {
                    return Some(Alvo::Var(v));
                }
                of_element(program.lookup_na_unidade(u, n.sym).and_then(|b| b.getter), None)
            }
            [x, y] => match program.lookup_prefixed_na_unidade(u, x.sym, y.sym) {
                Some(b) => of_element(b.getter, None),
                None => of_element(program.lookup_na_unidade(u, x.sym).and_then(|b| b.getter), Some(y.sym)),
            },
            [p, k, n] => of_element(program.lookup_prefixed_na_unidade(u, p.sym, k.sym).and_then(|b| b.getter), Some(n.sym)),
            _ => None,
        }
    }

    // -- Comandos ------------------------------------------------------------------

    fn lista_local(&mut self, l: &'a ast::VariableList) {
        for var in l.variables.iter() {
            let Some(init) = var.initializer else { continue };
            self.expr(init, l.const_);
            if l.const_ && !self.referencias {
                self.alvos.push(Alvo::Local(self.u, var.name.span.start));
            }
        }
    }

    fn inicio_de_for(&mut self, init: &'a Option<ast::ForInit>) {
        match init {
            Some(ast::ForInit::Variables(l)) => self.lista_local(l),
            Some(ast::ForInit::Expression(e)) => self.expr(*e, false),
            Some(ast::ForInit::Pattern { pattern, value, .. }) => {
                self.padrao(*pattern);
                self.expr(*value, false);
            }
            None => {}
        }
    }

    fn alvo_de_for_in(&mut self, tgt: &'a ast::ForInTarget) {
        match tgt {
            ast::ForInTarget::Declared { metadata, .. } => self.anotacoes(metadata),
            ast::ForInTarget::Pattern { pattern, .. } => self.padrao(*pattern),
            ast::ForInTarget::Expression(e) => self.expr(*e, false),
        }
    }

    fn stmt(&mut self, s: StmtId) {
        let a = self.a;
        match &a.stmt(s).kind {
            StmtKind::Block(xs) => {
                for x in xs.iter() {
                    self.stmt(*x);
                }
            }
            StmtKind::Variables(l) => self.lista_local(l),
            StmtKind::PatternVariables { pattern, value, .. } => {
                self.padrao(*pattern);
                self.expr(*value, false);
            }
            StmtKind::Function(f) => self.funcao(*f),
            StmtKind::Expression(e) => self.expr(*e, false),
            StmtKind::If { condition, case_pattern, guard, then, else_ } => {
                self.expr(*condition, false);
                if let Some(p) = case_pattern {
                    self.padrao(*p);
                }
                if let Some(g) = guard {
                    self.expr(*g, false);
                }
                self.stmt(*then);
                if let Some(e) = else_ {
                    self.stmt(*e);
                }
            }
            StmtKind::For { init, condition, updates, body, .. } => {
                self.inicio_de_for(init);
                if let Some(c) = condition {
                    self.expr(*c, false);
                }
                for x in updates.iter() {
                    self.expr(*x, false);
                }
                self.stmt(*body);
            }
            StmtKind::ForIn { target, iterable, body, .. } => {
                self.alvo_de_for_in(target);
                self.expr(*iterable, false);
                self.stmt(*body);
            }
            StmtKind::While { condition, body } => {
                self.expr(*condition, false);
                self.stmt(*body);
            }
            StmtKind::DoWhile { body, condition } => {
                self.stmt(*body);
                self.expr(*condition, false);
            }
            StmtKind::Switch { value, cases } => {
                self.expr(*value, false);
                for case in cases.iter() {
                    if let Some(p) = case.pattern {
                        self.padrao(p);
                    }
                    if let Some(g) = case.guard {
                        self.expr(g, false);
                    }
                    for x in case.body.iter() {
                        self.stmt(*x);
                    }
                }
            }
            StmtKind::Return(Some(e)) => self.expr(*e, false),
            StmtKind::Yield { value, .. } => self.expr(*value, false),
            StmtKind::Try { body, catches, finally_ } => {
                self.stmt(*body);
                for c in catches.iter() {
                    self.stmt(c.body);
                }
                if let Some(f) = finally_ {
                    self.stmt(*f);
                }
            }
            StmtKind::Labeled { body, .. } => self.stmt(*body),
            StmtKind::Assert { condition, message } => {
                self.expr(*condition, false);
                if let Some(x) = message {
                    self.expr(*x, false);
                }
            }
            _ => {}
        }
    }

    // -- Patterns --------------------------------------------------------------------

    fn padrao(&mut self, p: PatternId) {
        let a = self.a;
        match &a.pattern(p).kind {
            // `ConstantPattern.expression` and `SwitchCase.expression`.
            PatternKind::Constant(e) => {
                let e = *e;
                if self.referencias {
                    self.expr(e, true);
                } else {
                    self.release(|s| s.expr(e, true));
                }
            }
            // `case the_name:` (the parser stores it as a variable): the top-level
            // constant with that name, as the verifier reads it.
            PatternKind::Variable { final_: false, var_: false, ty: None, name } if self.patterns_enabled => {
                let program = self.m.program;
                if let Some(Element::Variable(v)) = program.lookup_na_unidade(self.u, name.sym).and_then(|b| b.getter) {
                    if program.variable(v).const_ {
                        if self.referencias {
                            self.saida.push(Alvo::Var(v));
                        } else if self.the_seen.insert(Alvo::Var(v)) {
                            self.loose.push(Alvo::Var(v));
                        }
                    }
                }
            }
            PatternKind::Relational { value, .. } => {
                let e = *value;
                if self.referencias {
                    self.expr(e, true);
                } else {
                    self.release(|s| s.expr(e, true));
                }
            }
            PatternKind::Or(x, y) | PatternKind::And(x, y) => {
                self.padrao(*x);
                self.padrao(*y);
            }
            PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Parenthesized(x) => self.padrao(*x),
            PatternKind::Cast { pattern, .. } => self.padrao(*pattern),
            PatternKind::List { elements, .. } => {
                for el in elements.iter() {
                    match el {
                        ast::ListPatternElement::Pattern(x) | ast::ListPatternElement::Rest(Some(x)) => self.padrao(*x),
                        _ => {}
                    }
                }
            }
            PatternKind::Map { entries, .. } => {
                for en in entries.iter() {
                    let the_key = en.key;
                    if self.referencias {
                        self.expr(the_key, true);
                    } else {
                        self.release(|s| s.expr(the_key, true));
                    }
                    self.padrao(en.value);
                }
            }
            PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
                for f in fields.iter() {
                    self.padrao(f.pattern);
                }
            }
            _ => {}
        }
    }

    // -- Expressions -----------------------------------------------------------------

    /// The reference that node `e` itself is: the `const` variable (direct or
    /// through the getter), top-level, static or local.
    fn referencia(&mut self, e: ExprId) {
        if !self.referencias {
            return;
        }
        let m = self.m;
        let program = m.program;
        let v = match m.resolvido(self.u, e) {
            Some(Resolved::Element(Element::Variable(v))) | Some(Resolved::Member { member: MemberRef::Variable(v), .. }) => {
                Some(*v)
            }
            Some(Resolved::Element(Element::Function(f))) | Some(Resolved::Member { member: MemberRef::Function(f), .. }) => {
                let fe = program.function(*f);
                if fe.kind == FunctionKind::ImplicitAccessor {
                    fe.variable
                } else {
                    // The enum's synthetic `values` getter.
                    if fe.node == FunctionRef::None
                        && fe.variable.is_none()
                        && fe.static_
                        && let Some(k) = fe.class
                        && program.class(k).kind == ClassKind::Enum
                        && m.interner.resolve(fe.name) == "values"
                    {
                        self.saida.push(Alvo::Values(k));
                    }
                    None
                }
            }
            // The local (or the local declared ahead, without resolution in
            // inference, but with the declaration recorded).
            Some(Resolved::Local(_)) | None => {
                let offset = m.body.units.get(self.u.0 as usize).and_then(|b| b.declaracao_local(e));
                if let Some(offset) = offset {
                    if self.est.local_initializer(m, self.u, offset).is_some() {
                        self.saida.push(Alvo::Local(self.u, offset));
                    }
                }
                None
            }
            _ => None,
        };
        if let Some(v) = v {
            if program.variable(v).const_ {
                self.saida.push(Alvo::Var(v));
            }
        }
    }

    /// An instance creation: `e_const` is the node's `isConst` (the written
    /// `const`, or the implicit creation in a constant context).
    fn criacao(&mut self, e: ExprId, e_const: bool, in_const: bool, args: &'a ast::Arguments) {
        if self.referencias {
            if e_const {
                if let Some(Resolved::Constructor(f)) = self.m.resolvido(self.u, e) {
                    let program = self.m.program;
                    if program.function(program.publico(*f)).const_ {
                        self.saida.push(Alvo::Ctor(*f));
                    }
                }
            }
            for x in args.args.iter() {
                self.expr(x.value, in_const || e_const);
            }
        } else if e_const {
            self.release(|s| s.expr(e, in_const));
        } else {
            for x in args.args.iter() {
                self.expr(x.value, in_const);
            }
        }
    }

    fn elemento(&mut self, el: &'a CollectionElement, c: bool) {
        match el {
            CollectionElement::Expression(x) | CollectionElement::NullAwareExpression(x) => self.expr(*x, c),
            CollectionElement::MapEntry { key, value, .. } => {
                self.expr(*key, c);
                self.expr(*value, c);
            }
            CollectionElement::Spread { value, .. } => self.expr(*value, c),
            CollectionElement::If { condition, case_pattern, guard, then, else_ } => {
                self.expr(*condition, c);
                if let Some(p) = case_pattern {
                    self.padrao(*p);
                }
                if let Some(g) = guard {
                    self.expr(*g, c);
                }
                self.elemento(then, c);
                if let Some(x) = else_ {
                    self.elemento(x, c);
                }
            }
            CollectionElement::For { init, condition, updates, body, .. } => {
                self.inicio_de_for(init);
                if let Some(x) = condition {
                    self.expr(*x, c);
                }
                for x in updates.iter() {
                    self.expr(*x, c);
                }
                self.elemento(body, c);
            }
            CollectionElement::ForIn { target, iterable, body, .. } => {
                self.alvo_de_for_in(target);
                self.expr(*iterable, c);
                self.elemento(body, c);
            }
        }
    }

    fn expr(&mut self, e: ExprId, in_const: bool) {
        let a = self.a;
        let u = self.u;
        match &a.expr(e).kind {
            ExprKind::Identifier(_) | ExprKind::DotShorthand { .. } => self.referencia(e),
            ExprKind::InstanceCreation { keyword, arguments, .. } => {
                let e_const = match keyword {
                    Some(ast::CreationKeyword::Const) => true,
                    Some(ast::CreationKeyword::New) => false,
                    None => in_const,
                };
                self.criacao(e, e_const, in_const, arguments);
            }
            ExprKind::Call { target, arguments } => {
                if matches!(self.m.resolvido(u, e), Some(Resolved::Constructor(_))) {
                    // Implicit creation: constant in a constant context
                    // (or with the dot shorthand's `const`).
                    let const_shorthand = matches!(a.expr(*target).kind, ExprKind::DotShorthand { const_: true, .. });
                    self.criacao(e, in_const || const_shorthand, in_const, arguments);
                    return;
                }
                self.expr(*target, in_const);
                for x in arguments.args.iter() {
                    self.expr(x.value, in_const);
                }
            }
            ExprKind::List { const_, elements, .. } => {
                let c = *const_ || in_const;
                if c && !self.referencias {
                    self.release(|s| s.expr(e, in_const));
                    return;
                }
                for el in elements.iter() {
                    self.elemento(el, c);
                }
            }
            ExprKind::SetOrMap { const_, elements, .. } => {
                let c = *const_ || in_const;
                if !self.referencias {
                    if c {
                        self.release(|s| s.expr(e, in_const));
                        return;
                    }
                    // Without `const`: the keys of a map and the elements of a set
                    // are evaluated for the uniqueness check.
                    let cls = match self.m.table.get(self.m.estatico(u, e)) {
                        Type::Interface { class, .. } => Some(*class),
                        _ => None,
                    };
                    if cls.is_some() && cls == self.m.core.map_class {
                        for el in elements.iter() {
                            if let CollectionElement::MapEntry { key, .. } = el {
                                let the_key = *key;
                                self.release(|s| s.expr(the_key, false));
                            }
                        }
                    } else if cls.is_some() && cls == self.m.core.set_class {
                        for el in elements.iter() {
                            self.release(|s| s.elemento(el, false));
                        }
                    }
                }
                for el in elements.iter() {
                    self.elemento(el, c);
                }
            }
            ExprKind::Record { const_, positional, named } => {
                let c = *const_ || in_const;
                if c && !self.referencias {
                    self.release(|s| s.expr(e, in_const));
                    return;
                }
                for x in positional.iter().chain(named.iter().map(|(_, x)| x)) {
                    self.expr(*x, c);
                }
            }
            ExprKind::FunctionExpression(f) => self.funcao(*f),
            ExprKind::String(lit) => {
                for p in lit.parts.iter() {
                    if let ast::StringPart::Interpolation(x) = p {
                        self.expr(*x, in_const);
                    }
                }
            }
            ExprKind::Parenthesized(x) | ExprKind::Await(x) | ExprKind::Throw(x) => self.expr(*x, in_const),
            ExprKind::Property { target, .. } => {
                self.expr(*target, in_const);
                self.referencia(e);
            }
            ExprKind::Index { target, index, .. } => {
                self.expr(*target, in_const);
                self.expr(*index, in_const);
            }
            ExprKind::TypeArguments { target, .. } => self.expr(*target, in_const),
            ExprKind::Unary { operand, .. } => self.expr(*operand, in_const),
            ExprKind::Binary { left, right, .. } => {
                self.expr(*left, in_const);
                self.expr(*right, in_const);
            }
            ExprKind::Conditional { condition, then, else_ } => {
                self.expr(*condition, in_const);
                self.expr(*then, in_const);
                self.expr(*else_, in_const);
            }
            ExprKind::Is { value, .. } | ExprKind::As { value, .. } => self.expr(*value, in_const),
            ExprKind::Assign { target, value, .. } => {
                self.expr(*target, in_const);
                self.expr(*value, in_const);
            }
            ExprKind::PatternAssign { pattern, value } => {
                self.padrao(*pattern);
                self.expr(*value, in_const);
            }
            ExprKind::Cascade { target, sections, .. } => {
                self.expr(*target, in_const);
                for s in sections.iter() {
                    self.expr(*s, in_const);
                }
            }
            ExprKind::Switch { value, cases } => {
                self.expr(*value, in_const);
                for case in cases.iter() {
                    self.padrao(case.pattern);
                    if let Some(g) = case.guard {
                        self.expr(g, false);
                    }
                    self.expr(case.body, in_const);
                }
            }
            _ => {}
        }
    }
}
