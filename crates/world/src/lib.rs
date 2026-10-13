//! Closed world over the element model: what of the user's program and of the
//! packages is reachable from the roots.
//!
//! It is Bacon's RTA in the shape of dart2js's `ResolutionWorldBuilder`
//! (`pkg/compiler/lib/src/universe/resolution_world_builder.dart:196-240`):
//! two sets that only grow — **instantiated** classes and invoked
//! **selectors** — and the rule "an instance member is live if its class is
//! instantiated **and** its name is a live selector". The impact of each
//! element is computed when it enters the queue
//! (`resolution/enqueuer.dart:283-302`), so dead code is never traversed.
//!
//! The crate knows no backend. What only the backend knows — what the runtime
//! calls by name, what interop exposes — comes in through the [`Roots`].
//!
//! Conservativeness decisions (the contract is in `docs/JS-PRODUCAO.md` §1.7):
//!
//! * selector by name **restricted by the receiver type**: `foo` called only
//!   on receivers of static type `T` keeps `foo` live in the classes reachable
//!   from `T` — `T` itself, the superclasses it inherits from (the member
//!   lives in the owner, not in the receiver), the subclasses that override it
//!   and the hierarchy that an instantiated subtype of `T` brings in (a mixin
//!   applied or an interface implemented only in the subclass). A dynamic or
//!   unknown receiver (`dynamic`, an inference gap) records the selector
//!   unrestricted, as before — nothing is ever pruned for lack of information;
//! * selector **by kind**: read/call (`foo`) and write (`foo_=`) are distinct
//!   selectors, like the keys of `instance_members`. Names that come from
//!   outside the program (the runtime calling by string) apply to both kinds;
//!   uses in the program record the exact kind;
//! * operators, `Object` members and `call` are live in every instantiated
//!   class, and so are the members that implement an SDK supertype (the
//!   precompiled runtime calls those names without the program seeing it);
//! * three class levels — dead, type only (identity and rti) and
//!   instantiated — which are the three reachabilities of
//!   `docs/PESQUISA-OTIMIZACAO.md` §5.

mod constants;
mod impact;

pub use constants::Constants;

use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionElementId, FunctionKind, FunctionRef, LibraryId, Program, VariableId, VariableRef};
use dartforge_frontend::ast;
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::resolve::OutlineTypes;
use dartforge_types::resolved::BodyTypes;
use dartforge_types::table::{Type, TypeTable};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

/// Supertype closure of every class, computed once: `sub ⊑ sup` becomes a
/// bit test (`docs/JS-PRODUCAO-TAMANHO.md` §2.1). The chain is the nominal
/// one — the class itself, superclass, applied mixins, interfaces and `on`,
/// transitively —, without type arguments (conservative for generics).
pub struct Hierarchy {
    words: usize,
    bits: Vec<u64>,
    lists: Vec<Vec<ClassId>>,
}

impl Hierarchy {
    pub fn new(p: &Program) -> Hierarchy {
        let n = p.classes.len();
        let words = n.div_ceil(64).max(1);
        let mut h = Hierarchy { words, bits: vec![0; n * words], lists: vec![Vec::new(); n] };
        // 0 = to do, 1 = in progress (cycle: the class keeps what it already
        // has), 2 = done.
        let mut state = vec![0u8; n];
        for root in 0..n {
            if state[root] != 0 {
                continue;
            }
            // Post-order DFS without recursion: each class merges its parents' bits.
            let mut stack: Vec<(usize, bool)> = vec![(root, false)];
            while let Some((c, parents_done)) = stack.pop() {
                if parents_done {
                    let base = c * words;
                    h.bits[base + c / 64] |= 1 << (c % 64);
                    let k = p.class(ClassId(c as u32));
                    let parents: Vec<usize> = k
                        .supertype_class
                        .iter()
                        .chain(k.mixin_classes.iter())
                        .chain(k.interface_classes.iter())
                        .chain(k.on_classes.iter())
                        .map(|x| x.0 as usize)
                        .collect();
                    for q in parents {
                        if q == c || q >= n {
                            continue;
                        }
                        for w in 0..words {
                            let v = h.bits[q * words + w];
                            h.bits[base + w] |= v;
                        }
                    }
                    state[c] = 2;
                    continue;
                }
                if state[c] != 0 {
                    continue;
                }
                state[c] = 1;
                stack.push((c, true));
                let k = p.class(ClassId(c as u32));
                for q in k.supertype_class.iter().chain(k.mixin_classes.iter()).chain(k.interface_classes.iter()).chain(k.on_classes.iter()) {
                    let q = q.0 as usize;
                    if q < n && state[q] == 0 {
                        stack.push((q, false));
                    }
                }
            }
        }
        for c in 0..n {
            let base = c * words;
            let mut l = Vec::new();
            for w in 0..words {
                let mut v = h.bits[base + w];
                while v != 0 {
                    let b = v.trailing_zeros() as usize;
                    l.push(ClassId((w * 64 + b) as u32));
                    v &= v - 1;
                }
            }
            h.lists[c] = l;
        }
        h
    }
    /// `sub ⊑ sup` in the nominal chain (the class itself included).
    pub fn is_subtype(&self, sub: ClassId, sup: ClassId) -> bool {
        let (a, b) = (sub.0 as usize, sup.0 as usize);
        if a * self.words >= self.bits.len() {
            return sub == sup;
        }
        (self.bits[a * self.words + b / 64] >> (b % 64)) & 1 == 1
    }
    /// The supertypes of `c`, itself included, in id order.
    pub fn supertypes(&self, c: ClassId) -> &[ClassId] {
        self.lists.get(c.0 as usize).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// The cone reach rule, with the hierarchy ready and the instantiated
/// subtypes of each class: the member of owner `member` serves a receiver of
/// static type `cone` if they are the same class, if one is an ancestor of the
/// other (inherited or overridden) or if some **instantiated** subtype of the
/// cone has the member in its chain (the subtype brings a mixin/interface
/// outside the cone's hierarchy). Walks the shorter of the two instantiated
/// lists.
fn reaches(h: &Hierarchy, inst_sub: &[Vec<ClassId>], member: ClassId, cone: ClassId) -> bool {
    if h.is_subtype(member, cone) || h.is_subtype(cone, member) {
        return true;
    }
    let of_cone = inst_sub.get(cone.0 as usize).map(Vec::as_slice).unwrap_or(&[]);
    let of_member = inst_sub.get(member.0 as usize).map(Vec::as_slice).unwrap_or(&[]);
    if of_cone.len() <= of_member.len() {
        of_cone.iter().any(|s| h.is_subtype(*s, member))
    } else {
        of_member.iter().any(|s| h.is_subtype(*s, cone))
    }
}

/// The analyzed program, borrowed from whoever compiled it.
#[derive(Clone, Copy)]
pub struct WorldInput<'a> {
    pub program: &'a Program,
    pub interner: &'a Interner,
    pub table: &'a TypeTable,
    pub outline: &'a OutlineTypes,
    pub bodies: &'a BodyTypes,
}

/// What the Dart program does not see, but which calls or creates its code.
#[derive(Default, Clone)]
pub struct Roots {
    /// `main` and whatever else is called from outside.
    pub functions: Vec<FunctionElementId>,
    pub variables: Vec<VariableId>,
    /// Instantiated by code we do not see.
    pub instantiated_classes: Vec<ClassId>,
    /// At least type identity (shell and rti rules).
    pub type_classes: Vec<ClassId>,
    /// Instantiated and with **all** members live (`@JSExport`).
    pub all_members_classes: Vec<ClassId>,
    /// Names called from outside the program (`dart.dsend(o, "toJson")` in
    /// the runtime): they apply to every instantiated class.
    pub selectors: Vec<String>,
    /// Constructors whose static tearoff is referenced from outside.
    pub tearoffs: Vec<FunctionElementId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClassLevel {
    Dead,
    /// Identity: shell, `addRtiResources`, rti rules, live statics.
    Type,
    /// Constructed: instance members eligible by selector.
    Instantiated,
}

/// Who pulled an element into the world (the first one that did).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    Root,
    Function(FunctionElementId),
    Variable(VariableId),
    Class(ClassId),
}

#[derive(Default, Debug, Clone)]
pub struct Statistics {
    pub user_classes: usize,
    pub instantiated_classes: usize,
    pub type_classes: usize,
    pub user_functions: usize,
    pub live_functions: usize,
    pub live_variables: usize,
    pub selectors: usize,
    /// Selectors with a receiver restriction (`foo` only in the cone of `T`).
    pub restricted_selectors: usize,
    pub processed_items: usize,
}

/// The result: what is live.
pub struct World {
    classes: Vec<ClassLevel>,
    functions: Vec<bool>,
    variables: Vec<bool>,
    tearoffs: Vec<bool>,
    selectors: HashSet<String>,
    /// Closed-world restriction by receiver type: `name` called only on
    /// receivers of known static type is live only in the listed cones. The
    /// list per name is sorted (determinism).
    sel_cone: HashMap<String, Vec<ClassId>>,
    cause_fn: Vec<Option<Cause>>,
    cause_class: Vec<Option<Cause>>,
    cause_var: Vec<Option<Cause>>,
    pub stats: Statistics,
    /// Computed with the SDK in the fixed point (`Options::include_sdk`).
    pub include_sdk: bool,
    hier: Arc<Hierarchy>,
    /// Instantiated subtypes of each class (the cone rule, [`reaches`]).
    inst_sub: Vec<Vec<ClassId>>,
    /// Words of the live `JS()` *templates* (JavaScript names).
    pub js_names: HashSet<String>,
}

impl World {
    pub fn class_level(&self, c: ClassId) -> ClassLevel {
        self.classes.get(c.0 as usize).copied().unwrap_or(ClassLevel::Dead)
    }
    pub fn is_live_function(&self, f: FunctionElementId) -> bool {
        self.functions.get(f.0 as usize).copied().unwrap_or(false)
    }
    pub fn is_live_variable(&self, v: VariableId) -> bool {
        self.variables.get(v.0 as usize).copied().unwrap_or(false)
    }
    pub fn selector(&self, name: &str) -> bool {
        self.selectors.contains(name)
    }
    /// Some use with a receiver of known type recorded `name` (even if
    /// restricted to another cone). The forwarder filter uses it so as not to
    /// emit too little; member pruning uses [`World::selector_live_for`].
    pub fn has_restriction(&self, name: &str) -> bool {
        self.sel_cone.contains_key(name)
    }
    /// `name` keeps a member of `class` live: an unrestricted selector applies
    /// to all classes; a restricted one, only to the classes reachable from
    /// the receiver's cone (the same rule as the fixed point, `reaches`).
    pub fn selector_live_for(&self, _program: &Program, name: &str, class: ClassId) -> bool {
        if self.selectors.contains(name) {
            return true;
        }
        let Some(cones) = self.sel_cone.get(name) else { return false };
        cones.iter().any(|c| reaches(&self.hier, &self.inst_sub, class, *c))
    }
    /// The live selectors, in alphabetical order (the set is a `HashSet`).
    pub fn selectors(&self) -> impl Iterator<Item = &str> {
        let mut v: Vec<&str> = self.selectors.iter().map(String::as_str).collect();
        v.sort_unstable();
        v.into_iter()
    }
    pub fn ctor_tearoff(&self, f: FunctionElementId) -> bool {
        self.tearoffs.get(f.0 as usize).copied().unwrap_or(false)
    }
    pub fn function_cause(&self, f: FunctionElementId) -> Option<Cause> {
        self.cause_fn.get(f.0 as usize).copied().flatten()
    }
    pub fn class_cause(&self, c: ClassId) -> Option<Cause> {
        self.cause_class.get(c.0 as usize).copied().flatten()
    }
    pub fn variable_cause(&self, v: VariableId) -> Option<Cause> {
        self.cause_var.get(v.0 as usize).copied().flatten()
    }
}

/// An element waiting to have its impact computed.
#[derive(Clone, Copy, Debug)]
enum Item {
    Function(FunctionElementId),
    Variable(VariableId),
    /// Initializers of the instance fields of an instantiated class.
    Fields(ClassId),
}

/// Options of the computation.
#[derive(Clone, Copy, Debug, Default)]
pub struct Options {
    /// The `dart:` libraries enter the fixed point like the program's (the
    /// production profile with the SDK compiled by our own pipeline,
    /// `docs/JS-PRODUCAO-SDK-PROPRIO.md` §4). Requires the SDK bodies to be
    /// inferred. Without it, the SDK is a boundary, as always.
    pub include_sdk: bool,
    /// `assert`s disabled (the production profile's default, as in
    /// `dart2js`): their bodies are not traversed, and the conditions that
    /// only they changed become constants (`constants.rs`).
    pub without_asserts: bool,
}

/// Computes the world. Deterministic: the queue is FIFO, the roots are
/// sorted, and the result (the sets) is the least fixed point, which does not
/// depend on order.
pub fn compute(e: WorldInput<'_>, r: &Roots) -> World {
    compute_with(e, r, Options::default())
}

/// Constant whose value the emitter folds at the use site (DDC's
/// `shouldInlineConstant`, `kernel/constants.dart:105-120`): a `const` with a
/// numeric, boolean or null literal initializer, or a *string* of fewer than
/// 32 units. It is the same criterion as
/// `emit_js::own_sdk::inlinable_literal`.
pub fn is_inlinable_constant(p: &Program, v: VariableId) -> bool {
    inlinable_constant_depth(p, v, 0)
}

/// [`is_inlinable_constant`], following aliases (`static const String
/// separatorString = _commaString;` in `dart:_recipe_syntax`): the alias
/// folds to the value of the constant it names, which DDC's constant
/// evaluator also does. The same rule is `dartforge_emit_js::own_sdk::inline_constant`.
fn inlinable_constant_depth(p: &Program, v: VariableId, depth: usize) -> bool {
    let var = p.variable(v);
    // An extension static is read through the key `L['Ext|x']`, without folding.
    if !var.const_ || var.extension.is_some() || depth > 8 {
        return false;
    }
    let (unit, init) = match var.node {
        VariableRef::TopLevel { unit, decl, index } => match &p.unit(unit).ast.decl(decl).kind {
            ast::DeclKind::Variables(l) => (unit, l.variables.get(index).and_then(|x| x.initializer)),
            _ => return false,
        },
        VariableRef::Field { unit, member, index } => match &p.unit(unit).ast.member(member).kind {
            ast::MemberKind::Field(l) => (unit, l.variables.get(index).and_then(|x| x.initializer)),
            _ => return false,
        },
        _ => return false,
    };
    let Some(x) = init else { return false };
    fn literal(a: &ast::Ast, x: ast::ExprId) -> bool {
        match &a.expr(x).kind {
            ast::ExprKind::Int(_) | ast::ExprKind::Double(_) | ast::ExprKind::Bool(_) | ast::ExprKind::Null => true,
            ast::ExprKind::String(lit) => lit.constant_value().is_some_and(|v| v.utf16_len() < 32),
            ast::ExprKind::Unary { op: ast::UnaryOp::Neg, operand } => matches!(a.expr(*operand).kind, ast::ExprKind::Int(_) | ast::ExprKind::Double(_)),
            ast::ExprKind::Parenthesized(i) => literal(a, *i),
            _ => false,
        }
    }
    let a = &p.unit(unit).ast;
    if literal(a, x) {
        return true;
    }
    match const_alias(p, v, a, x) {
        Some(target) => inlinable_constant_depth(p, target, depth + 1),
        None => false,
    }
}

/// The constant variable that the initializer `x` of `v` names with a plain
/// identifier: a static of `v`'s class or a declaration of its library.
pub fn const_alias(p: &Program, v: VariableId, a: &ast::Ast, x: ast::ExprId) -> Option<VariableId> {
    let ast::ExprKind::Identifier(n) = &a.expr(x).kind else { return None };
    let var = p.variable(v);
    let as_variable = |el: Element| match el {
        Element::Variable(t) => Some(t),
        Element::Function(f) => p.function(f).variable,
        _ => None,
    };
    if let Some(c) = var.class {
        if let Some(f) = p.class(c).static_members.get(&n.sym) {
            return p.function(*f).variable;
        }
    }
    let lib = p.library(var.library);
    lib.declared.get(&n.sym).or_else(|| lib.scope.get(&n.sym)).and_then(|b| b.getter).and_then(as_variable)
}

/// `@Native`/`@JsPeerInterface` classes of the SDK libraries (DDC's
/// `native_types.dart`).
fn native_classes(e: &WorldInput<'_>) -> HashSet<ClassId> {
    let p = e.program;
    let mut out = HashSet::new();
    for (i, c) in p.classes.iter().enumerate() {
        if !p.library(c.library).is_sdk {
            continue;
        }
        let Some(d) = c.decl else { continue };
        let decl = p.unit(d.unit).ast.decl(d.decl);
        if decl.metadata.iter().any(|a| a.name.last().is_some_and(|n| matches!(e.interner.resolve(n.sym), "Native" | "JsPeerInterface"))) {
            out.insert(ClassId(i as u32));
        }
    }
    out
}

/// Native SDK classes with the JS *tags* of `@Native('A,B')` /
/// `@JsPeerInterface(name: 'A')` — whoever compiles the SDK uses the ones for
/// the JS built-in tags (`String`, `Number`, `Array`…) as roots: a JS
/// primitive value becomes an instance of them without any `new`.
pub fn native_tags(e: &WorldInput<'_>) -> Vec<(ClassId, Vec<String>)> {
    let p = e.program;
    let mut out = Vec::new();
    for (i, c) in p.classes.iter().enumerate() {
        if !p.library(c.library).is_sdk {
            continue;
        }
        let Some(d) = c.decl else { continue };
        let unit = p.unit(d.unit);
        let decl = unit.ast.decl(d.decl);
        for a in decl.metadata.iter() {
            if !a.name.last().is_some_and(|n| matches!(e.interner.resolve(n.sym), "Native" | "JsPeerInterface")) {
                continue;
            }
            let Some(args) = &a.arguments else { continue };
            for arg in args.args.iter() {
                if arg.name.is_some_and(|n| e.interner.resolve(n.sym) != "name") {
                    continue;
                }
                if let ast::ExprKind::String(lit) = &unit.ast.expr(arg.value).kind {
                    if let Some(v) = lit.constant_value() {
                        let tags = v.to_string_lossy().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && !s.starts_with('!')).collect();
                        out.push((ClassId(i as u32), tags));
                    }
                }
            }
        }
    }
    out
}

/// For each supertype of a native class that is neither native nor from
/// `dart:core`/`dart:_interceptors`/`dart:collection`, the natives that
/// implement it (deterministic: sorted).
fn natives_by_interface(e: &WorldInput<'_>, natives: &HashSet<ClassId>) -> HashMap<ClassId, Vec<ClassId>> {
    let p = e.program;
    let mut out: HashMap<ClassId, Vec<ClassId>> = HashMap::new();
    let mut ns: Vec<ClassId> = natives.iter().copied().collect();
    ns.sort();
    for n in ns {
        let mut seen: HashSet<ClassId> = HashSet::new();
        let mut stack = vec![n];
        while let Some(k) = stack.pop() {
            if !seen.insert(k) {
                continue;
            }
            let c = p.class(k);
            if k != n && !natives.contains(&k) && !matches!(p.library(c.library).uri.as_str(), "dart:core" | "dart:_interceptors" | "dart:collection") {
                out.entry(k).or_default().push(n);
            }
            stack.extend(c.supertype_class);
            stack.extend(c.mixin_classes.iter().copied());
            stack.extend(c.interface_classes.iter().copied());
        }
    }
    out
}

/// `int` and `num` from `dart:core`.
fn int_num(e: &WorldInput<'_>) -> Option<(ClassId, ClassId)> {
    let core = e.program.core?;
    let class_named = |n: &str| match e.program.library(core).declared.get(&e.interner.lookup(n)?)?.getter? {
        Element::Class(c) => Some(c),
        _ => None,
    };
    Some((class_named("int")?, class_named("num")?))
}

/// Like [`compute`], with [`Options`].
pub fn compute_with(e: WorldInput<'_>, r: &Roots, op: Options) -> World {
    let mut c = Computation::new(e, op);
    c.seed(r);
    c.engine.finish()
}

/// The resumable fixed point: the rounds of the outer loop
/// (`emit_js_production`, world → emission → text verification) only add
/// roots, and the world is monotonic, so the engine stays alive between them
/// and each round processes only what the new roots reach
/// (`docs/JS-PRODUCAO-TAMANHO.md` §2.1).
pub struct Computation<'a> {
    engine: Engine<'a>,
}

impl<'a> Computation<'a> {
    pub fn new(e: WorldInput<'a>, op: Options) -> Computation<'a> {
        let mut m = Engine::new(e);
        if op.include_sdk {
            m.include_sdk = true;
            m.natives = native_classes(&e);
            m.int_num = int_num(&e);
            m.natives_by_interface = natives_by_interface(&e, &m.natives);
            m.constants = Some(Constants::new(e, op.without_asserts));
        }
        Computation { engine: m }
    }
    /// Adds roots and runs until the fixed point.
    pub fn seed(&mut self, r: &Roots) {
        seed(&mut self.engine, r);
    }
    /// The current world (a copy; the engine stays resumable).
    pub fn world(&self) -> World {
        self.engine.snapshot()
    }
}

fn seed(m: &mut Engine<'_>, r: &Roots) {
    m.cause = Cause::Root;
    for s in {
        let mut v = r.selectors.clone();
        v.sort();
        v
    } {
        m.new_external_selector(&s);
    }
    let mut all: Vec<ClassId> = r.all_members_classes.clone();
    all.sort();
    for c in all {
        m.all_members.insert(c);
        m.instantiate(c);
    }
    let mut cs = r.instantiated_classes.clone();
    cs.sort();
    for c in cs {
        m.instantiate(c);
    }
    let mut ts = r.type_classes.clone();
    ts.sort();
    for c in ts {
        m.mark_type(c);
    }
    let mut fs = r.functions.clone();
    fs.sort();
    for f in fs {
        m.use_function_directly(f);
    }
    let mut vs = r.variables.clone();
    vs.sort();
    for v in vs {
        m.use_variable(v);
    }
    let mut to = r.tearoffs.clone();
    to.sort();
    for f in to {
        m.use_constructor(f, true);
    }
    m.init_extensions();
    m.run();
}

/// An inconsistency found by the dry-run check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inconsistency {
    Function(FunctionElementId),
    Variable(VariableId),
    Class(ClassId),
    Selector(String),
    /// A receiver restriction (`name` in the cone of `class`) that the fixed
    /// point did not have.
    RestrictedSelector { name: String, class: ClassId },
}

/// `checkEnqueuerConsistency` (`pkg/compiler/lib/src/enqueue.dart:142-156`):
/// recomputes the impact of **all** live elements, without reusing the fixed
/// point's state, and reports every use that points to something the world
/// says is dead. A non-empty result is a defect of the fixed point.
pub fn check_consistency(e: WorldInput<'_>, r: &Roots, world: &World) -> Vec<Inconsistency> {
    // Recompute in a new engine, seeded with the finished world, and compare:
    // if the engine finds anything new, the world was not a fixed point.
    let mut m = Engine::new(e);
    if world.include_sdk {
        m.include_sdk = true;
        m.natives = native_classes(&e);
        m.int_num = int_num(&e);
        m.natives_by_interface = natives_by_interface(&e, &m.natives);
    }
    m.classes = world.classes.clone();
    m.fn_live = world.functions.clone();
    m.foreign_open |= world.functions.iter().enumerate().any(|(i, v)| *v && opens_foreign(e.program, FunctionElementId(i as u32)));
    m.var_live = world.variables.clone();
    m.tearoff = world.tearoffs.clone();
    m.sel = world.selectors.clone();
    m.inst_sub = world.inst_sub.clone();
    for (n, cs) in world.sel_cone.iter() {
        m.sel_cone.insert(n.clone(), cs.iter().copied().collect());
        for c in cs {
            if let Some(l) = m.names_by_cone.get_mut(c.0 as usize) {
                l.push(n.clone());
            }
        }
    }
    for c in &r.all_members_classes {
        m.all_members.insert(*c);
    }
    // Pending: members of instantiated classes whose name is not live yet.
    for (i, n) in world.classes.iter().enumerate() {
        if *n == ClassLevel::Instantiated {
            m.process_chain_members(ClassId(i as u32), true);
        }
    }
    m.init_extensions();
    // Redo the impact of everything that is live.
    for (i, v) in world.functions.iter().enumerate() {
        if *v {
            m.queue.push_back(Item::Function(FunctionElementId(i as u32)));
        }
    }
    for (i, v) in world.variables.iter().enumerate() {
        if *v {
            m.queue.push_back(Item::Variable(VariableId(i as u32)));
        }
    }
    for (i, n) in world.classes.iter().enumerate() {
        if *n == ClassLevel::Instantiated {
            m.queue.push_back(Item::Fields(ClassId(i as u32)));
        }
    }
    let base = m.queue.len();
    m.run_items(base);
    let mut out = Vec::new();
    for (i, v) in m.fn_live.iter().enumerate() {
        if *v && !world.functions[i] {
            out.push(Inconsistency::Function(FunctionElementId(i as u32)));
        }
    }
    for (i, v) in m.var_live.iter().enumerate() {
        if *v && !world.variables[i] {
            out.push(Inconsistency::Variable(VariableId(i as u32)));
        }
    }
    for (i, n) in m.classes.iter().enumerate() {
        if *n > world.classes[i] {
            out.push(Inconsistency::Class(ClassId(i as u32)));
        }
    }
    let mut new_selectors: Vec<&String> = m.sel.iter().filter(|s| !world.selectors.contains(*s)).collect();
    new_selectors.sort();
    out.extend(new_selectors.into_iter().map(|s| Inconsistency::Selector(s.clone())));
    let mut new_cones: Vec<(&String, ClassId)> = Vec::new();
    for (n, cs) in m.sel_cone.iter() {
        for c in cs.iter() {
            if !world.sel_cone.get(n).is_some_and(|v| v.contains(c)) {
                new_cones.push((n, *c));
            }
        }
    }
    new_cones.sort();
    out.extend(new_cones.into_iter().map(|(n, c)| Inconsistency::RestrictedSelector { name: n.clone(), class: c }));
    out
}

/// Owner of the pending members that have no class (extension members).
const NO_OWNER: ClassId = ClassId(u32::MAX);

pub(crate) struct Engine<'a> {
    pub(crate) e: WorldInput<'a>,
    classes: Vec<ClassLevel>,
    fn_live: Vec<bool>,
    var_live: Vec<bool>,
    tearoff: Vec<bool>,
    sel: HashSet<String>,
    /// Selectors with a receiver of known type: name → classes of the
    /// receivers' static types (`foo` called on `T` is live in the cone of `T`).
    sel_cone: HashMap<String, HashSet<ClassId>>,
    /// Supertype closure (`is_subtype` in O(1)).
    hier: Arc<Hierarchy>,
    /// Instantiated subtypes of each class (the third case of [`reaches`]).
    inst_sub: Vec<Vec<ClassId>>,
    /// Words of the live `JS()` *templates*: JavaScript names that property
    /// renaming must not touch.
    js_names: HashSet<String>,
    /// Constant conditions (production profile with the SDK in the world).
    pub(crate) constants: Option<Constants<'a>>,
    /// Names recorded with a receiver of each cone class (the "new class"
    /// side of the matching wakes up the pending members of these names).
    names_by_cone: Vec<Vec<String>>,
    /// Members of instantiated classes whose name is not yet a live selector
    /// for them (`_invokableInstanceMembersByName`,
    /// `resolution_world_builder.dart:232`), by name and by owner class
    /// (`NO_OWNER` for extension members).
    pending: HashMap<String, HashMap<ClassId, Vec<FunctionElementId>>>,
    all_members: HashSet<ClassId>,
    queue: VecDeque<Item>,
    /// Types (from the `TypeTable`) already scanned for classes.
    seen_types: HashSet<dartforge_types::table::TypeId>,
    /// Names the runtime calls on any object: `Object` members and `call`.
    universal_names: HashSet<String>,
    /// Instantiated classes whose SDK protocol rule has already been applied.
    protocol_done: HashSet<ClassId>,
    cause: Cause,
    cause_fn: Vec<Option<Cause>>,
    cause_class: Vec<Option<Cause>>,
    cause_var: Vec<Option<Cause>>,
    items: usize,
    extensions_ready: bool,
    /// The target of a simple `Assign` being traversed: its outermost member
    /// is a write (`foo_=`), not a read. Consumed (and cleared) by the
    /// `Property`/`Identifier` arm; the rest of the target is still a read.
    pub(crate) write_target: Option<dartforge_frontend::ast::ExprId>,
    /// The `C.name` being visited is the target of a call (`C.name(..)`): it
    /// creates through the constructor without the static *tearoff*, which
    /// only the reference without a call cites.
    pub(crate) call_target: Option<dartforge_frontend::ast::ExprId>,
    /// `JS_CLASS_REF(C)` calls that a `JS()` *template* only compares
    /// (`# instanceof #`, `# === #`): the class is live as a type, not
    /// instantiated.
    pub(crate) compared_class_refs: HashSet<(dartforge_elements::model::UnitId, dartforge_frontend::ast::ExprId)>,
    /// The SDK enters the fixed point like the program (`Options::include_sdk`).
    include_sdk: bool,
    /// Native classes (`@Native`/`@JsPeerInterface`): live as a type means
    /// instantiated, because the object is born in the browser.
    natives: HashSet<ClassId>,
    /// `(int, num)` from `dart:core`, with the SDK in the world.
    int_num: Option<(ClassId, ClassId)>,
    /// Interface (outside `core`/`_interceptors`/`collection`) → native
    /// classes that implement it: a live `ByteBuffer` may be a JS `ArrayBuffer`.
    natives_by_interface: HashMap<ClassId, Vec<ClassId>>,
    /// Whether a foreign object (a typed array, a DOM node) can reach the
    /// program: some function of a [`FOREIGN_LIBRARIES`] library or a
    /// non-SDK `external` function is live. Until then, the natives outside
    /// the JS built-ins are not instantiated by being live as a type
    /// ([`Engine::held_natives`]).
    foreign_open: bool,
    /// Natives live as a type while [`Engine::foreign_open`] is false: they
    /// are instantiated when it opens.
    held_natives: Vec<ClassId>,
}

/// Libraries whose code creates or receives objects of the native classes
/// outside the JS built-ins (`dart:_interceptors`). Without a live function
/// of one of them (or a non-SDK `external`), no `Uint8Array` or DOM node
/// exists in the program, whatever types it mentions: the typed data views
/// are only born in `dart:_native_typed_data` (`JS('NativeUint8List',
/// 'new Uint8Array(#)')`), reached through the factories of
/// `dart:typed_data`. It is the `NativeBehavior.typesInstantiated` of
/// dart2js (`native/behavior.dart`), coarsened to the library.
pub const FOREIGN_LIBRARIES: &[&str] = &[
    "dart:typed_data",
    "dart:_native_typed_data",
    "dart:html",
    "dart:html_common",
    "dart:svg",
    "dart:web_audio",
    "dart:web_gl",
    "dart:indexed_db",
    "dart:web_sql",
    "dart:js",
    "dart:js_util",
    "dart:js_interop",
    "dart:js_interop_unsafe",
    "dart:_js_annotations",
];

/// Whether the native class `c` is a JS built-in (its library is
/// `dart:_interceptors`, `dart:_js_helper` or `dart:core`): those become
/// instances without any code of the program (`"olá"`, `[]`), and the gate of
/// [`Engine::foreign_open`] does not apply to them.
fn is_builtin_native(p: &dartforge_elements::model::Program, c: ClassId) -> bool {
    matches!(p.library(p.class(c).library).uri.as_str(), "dart:_interceptors" | "dart:_js_helper" | "dart:core")
}

/// Whether the live function `f` opens the gate of [`Engine::foreign_open`].
fn opens_foreign(p: &dartforge_elements::model::Program, f: FunctionElementId) -> bool {
    let func = p.function(f);
    let lib = p.library(func.library);
    if lib.is_sdk {
        FOREIGN_LIBRARIES.contains(&lib.uri.as_str())
    } else {
        func.external
    }
}

impl<'a> Engine<'a> {
    fn new(e: WorldInput<'a>) -> Engine<'a> {
        let p = e.program;
        let mut universal: HashSet<String> = HashSet::new();
        universal.insert("call".into());
        // The `Object` members come from the element model, not from a list.
        if let Some(core) = p.core {
            if let Some(sym) = e.interner.lookup("Object") {
                if let Some(Element::Class(obj)) = p.library(core).declared.get(&sym).and_then(|b| b.getter) {
                    for f in p.class(obj).instance_members.values() {
                        universal.insert(e.interner.resolve(p.function(*f).name).to_string());
                    }
                }
            }
        }
        Engine {
            e,
            classes: vec![ClassLevel::Dead; p.classes.len()],
            fn_live: vec![false; p.functions.len()],
            var_live: vec![false; p.variables.len()],
            tearoff: vec![false; p.functions.len()],
            sel: HashSet::new(),
            sel_cone: HashMap::new(),
            hier: Arc::new(Hierarchy::new(p)),
            inst_sub: vec![Vec::new(); p.classes.len()],
            names_by_cone: vec![Vec::new(); p.classes.len()],
            js_names: HashSet::new(),
            constants: None,
            pending: HashMap::new(),
            all_members: HashSet::new(),
            queue: VecDeque::new(),
            seen_types: HashSet::new(),
            universal_names: universal,
            protocol_done: HashSet::new(),
            cause: Cause::Root,
            cause_fn: vec![None; p.functions.len()],
            cause_class: vec![None; p.classes.len()],
            cause_var: vec![None; p.variables.len()],
            items: 0,
            extensions_ready: false,
            write_target: None,
            call_target: None,
            compared_class_refs: HashSet::new(),
            include_sdk: false,
            natives: HashSet::new(),
            int_num: None,
            natives_by_interface: HashMap::new(),
            foreign_open: std::env::var("DARTFORGE_JSPROD_NATIVE_GATE").is_ok_and(|v| v == "0"),
            held_natives: Vec::new(),
        }
    }

    fn is_user_lib(&self, l: LibraryId) -> bool {
        self.include_sdk || !self.e.program.library(l).is_sdk
    }
    fn is_user_class(&self, c: ClassId) -> bool {
        self.is_user_lib(self.e.program.class(c).library)
    }

    fn name(&self, s: SymbolId) -> &'a str {
        self.e.interner.resolve(s)
    }

    // ------------------------------------------------------------ marking

    fn mark_fn_live(&mut self, f: FunctionElementId) {
        let i = f.0 as usize;
        if i >= self.fn_live.len() || self.fn_live[i] {
            return;
        }
        let func = self.e.program.function(f);
        if !self.is_user_lib(func.library) {
            return;
        }
        self.fn_live[i] = true;
        self.cause_fn[i] = Some(self.cause);
        self.queue.push_back(Item::Function(f));
        if !self.foreign_open && opens_foreign(self.e.program, f) {
            self.open_foreign(f);
        }
    }

    /// Opens the gate of [`Engine::foreign_open`]: the held natives are
    /// instantiated, with the function that opened it as the cause.
    fn open_foreign(&mut self, f: FunctionElementId) {
        self.foreign_open = true;
        let saved = self.cause;
        self.cause = Cause::Function(f);
        for c in std::mem::take(&mut self.held_natives) {
            self.instantiate(c);
        }
        self.cause = saved;
    }

    pub(crate) fn use_variable(&mut self, v: VariableId) {
        self.use_variable_with_receiver(v, None);
    }

    pub(crate) fn use_variable_with_receiver(&mut self, v: VariableId, receiver: Option<ClassId>) {
        let i = v.0 as usize;
        if i >= self.var_live.len() || self.var_live[i] {
            return;
        }
        let var = self.e.program.variable(v);
        if !self.is_user_lib(var.library) {
            return;
        }
        // With the SDK in the world (the production profile of our own SDK),
        // the primitive constant is folded at the use site by the emitter:
        // the variable is not read at run time and does not need to exist.
        if self.include_sdk && is_inlinable_constant(self.e.program, v) {
            return;
        }
        // An instance field is storage: it lives with the instantiated class,
        // and access goes through a selector. Only top-level and static
        // variables get here. `Resolved` does not say whether the use is a
        // read or a write, so the read kind applies (the exact write comes
        // from the `Assign` target); the worst case is keeping the getter,
        // never pruning the setter without a record.
        if var.class.is_some() && !var.static_ && var.extension.is_none() {
            let n = self.name(var.name).to_string();
            self.new_selector_with_receiver(&n, receiver);
            return;
        }
        self.var_live[i] = true;
        self.cause_var[i] = Some(self.cause);
        if let Some(g) = var.getter {
            self.mark_fn_live(g);
        }
        if let Some(s) = var.setter {
            self.mark_fn_live(s);
        }
        if let Some(c) = var.class {
            self.mark_type(c);
        }
        self.queue.push_back(Item::Variable(v));
    }

    pub(crate) fn new_selector(&mut self, name: &str) {
        // Read and call (`x.foo`, `x.foo()`) apply to the getter and the
        // method; the write (`x.foo = v`) records `foo_=` separately (the
        // setter's key in `instance_members`). Without a suffix there is no
        // serving across kinds: a read `foo` never keeps a `set foo` live.
        if self.sel.contains(name) {
            return;
        }
        self.sel.insert(name.to_string());
        if let Some(by_owner) = self.pending.remove(name) {
            let mut fs: Vec<FunctionElementId> = by_owner.into_values().flatten().collect();
            fs.sort();
            for f in fs {
                self.mark_fn_live(f);
            }
        }
    }

    /// A name called from outside the program (`dart.dsend`/`dput` in the
    /// runtime): it may be a read or a write, so it applies to both kinds (the
    /// write one is the `name_=` key of `instance_members`).
    pub(crate) fn new_external_selector(&mut self, name: &str) {
        self.new_selector(name);
        if !name.ends_with('=') {
            self.new_selector(&format!("{name}_="));
        }
    }

    /// A member use with the receiver's static type known: `None` is the old
    /// behavior (unrestricted selector, applies to every instantiated class);
    /// `Some(t)` restricts to the cone of `t` — classes outside `t`'s
    /// hierarchy do not keep the member live. Monotonic like the rest of the
    /// fixed point: each new call only turns on more members, never off.
    pub(crate) fn new_selector_with_receiver(&mut self, name: &str, receiver: Option<ClassId>) {
        let Some(t) = receiver else {
            self.new_selector(name);
            return;
        };
        // With the SDK in the world: an `int` at run time is a JS number,
        // whose class is `JSNumber` — which implements `double`, not `int`
        // (DDC's `JSInt` is never instantiated). The cone of `int` is `num`'s.
        let t = match self.int_num {
            Some((int, num)) if t == int => num,
            _ => t,
        };
        // Extension type (including the implicit `this` of one of its
        // members): at run time the value is the representation.
        let t = if self.include_sdk && self.e.program.class(t).kind == ClassKind::ExtensionType {
            match self.e.program.class(t).representation.and_then(|v| self.e.outline.variables.get(v.0 as usize)).and_then(|vt| vt.declared_type.or(vt.inferred)).and_then(|ty| self.class_of_type(ty)) {
                Some(r) if r != t => match self.int_num {
                    Some((int, num)) if r == int => num,
                    _ => r,
                },
                _ => t,
            }
        } else {
            t
        };
        if self.sel.contains(name) {
            return;
        }
        // An already recorded (name, cone) pair wakes up nothing new (dart2js's
        // `addReceiverConstraint`, `resolution_world_builder.dart:541-560`).
        match self.sel_cone.get_mut(name) {
            Some(cs) => {
                if !cs.insert(t) {
                    return;
                }
            }
            None => {
                self.sel_cone.insert(name.to_string(), HashSet::from([t]));
            }
        }
        if let Some(l) = self.names_by_cone.get_mut(t.0 as usize) {
            l.push(name.to_string());
        }
        // Wake up only the pending members of the same key inside the cone of
        // `t`; the rest stays pending (it is not pruning, it is waiting).
        let Some(by_owner) = self.pending.get_mut(name) else { return };
        let mut to_wake: Vec<ClassId> = Vec::new();
        for &k in by_owner.keys() {
            // An extension member has no owner class: it lives by name, as
            // before (the `on` applicability is the emitter's business).
            if k == NO_OWNER || reaches(&self.hier, &self.inst_sub, k, t) {
                to_wake.push(k);
            }
        }
        let mut fs: Vec<FunctionElementId> = Vec::new();
        for k in to_wake {
            fs.extend(by_owner.remove(&k).unwrap_or_default());
        }
        if by_owner.is_empty() {
            self.pending.remove(name);
        }
        fs.sort();
        for f in fs {
            self.mark_fn_live(f);
        }
    }

    /// The selector `name` (unrestricted or restricted) keeps a member of the
    /// instantiated `class` live (the rule is `reaches`).
    pub(crate) fn selector_for(&mut self, name: &str, class: ClassId) -> bool {
        if self.sel.contains(name) {
            return true;
        }
        match self.sel_cone.get(name) {
            Some(cs) => cs.iter().any(|c| reaches(&self.hier, &self.inst_sub, class, *c)),
            None => false,
        }
    }

    /// The selector `name` is live in some way (unrestricted or restricted):
    /// what extension members use, because they have no owner class for the
    /// cone.
    pub(crate) fn selector_any(&self, name: &str) -> bool {
        self.sel.contains(name) || self.sel_cone.contains_key(name)
    }

    /// The class of static type `t`, when it exists and is nominal: `C`,
    /// `C?`, an extension type, a type variable (through its bound) and a
    /// promoted variable `X & B` (through `B`). `dynamic`, an inference gap, a
    /// function, a record, `FutureOr`, a type variable without a class in its
    /// bound and `Null`/`Never`/`void` give `None` — unknown receiver,
    /// unrestricted selector.
    pub(crate) fn class_of_type(&self, t: dartforge_types::table::TypeId) -> Option<ClassId> {
        use dartforge_types::table::Type;
        let mut cur = t;
        for _ in 0..8 {
            if cur.0 as usize >= self.e.table.len() {
                return None;
            }
            match self.e.table.get(cur) {
                Type::Dynamic | Type::Void | Type::Never | Type::Null => return None,
                Type::Interface { class, .. } => return Some(*class),
                // With the SDK in the world, an extension type is, at run
                // time, the value of its representation: the cone is the
                // representation's (`$this[$toUpperCase]` on an extension type
                // over `String` calls `JSString`).
                Type::ExtensionType { decl: class, .. } if self.include_sdk => {
                    let rep = self.e.program.class(*class).representation;
                    match rep.and_then(|v| self.e.outline.variables.get(v.0 as usize)).and_then(|vt| vt.declared_type.or(vt.inferred)) {
                        Some(t) if t != cur => cur = t,
                        _ => return Some(*class),
                    }
                }
                Type::ExtensionType { decl: class, .. } => return Some(*class),
                Type::Function { .. } | Type::Record { .. } => return None,
                Type::TypeParameter { param, .. } => cur = self.e.table.param(*param).bound,
                // `X & B` (promoted type variable): the value is a `B`.
                Type::Intersection { bound, .. } => cur = *bound,
                Type::FutureOr { .. } => return None,
            }
        }
        None
    }

    /// The selector key of an instance member: `foo` for read and call,
    /// `foo_=` for write — the same key as in `instance_members`.
    pub(crate) fn instance_member_key(&self, f: FunctionElementId) -> String {
        let func = self.e.program.function(f);
        let name = self.name(func.name);
        if func.kind == FunctionKind::Setter {
            return format!("{name}_=");
        }
        if func.kind == FunctionKind::ImplicitAccessor {
            if let Some(v) = func.variable {
                if self.e.program.variable(v).setter == Some(f) {
                    return format!("{name}_=");
                }
            }
        }
        name.to_string()
    }

    /// Use of a function through its element: static, top-level, extension
    /// or constructor go directly; an instance member becomes a selector
    /// (restricted to the cone of `receiver`, when known).
    pub(crate) fn use_function(&mut self, f: FunctionElementId) {
        self.use_function_with_receiver(f, None);
    }

    pub(crate) fn use_function_with_receiver(&mut self, f: FunctionElementId, receiver: Option<ClassId>) {
        let func = self.e.program.function(f);
        match func.kind {
            FunctionKind::Constructor | FunctionKind::SyntheticConstructor => self.use_constructor(f, false),
            _ => {
                // Member of an extension type: static dispatch (like an
                // extension's), live through use, without waiting for
                // instantiation.
                let in_extension_type = func.class.is_some_and(|c| self.e.program.class(c).kind == ClassKind::ExtensionType);
                let instance = func.class.is_some() && !func.static_ && func.extension.is_none() && !in_extension_type;
                if instance {
                    let n = self.instance_member_key(f);
                    self.new_selector_with_receiver(&n, receiver);
                } else {
                    if let Some(v) = func.variable {
                        if func.kind == FunctionKind::ImplicitAccessor {
                            self.use_variable(v);
                            return;
                        }
                    }
                    if func.extension.is_some() {
                        let n = self.instance_member_key(f);
                        self.new_selector(&n);
                    }
                    if let Some(c) = func.class {
                        self.mark_type(c);
                    }
                    self.mark_fn_live(f);
                }
            }
        }
    }

    fn use_function_directly(&mut self, f: FunctionElementId) {
        self.use_function(f);
        self.mark_fn_live(f);
    }

    /// `C(..)`, `new C.n(..)`, `const C()`, tearoff `C.new`.
    pub(crate) fn use_constructor(&mut self, f: FunctionElementId, tearoff: bool) {
        let func = self.e.program.function(f);
        let Some(c) = func.class else { return };
        if !self.is_user_class(c) {
            return;
        }
        if tearoff {
            self.tearoff[f.0 as usize] = true;
        }
        if func.factory {
            self.mark_type(c);
        } else {
            self.instantiate(c);
        }
        self.mark_fn_live(f);
    }

    /// `C.name(..)` / `C(..)` through the class: the declared constructor, or
    /// — a class with no constructor at all (`class C = S with M`, abstract) —
    /// the direct instantiation, forwarding to the superclass constructor.
    pub(crate) fn create(&mut self, c: ClassId, name: &str, tearoff: bool) {
        if let Some(f) = self.constructor(c, name) {
            self.use_constructor(f, tearoff);
        } else if (name.is_empty() || name == "new") && self.e.program.class(c).constructors.is_empty() && self.is_user_class(c) {
            self.instantiate(c);
            if let Some(sc) = self.super_constructor(c, "") {
                self.use_constructor_from_initializer(sc);
            }
        }
    }

    /// A constructor call as an initializer (`super(..)`, `this.n(..)`,
    /// implicit super): it does not instantiate the class — the instantiated
    /// subclass has already instantiated the chain.
    pub(crate) fn use_constructor_from_initializer(&mut self, f: FunctionElementId) {
        let func = self.e.program.function(f);
        if let Some(c) = func.class {
            self.mark_type(c);
        }
        self.mark_fn_live(f);
    }

    pub(crate) fn mark_type(&mut self, c: ClassId) {
        let i = c.0 as usize;
        if i >= self.classes.len() || !self.is_user_class(c) || self.classes[i] >= ClassLevel::Type {
            return;
        }
        self.classes[i] = ClassLevel::Type;
        self.cause_class[i] = Some(self.cause);
        let p = self.e.program;
        let class = p.class(c);
        // A referenced enum: the constants exist with the class.
        if class.kind == ClassKind::Enum {
            self.instantiate(c);
            // The `_Enum` fields that the emitter reads without going through
            // the model (the enum's synthetic `index`, the `_name` of
            // `toString`). In the element model the enum implements `Enum`; in
            // JS it extends `core._Enum`, which the emitter sets as the
            // superclass.
            if self.include_sdk {
                let en = p.core.and_then(|core| {
                    let s = self.e.interner.lookup("_Enum")?;
                    match p.library(core).declared.get(&s)?.getter? {
                        Element::Class(k) => Some(k),
                        _ => None,
                    }
                });
                if let Some(en) = en {
                    self.instantiate(en);
                    for n in ["index", "_name", "toString", "_enumToString", "name"] {
                        self.new_selector_with_receiver(n, Some(en));
                    }
                }
            }
        }
        // A native class live as a type is instantiated: the object comes from
        // the browser (or is a JS primitive), not from a `new` in the program.
        if self.natives.contains(&c) {
            if self.foreign_open || is_builtin_native(p, c) {
                self.instantiate(c);
            } else if !self.held_natives.contains(&c) {
                self.held_natives.push(c);
            }
        }
        if let Some(ns) = self.natives_by_interface.get(&c).cloned() {
            for n in ns {
                // With the gate closed no native object exists, so the
                // interface does not make the native live, not even as a type.
                if self.foreign_open || is_builtin_native(p, n) {
                    self.mark_type(n);
                } else if !self.held_natives.contains(&n) {
                    self.held_natives.push(n);
                }
            }
        }
        let mut supers: Vec<ClassId> = Vec::new();
        supers.extend(class.supertype_class);
        supers.extend(class.mixin_classes.iter().copied());
        supers.extend(class.interface_classes.iter().copied());
        supers.extend(class.on_classes.iter().copied());
        for s in supers {
            self.mark_type(s);
        }
        // Type arguments of the supertypes enter the class's rti rules.
        if let Some(ct) = self.e.outline.classes.get(i) {
            let mut ts: Vec<dartforge_types::table::TypeId> = Vec::new();
            ts.extend(ct.supertype);
            ts.extend(ct.mixins.iter().copied());
            ts.extend(ct.interfaces.iter().copied());
            ts.extend(ct.on.iter().copied());
            for &tp in ct.type_params.iter() {
                ts.push(self.e.table.param(tp).bound);
            }
            for t in ts {
                self.visit_type(t);
            }
        }
        // Types of the instance fields: the emitter writes `setFieldSignature`
        // for every emitted class, including the type-only one.
        for &vid in &class.fields {
            if p.variable(vid).static_ {
                continue;
            }
            if let Some(vt) = self.e.outline.variables.get(vid.0 as usize) {
                if let Some(t) = vt.declared_type.or(vt.inferred) {
                    self.visit_type(t);
                }
            }
        }
        if let Some(rep) = class.representation {
            if let Some(vt) = self.e.outline.variables.get(rep.0 as usize) {
                if let Some(t) = vt.declared_type.or(vt.inferred) {
                    self.visit_type(t);
                }
            }
        }
    }

    /// Classes cited by a type of the `TypeTable`.
    pub(crate) fn visit_type(&mut self, t: dartforge_types::table::TypeId) {
        if !self.seen_types.insert(t) {
            return;
        }
        let table = self.e.table;
        if t.0 as usize >= table.len() {
            return;
        }
        match table.get(t) {
            Type::Dynamic | Type::Void | Type::Never | Type::Null => {}
            Type::Interface { class, args, .. } | Type::ExtensionType { decl: class, args, .. } => {
                let (class, args) = (*class, args.clone());
                self.mark_type(class);
                for a in args.iter() {
                    self.visit_type(*a);
                }
            }
            Type::Function { type_params, ret, positional, optional, named, .. } => {
                let mut ts: Vec<dartforge_types::table::TypeId> = vec![*ret];
                ts.extend(positional.iter().copied());
                ts.extend(optional.iter().copied());
                ts.extend(named.iter().map(|(_, t, _)| *t));
                ts.extend(type_params.iter().map(|p| table.param(*p).bound));
                for x in ts {
                    self.visit_type(x);
                }
            }
            Type::Record { positional, named, .. } => {
                let mut ts: Vec<dartforge_types::table::TypeId> = positional.to_vec();
                ts.extend(named.iter().map(|(_, t)| *t));
                for x in ts {
                    self.visit_type(x);
                }
            }
            Type::TypeParameter { param, .. } => {
                let b = table.param(*param).bound;
                self.visit_type(b);
            }
            Type::FutureOr { arg, .. } => {
                let a = *arg;
                self.visit_type(a);
            }
            Type::Intersection { param, bound } => {
                let (b, pb) = (*bound, table.param(*param).bound);
                self.visit_type(b);
                self.visit_type(pb);
            }
        }
    }

    /// Instantiates `c` and its superclass chain (`_processInstantiatedClass`,
    /// `resolution_world_builder.dart:706-737`), with the applied mixins.
    pub(crate) fn instantiate(&mut self, c: ClassId) {
        if !self.is_user_class(c) {
            return;
        }
        let p = self.e.program;
        let mut chain: Vec<ClassId> = Vec::new();
        let mut cur = Some(c);
        while let Some(k) = cur {
            if !self.is_user_class(k) || chain.contains(&k) {
                break;
            }
            chain.push(k);
            cur = p.class(k).supertype_class;
        }
        for k in chain.clone() {
            let class = p.class(k);
            let mixins = class.mixin_classes.clone();
            self.raise_to_instantiated(k);
            for mx in mixins {
                if self.is_user_class(mx) {
                    self.raise_to_instantiated(mx);
                }
            }
        }
        // The SDK protocol is relative to the constructed class: a `compareTo`
        // inherited from a superclass without `Comparable` is called too.
        if self.protocol_done.insert(c) {
            self.apply_protocol(c);
        }
    }

    fn raise_to_instantiated(&mut self, k: ClassId) {
        let i = k.0 as usize;
        if self.classes[i] == ClassLevel::Instantiated {
            return;
        }
        self.mark_type(k);
        self.classes[i] = ClassLevel::Instantiated;
        if self.cause_class[i].is_none() {
            self.cause_class[i] = Some(self.cause);
        }
        self.register_instantiated(k);
        self.queue.push_back(Item::Fields(k));
        self.process_members(k, false);
    }

    /// Stores the words (identifiers) of a `JS()` *template*.
    pub(crate) fn record_js_words(&mut self, t: &str) {
        // Only what can be a property name: after `.`, before `:` (an object
        // literal key) or between quotes. Local and global variables of the
        // *template* are not properties.
        let b = t.as_bytes();
        let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
        let mut i = 0;
        let mut quote: Option<u8> = None;
        while i < b.len() {
            let c = b[i];
            if let Some(q) = quote {
                if c == q {
                    quote = None;
                }
            } else if c == b'\'' || c == b'"' {
                quote = Some(c);
            }
            if !ident(c) || c.is_ascii_digit() {
                i += 1;
                continue;
            }
            let start = i;
            while i < b.len() && ident(b[i]) {
                i += 1;
            }
            let w = &t[start..i];
            let mut j = i;
            while j < b.len() && b[j] == b' ' {
                j += 1;
            }
            let after_dot = start > 0 && b[start - 1] == b'.';
            let key = j < b.len() && b[j] == b':' && b.get(j + 1) != Some(&b':');
            if (after_dot || key || quote.is_some()) && !self.js_names.contains(w) {
                self.js_names.insert(w.to_string());
            }
        }
    }

    /// `k` has just been instantiated: it enters the instantiated subtypes of
    /// each of its supertypes and wakes up the pending members that the third
    /// case of [`reaches`] now serves — a member of an owner `M` with
    /// `k ⊑ M`, pending on a name recorded with cone `T` and `k ⊑ T`. It is the
    /// "new class" side of the matching (`_processInstantiatedClass` →
    /// `processClassMembers`, `resolution/enqueuer.dart:135-139`).
    fn register_instantiated(&mut self, k: ClassId) {
        let hier = Arc::clone(&self.hier);
        let sups = hier.supertypes(k);
        for t in sups {
            if let Some(l) = self.inst_sub.get_mut(t.0 as usize) {
                l.push(k);
            }
        }
        if self.pending.is_empty() {
            return;
        }
        let mut fs: Vec<FunctionElementId> = Vec::new();
        for t in sups {
            let Some(names) = self.names_by_cone.get(t.0 as usize) else { continue };
            if names.is_empty() {
                continue;
            }
            for n in names {
                let Some(by_owner) = self.pending.get_mut(n.as_str()) else { continue };
                for m in sups {
                    if let Some(v) = by_owner.remove(m) {
                        fs.extend(v);
                    }
                }
            }
        }
        if fs.is_empty() {
            return;
        }
        self.pending.retain(|_, v| !v.is_empty());
        fs.sort();
        fs.dedup();
        for f in fs {
            self.mark_fn_live(f);
        }
    }

    /// Instance members of `k` (already instantiated): live if the key is a
    /// live selector **for `k`** (unrestricted or in its cone) or a protocol
    /// one; otherwise pending on the key (`foo` and `foo_=` are pending
    /// separately).
    fn process_members(&mut self, k: ClassId, only_pending: bool) {
        let p = self.e.program;
        let class = p.class(k);
        let all = self.all_members.contains(&k) || class.kind == ClassKind::Enum;
        let mut members: Vec<(dartforge_intern::SymbolId, FunctionElementId)> =
            class.instance_members.iter().map(|(s, f)| (*s, *f)).collect();
        members.sort_by_key(|(_, f)| *f);
        for (key, f) in members {
            let func = p.function(f);
            let name = self.name(key).to_string();
            let live = all
                || func.kind == FunctionKind::Operator
                || self.universal_names.contains(name.as_str())
                || self.selector_for(name.as_str(), k);
            if live && !only_pending {
                self.mark_fn_live(f);
            } else if !live {
                self.pending.entry(name).or_default().entry(k).or_default().push(f);
            }
        }
    }

    fn process_chain_members(&mut self, k: ClassId, only_pending: bool) {
        self.process_members(k, only_pending);
    }

    /// Rule (i) of the contract: the implementation, in `c`'s chain, of every
    /// instance member declared in an SDK supertype of `c`. Matching is by
    /// key (`foo_=` matches `set foo`), as for the pending members.
    fn apply_protocol(&mut self, c: ClassId) {
        let p = self.e.program;
        let mut seen: HashSet<ClassId> = HashSet::new();
        let mut work = vec![c];
        let mut names: HashSet<&'a str> = HashSet::new();
        while let Some(k) = work.pop() {
            if !seen.insert(k) {
                continue;
            }
            let class = p.class(k);
            if !self.is_user_class(k) {
                for s in class.instance_members.keys() {
                    names.insert(self.name(*s));
                }
            }
            work.extend(class.supertype_class);
            work.extend(class.mixin_classes.iter().copied());
            work.extend(class.interface_classes.iter().copied());
            work.extend(class.on_classes.iter().copied());
        }
        if names.is_empty() {
            return;
        }
        // The members with those names, in the chain and in the applied mixins.
        let mut targets: Vec<ClassId> = Vec::new();
        let mut cur = Some(c);
        while let Some(k) = cur {
            if !self.is_user_class(k) || targets.contains(&k) {
                break;
            }
            targets.push(k);
            targets.extend(p.class(k).mixin_classes.iter().copied().filter(|m| self.is_user_class(*m)));
            cur = p.class(k).supertype_class;
        }
        for k in targets {
            let mut members: Vec<(dartforge_intern::SymbolId, FunctionElementId)> =
                p.class(k).instance_members.iter().map(|(s, f)| (*s, *f)).collect();
            members.sort_by_key(|(_, f)| *f);
            for (key, f) in members {
                if names.contains(self.name(key)) {
                    self.mark_fn_live(f);
                }
            }
        }
    }

    /// Instance members of the user's extensions: "always instantiated"
    /// owners, live by name (unrestricted **or** restricted — an extension
    /// member has no owner class for the cone). The emitter resolves
    /// extensions on its own (`call.rs::try_extension_call`), so matching
    /// through `Resolved` would diverge from it.
    fn init_extensions(&mut self) {
        if self.extensions_ready {
            return;
        }
        self.extensions_ready = true;
        let p = self.e.program;
        for ext in p.extensions.iter() {
            if !self.is_user_lib(ext.library) {
                continue;
            }
            let mut members: Vec<(dartforge_intern::SymbolId, FunctionElementId)> =
                ext.instance_members.iter().map(|(s, f)| (*s, *f)).collect();
            members.sort_by_key(|(_, f)| *f);
            for (key, f) in members {
                let name = self.name(key).to_string();
                if self.selector_any(name.as_str()) || p.function(f).kind == FunctionKind::Operator {
                    self.mark_fn_live(f);
                } else {
                    self.pending.entry(name).or_default().entry(NO_OWNER).or_default().push(f);
                }
            }
        }
    }

    // ------------------------------------------------------------ the loop

    fn run(&mut self) {
        self.run_items(usize::MAX);
    }

    fn run_items(&mut self, _limit: usize) {
        while let Some(it) = self.queue.pop_front() {
            self.items += 1;
            self.cause = match it {
                Item::Function(f) => Cause::Function(f),
                Item::Variable(v) => Cause::Variable(v),
                Item::Fields(c) => Cause::Class(c),
            };
            match it {
                Item::Function(f) => impact::of_function(self, f),
                Item::Variable(v) => impact::of_variable(self, v),
                Item::Fields(c) => impact::of_fields(self, c),
            }
        }
    }

    /// The current world without consuming the engine.
    fn snapshot(&self) -> World {
        let mut stats = self.statistics();
        stats.processed_items = self.items;
        let mut sel_cone: HashMap<String, Vec<ClassId>> = HashMap::new();
        for (n, cs) in &self.sel_cone {
            let mut v: Vec<ClassId> = cs.iter().copied().collect();
            v.sort();
            sel_cone.insert(n.clone(), v);
        }
        World {
            classes: self.classes.clone(),
            functions: self.fn_live.clone(),
            variables: self.var_live.clone(),
            tearoffs: self.tearoff.clone(),
            selectors: self.sel.clone(),
            sel_cone,
            cause_fn: self.cause_fn.clone(),
            cause_class: self.cause_class.clone(),
            cause_var: self.cause_var.clone(),
            stats,
            include_sdk: self.include_sdk,
            hier: Arc::clone(&self.hier),
            inst_sub: self.inst_sub.clone(),
            js_names: self.js_names.clone(),
        }
    }

    fn statistics(&self) -> Statistics {
        let p = self.e.program;
        let mut stats = Statistics::default();
        for (i, c) in p.classes.iter().enumerate() {
            if p.library(c.library).is_sdk {
                continue;
            }
            stats.user_classes += 1;
            match self.classes[i] {
                ClassLevel::Instantiated => stats.instantiated_classes += 1,
                ClassLevel::Type => stats.type_classes += 1,
                ClassLevel::Dead => {}
            }
        }
        for (i, f) in p.functions.iter().enumerate() {
            if p.library(f.library).is_sdk {
                continue;
            }
            stats.user_functions += 1;
            if self.fn_live[i] {
                stats.live_functions += 1;
            }
        }
        stats.live_variables = self.var_live.iter().filter(|v| **v).count();
        stats.selectors = self.sel.len();
        stats.restricted_selectors = self.sel_cone.len();
        stats
    }

    fn finish(self) -> World {
        let p = self.e.program;
        let mut stats = Statistics::default();
        for (i, c) in p.classes.iter().enumerate() {
            if p.library(c.library).is_sdk {
                continue;
            }
            stats.user_classes += 1;
            match self.classes[i] {
                ClassLevel::Instantiated => stats.instantiated_classes += 1,
                ClassLevel::Type => stats.type_classes += 1,
                ClassLevel::Dead => {}
            }
        }
        for (i, f) in p.functions.iter().enumerate() {
            if p.library(f.library).is_sdk {
                continue;
            }
            stats.user_functions += 1;
            if self.fn_live[i] {
                stats.live_functions += 1;
            }
        }
        stats.live_variables = self.var_live.iter().filter(|v| **v).count();
        stats.selectors = self.sel.len();
        stats.restricted_selectors = self.sel_cone.len();
        stats.processed_items = self.items;
        let mut sel_cone: HashMap<String, Vec<ClassId>> = HashMap::new();
        for (n, cs) in self.sel_cone {
            let mut v: Vec<ClassId> = cs.into_iter().collect();
            v.sort();
            sel_cone.insert(n, v);
        }
        World {
            classes: self.classes,
            functions: self.fn_live,
            variables: self.var_live,
            tearoffs: self.tearoff,
            selectors: self.sel,
            sel_cone,
            cause_fn: self.cause_fn,
            cause_class: self.cause_class,
            cause_var: self.cause_var,
            stats,
            include_sdk: self.include_sdk,
            hier: self.hier,
            inst_sub: self.inst_sub,
            js_names: self.js_names,
        }
    }

    // ------------------------------------------------------------ resolution by name

    /// Constructor `name` of class `c` (`""`/`new` is the unnamed one).
    pub(crate) fn constructor(&self, c: ClassId, name: &str) -> Option<FunctionElementId> {
        let n = if name == "new" { "" } else { name };
        let sym = self.e.interner.lookup(n)?;
        self.e.program.class(c).constructors.get(&sym).copied()
    }

    /// Constructor called by `super.name(..)` from `k`: climbs the synthetic
    /// mixin applications until it finds the one that declares it.
    pub(crate) fn super_constructor(&self, k: ClassId, name: &str) -> Option<FunctionElementId> {
        let p = self.e.program;
        let mut cur = p.class(k).supertype_class;
        let mut n = 0;
        while let Some(s) = cur {
            if let Some(f) = self.constructor(s, name) {
                return Some(f);
            }
            // A mixin application (synthetic or `class C = S with M`) and an
            // abstract class without a constructor: the emitter writes a `new`
            // that only forwards to the superclass (`emit_super_call_default`).
            let no_ctor = p.class(s).constructors.is_empty();
            if (p.class(s).decl.is_some() && !no_ctor) || n > 32 {
                return None;
            }
            cur = p.class(s).supertype_class;
            n += 1;
        }
        None
    }

    /// Every binding of `name` visible without a prefix in `lib`, plus the
    /// statics of the enclosing class and extension.
    pub(crate) fn use_name(&mut self, ctx: &impact::Context, name: SymbolId) {
        let p = self.e.program;
        let text = self.name(name);
        let setter = self.e.interner.lookup(&format!("{text}_="));
        if let Some(c) = ctx.class {
            let class = p.class(c);
            let mut fs: Vec<FunctionElementId> = Vec::new();
            fs.extend(class.static_members.get(&name).copied());
            if let Some(s) = setter {
                fs.extend(class.static_members.get(&s).copied());
            }
            for f in fs {
                self.use_function(f);
            }
        }
        if let Some(x) = ctx.extension {
            self.use_extension_static(x, name);
        }
        if let Some(b) = p.lookup(ctx.library, name) {
            for el in [b.getter, b.setter].into_iter().flatten() {
                self.use_element(el);
            }
        }
    }

    /// `Ext.name` / `name` inside the extension: a static member or a static
    /// field (extension fields have no accessor in the element model, only
    /// `ExtensionElement::fields`).
    pub(crate) fn use_extension_static(&mut self, x: dartforge_elements::model::ExtensionId, name: SymbolId) {
        let p = self.e.program;
        let ext = p.extension(x);
        let setter = self.e.interner.lookup(&format!("{}_=", self.name(name)));
        let mut fs: Vec<FunctionElementId> = Vec::new();
        fs.extend(ext.static_members.get(&name).copied());
        if let Some(s) = setter {
            fs.extend(ext.static_members.get(&s).copied());
        }
        for f in fs {
            self.use_function(f);
        }
        let vs: Vec<VariableId> = ext.fields.iter().copied().filter(|v| p.variable(*v).name == name).collect();
        for v in vs {
            self.use_variable(v);
        }
    }

    pub(crate) fn use_element(&mut self, el: Element) {
        match el {
            Element::Class(c) => self.mark_type(c),
            Element::Extension(_) | Element::Prefix(..) => {}
            Element::Typedef(t) => {
                if let Some(td) = self.e.outline.typedefs.get(t.0 as usize) {
                    let tt = td.target_type;
                    self.visit_type(tt);
                }
            }
            Element::Function(f) => self.use_function(f),
            Element::Variable(v) => self.use_variable(v),
        }
    }

    /// AST node of a variable (its initializer), for the impact.
    pub(crate) fn variable_node(&self, v: VariableId) -> Option<(dartforge_elements::model::UnitId, Option<ast::ExprId>)> {
        let p = self.e.program;
        match p.variable(v).node {
            VariableRef::TopLevel { unit, decl, index } => match &p.unit(unit).ast.decl(decl).kind {
                ast::DeclKind::Variables(l) => Some((unit, l.variables.get(index).and_then(|x| x.initializer))),
                _ => None,
            },
            VariableRef::Field { unit, member, index } => match &p.unit(unit).ast.member(member).kind {
                ast::MemberKind::Field(l) => Some((unit, l.variables.get(index).and_then(|x| x.initializer))),
                _ => None,
            },
            VariableRef::EnumConstant { .. } | VariableRef::Representation { .. } | VariableRef::None => None,
        }
    }

    pub(crate) fn function_node(&self, f: FunctionElementId) -> FunctionRef {
        self.e.program.function(f).node
    }
}
