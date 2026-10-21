//! Parameters that are always `null` (the parameter part of dart2js's type
//! inference, `inferrer/type_graph_inferrer.dart`, reduced to one question:
//! can anything but `null` reach this parameter?).
//!
//! Every parameter of a non-SDK function, constructor or instance method has
//! a value in the lattice `Bottom < Null < Top`, computed from the call sites
//! the world walks:
//!
//! * a **direct** callee (top-level or static function, constructor) has its
//!   own table of arguments; a call site the world recognizes records its
//!   arguments while it *permits* the callee (`ParamFlow::permit`), and any
//!   other way the function becomes used — a tear-off, a root, a
//!   redirection, a forwarding constructor, a use the impact does not
//!   recognize as a call — poisons it (`Engine::touch`);
//! * an **instance** method shares the table of its name (every call site of
//!   the selector, whatever the receiver, and every override), and the name is
//!   poisoned by a read that is not a call (a tear-off), by a name the runtime
//!   or a `JS()` template calls and by the protocol calls the emitter writes
//!   on its own (`iterator`, `add`…). `Object` members and `call` are never
//!   tracked.
//!
//! An argument is `Null` when it is the literal, a constant `null` or a
//! parameter already proved `Null`; an omitted optional parameter takes its
//! default (`Null` when there is none or it is `null`). A parameter written in
//! its body (closures included), an initializing formal and a super parameter
//! are never proved. The proved set lives in [`Constants`], which the world
//! and the emitter consult in the same conditions (`p == null`); when a
//! parameter leaves the set, its function is walked again (the values only
//! rise, so this terminates).

use crate::constants::{Constants, ParamKey};
use crate::{Engine, Item};
use dartforge_elements::model::{ClassKind, FunctionElementId, FunctionKind, FunctionRef};
use dartforge_frontend::ast;
use dartforge_intern::SymbolId;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) enum Value {
    Bottom,
    Null,
    Top,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Slot {
    Positional(usize),
    Named(SymbolId),
}

/// The arguments seen for one callee (or one selector name).
#[derive(Default)]
struct Table {
    poisoned: bool,
    calls: bool,
    /// The fewest positional arguments of a call: the later slots were
    /// omitted at least once.
    min_positional: usize,
    /// The most positional arguments of a call: in JavaScript, one beyond the
    /// callee's positional parameters lands where the named ones are read.
    max_positional: usize,
    positional: Vec<Value>,
    named: HashMap<SymbolId, Value>,
    /// The names passed by **every** call (the others were omitted at least
    /// once).
    named_always: HashSet<SymbolId>,
}

impl Table {
    /// Records one call; whether something changed.
    fn record(&mut self, positional: &[Value], named: &[(SymbolId, Value)]) -> bool {
        if self.poisoned {
            return false;
        }
        let mut changed = false;
        if !self.calls {
            self.calls = true;
            self.min_positional = positional.len();
            self.max_positional = positional.len();
            self.named_always = named.iter().map(|(n, _)| *n).collect();
            changed = true;
        } else {
            if positional.len() < self.min_positional {
                self.min_positional = positional.len();
                changed = true;
            }
            if positional.len() > self.max_positional {
                self.max_positional = positional.len();
                changed = true;
            }
            let before = self.named_always.len();
            self.named_always
                .retain(|n| named.iter().any(|(m, _)| m == n));
            changed |= self.named_always.len() != before;
        }
        for (i, v) in positional.iter().enumerate() {
            if self.positional.len() <= i {
                self.positional.resize(i + 1, Value::Bottom);
            }
            if *v > self.positional[i] {
                self.positional[i] = *v;
                changed = true;
            }
        }
        for (n, v) in named {
            let slot = self.named.entry(*n).or_insert(Value::Bottom);
            if *v > *slot {
                *slot = *v;
                changed = true;
            }
        }
        changed
    }

    fn poison(&mut self) -> bool {
        !std::mem::replace(&mut self.poisoned, true)
    }

    /// The value of `slot` for a callee with `positional_count` positional
    /// parameters.
    fn value(&self, slot: Slot, default_null: bool, positional_count: usize) -> Value {
        if self.poisoned {
            return Value::Top;
        }
        if !self.calls {
            return Value::Bottom;
        }
        if matches!(slot, Slot::Named(_)) && self.max_positional > positional_count {
            return Value::Top;
        }
        let (passed, omitted) = match slot {
            Slot::Positional(i) => (
                self.positional.get(i).copied().unwrap_or(Value::Bottom),
                i >= self.min_positional,
            ),
            Slot::Named(n) => (
                self.named.get(&n).copied().unwrap_or(Value::Bottom),
                !self.named_always.contains(&n),
            ),
        };
        let default = match (omitted, default_null) {
            (false, _) => Value::Bottom,
            (true, true) => Value::Null,
            (true, false) => Value::Top,
        };
        passed.max(default)
    }
}

struct Param {
    key: ParamKey,
    slot: Slot,
    /// The callee's number of positional parameters.
    positional_count: usize,
    default_null: bool,
    /// Neither `this.x`/`super.x` nor written in the body.
    eligible: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tracked {
    No,
    Direct,
    Instance,
}

/// Who a call site calls, for the argument tables.
pub(crate) enum Callee {
    Direct(FunctionElementId),
    Name(SymbolId),
    Untracked,
}

pub(crate) struct ParamFlow {
    tracked: Vec<Tracked>,
    direct: HashMap<FunctionElementId, Table>,
    names: HashMap<SymbolId, Table>,
    /// Tracked instance methods by name.
    by_name: HashMap<SymbolId, Vec<FunctionElementId>>,
    params: HashMap<FunctionElementId, Rc<[Param]>>,
    /// The direct callee of the call being walked: its uses do not poison it.
    pub(crate) permit: Option<FunctionElementId>,
}

impl ParamFlow {
    pub(crate) fn new(e: crate::WorldInput<'_>, universal: &HashSet<String>) -> ParamFlow {
        let p = e.program;
        let mut tracked = vec![Tracked::No; p.functions.len()];
        let mut by_name: HashMap<SymbolId, Vec<FunctionElementId>> = HashMap::new();
        for (i, func) in p.functions.iter().enumerate() {
            if p.library(func.library).is_sdk
                || func.external
                || func.extension.is_some()
                || func
                    .class
                    .is_some_and(|c| p.class(c).kind == ClassKind::ExtensionType)
            {
                continue;
            }
            let t = match (func.kind, func.node) {
                (FunctionKind::Constructor, FunctionRef::Constructor { .. }) => Tracked::Direct,
                (FunctionKind::Function, FunctionRef::Function { .. }) => {
                    if func.class.is_none() || func.static_ {
                        Tracked::Direct
                    } else if universal.contains(e.interner.resolve(func.name)) {
                        Tracked::No
                    } else {
                        Tracked::Instance
                    }
                }
                _ => Tracked::No,
            };
            tracked[i] = t;
            if t == Tracked::Instance {
                by_name
                    .entry(func.name)
                    .or_default()
                    .push(FunctionElementId(i as u32));
            }
        }
        ParamFlow {
            tracked,
            direct: HashMap::new(),
            names: HashMap::new(),
            by_name,
            params: HashMap::new(),
            permit: None,
        }
    }

    fn tracked(&self, f: FunctionElementId) -> Tracked {
        self.tracked
            .get(f.0 as usize)
            .copied()
            .unwrap_or(Tracked::No)
    }

    fn params(&mut self, c: &Constants<'_>, f: FunctionElementId) -> Rc<[Param]> {
        if let Some(ps) = self.params.get(&f) {
            return Rc::clone(ps);
        }
        let ps: Rc<[Param]> = collect_params(c, f).into();
        self.params.insert(f, Rc::clone(&ps));
        ps
    }
}

/// The parameters of `f` with their slots, defaults and eligibility.
fn collect_params(c: &Constants<'_>, f: FunctionElementId) -> Vec<Param> {
    let p = c.program();
    let (unit, span, list): (_, _, &[ast::Parameter]) = match p.function(f).node {
        FunctionRef::Function { unit, function } => {
            let fun = p.unit(unit).ast.function(function);
            match fun.parameters.as_deref() {
                Some(ps) => (unit, fun.span, ps),
                None => return Vec::new(),
            }
        }
        FunctionRef::Constructor { unit, member } => {
            let m = p.unit(unit).ast.member(member);
            match &m.kind {
                ast::MemberKind::Constructor(k) => (unit, m.span, &k.parameters[..]),
                _ => return Vec::new(),
            }
        }
        FunctionRef::None => return Vec::new(),
    };
    let mut out = Vec::new();
    let mut position = 0;
    for prm in list {
        let slot = match prm.kind {
            ast::ParameterKind::Required | ast::ParameterKind::Optional => {
                position += 1;
                Slot::Positional(position - 1)
            }
            ast::ParameterKind::Named => match prm.external_name() {
                Some(n) => Slot::Named(n.sym),
                None => continue,
            },
        };
        let Some(name) = prm.name else { continue };
        let default_null = prm.default_value.is_none_or(|d| c.is_null(unit, d));
        let eligible =
            !prm.this_ && !prm.super_ && !c.written_in(unit, span.start, span.end, name.sym);
        out.push(Param {
            positional_count: 0,
            key: ParamKey {
                unit,
                offset: name.span.start as u32,
                name: name.sym,
            },
            slot,
            default_null,
            eligible,
        });
    }
    for prm in out.iter_mut() {
        prm.positional_count = position;
    }
    out
}

impl Engine<'_> {
    /// `f` becomes used outside a recognized call: a direct callee is
    /// poisoned unless it is the call being walked.
    pub(crate) fn touch(&mut self, f: FunctionElementId) {
        let Some(pf) = self.params.as_ref() else {
            return;
        };
        if pf.permit == Some(f) || pf.tracked(f) != Tracked::Direct {
            return;
        }
        if pf.direct.get(&f).is_some_and(|t| t.poisoned) {
            return;
        }
        self.poison_function(f);
    }

    pub(crate) fn poison_function(&mut self, f: FunctionElementId) {
        let Some(pf) = self.params.as_mut() else {
            return;
        };
        match pf.tracked(f) {
            Tracked::Direct => {
                if pf.direct.entry(f).or_default().poison() {
                    self.recompute(f);
                }
            }
            Tracked::Instance => {
                let n = self.e.program.function(f).name;
                self.poison_name(n);
            }
            Tracked::No => {}
        }
    }

    /// Every method named `name` may be called with anything.
    pub(crate) fn poison_name(&mut self, name: SymbolId) {
        let Some(pf) = self.params.as_mut() else {
            return;
        };
        let Some(fs) = pf.by_name.get(&name).cloned() else {
            return;
        };
        if pf.names.entry(name).or_default().poison() {
            for f in fs {
                self.recompute(f);
            }
        }
    }

    pub(crate) fn poison_name_str(&mut self, name: &str) {
        if self.params.is_none() {
            return;
        }
        if let Some(s) = self.e.interner.lookup(name) {
            self.poison_name(s);
        }
    }

    /// The value of argument `x` (unit `u`).
    pub(crate) fn argument_value(
        &self,
        u: dartforge_elements::model::UnitId,
        x: ast::ExprId,
    ) -> Value {
        match self.constants.as_ref() {
            Some(c) if c.is_null(u, x) => Value::Null,
            _ => Value::Top,
        }
    }

    /// A call site of `callee` with these arguments.
    pub(crate) fn record_call(
        &mut self,
        callee: &Callee,
        positional: &[Value],
        named: &[(SymbolId, Value)],
    ) {
        let Some(pf) = self.params.as_mut() else {
            return;
        };
        match *callee {
            Callee::Direct(f) => {
                if pf.tracked(f) == Tracked::Direct
                    && pf.direct.entry(f).or_default().record(positional, named)
                {
                    self.recompute(f);
                }
            }
            Callee::Name(n) => {
                let Some(fs) = pf.by_name.get(&n).cloned() else {
                    return;
                };
                if pf.names.entry(n).or_default().record(positional, named) {
                    for f in fs {
                        self.recompute(f);
                    }
                }
            }
            Callee::Untracked => {}
        }
    }

    /// Brings the proved set of `f`'s parameters up to date; a parameter that
    /// leaves it sends `f` (when live) to be walked again.
    fn recompute(&mut self, f: FunctionElementId) {
        let (Some(pf), Some(c)) = (self.params.as_mut(), self.constants.as_ref()) else {
            return;
        };
        let ps = pf.params(c, f);
        let table = match pf.tracked(f) {
            Tracked::Direct => pf.direct.get(&f),
            Tracked::Instance => pf.names.get(&self.e.program.function(f).name),
            Tracked::No => None,
        };
        let live = self.fn_live.get(f.0 as usize).copied().unwrap_or(false);
        let mut removed = false;
        for prm in ps.iter() {
            // `Bottom` (no call seen yet) is optimistic in a live function:
            // when the queue drains, a live function still without calls is
            // poisoned (`Engine::poison_uncalled`).
            let v = table.map_or(Value::Bottom, |t| {
                t.value(prm.slot, prm.default_null, prm.positional_count)
            });
            let null = prm.eligible && (v == Value::Null || (v == Value::Bottom && live));
            if null {
                c.add_null_param(prm.key);
            } else {
                removed |= c.remove_null_param(prm.key);
            }
        }
        if removed && live {
            self.queue.push_back(Item::Function(f));
        }
    }

    /// `f` has just become live.
    pub(crate) fn params_live(&mut self, f: FunctionElementId) {
        if self
            .params
            .as_ref()
            .is_some_and(|pf| pf.tracked(f) != Tracked::No)
        {
            self.recompute(f);
        }
    }

    /// When the queue drains: a live function no recognized call reached
    /// (its table, or its name's, has no call and is not poisoned) became
    /// live some other way, with unknown arguments — it is poisoned. Whether
    /// anything changed.
    pub(crate) fn poison_uncalled(&mut self) -> bool {
        let Some(pf) = self.params.as_ref() else {
            return false;
        };
        let p = self.e.program;
        let mut uncalled = Vec::new();
        for (i, live) in self.fn_live.iter().enumerate() {
            if !*live {
                continue;
            }
            let f = FunctionElementId(i as u32);
            let table = match pf.tracked(f) {
                Tracked::Direct => pf.direct.get(&f),
                Tracked::Instance => pf.names.get(&p.function(f).name),
                Tracked::No => continue,
            };
            if !table.is_some_and(|t| t.calls || t.poisoned) {
                uncalled.push(f);
            }
        }
        let changed = !uncalled.is_empty();
        for f in uncalled {
            self.poison_function(f);
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym(i: u32) -> SymbolId {
        let mut interner = dartforge_intern::Interner::default();
        let mut last = interner.intern("_");
        for k in 0..i {
            last = interner.intern(&format!("s{k}"));
        }
        last
    }

    #[test]
    fn omitted_optional_takes_its_default() {
        let mut t = Table::default();
        assert_eq!(t.value(Slot::Positional(0), true, 1), Value::Bottom);
        assert!(t.record(&[], &[]));
        assert_eq!(t.value(Slot::Positional(0), true, 1), Value::Null);
        assert_eq!(t.value(Slot::Positional(0), false, 1), Value::Top);
        let n = sym(1);
        assert_eq!(t.value(Slot::Named(n), true, 0), Value::Null);
        assert_eq!(t.value(Slot::Named(n), false, 0), Value::Top);
    }

    #[test]
    fn values_only_rise() {
        let n = sym(1);
        let mut t = Table::default();
        t.record(&[Value::Null], &[(n, Value::Null)]);
        assert_eq!(t.value(Slot::Positional(0), false, 1), Value::Null);
        assert_eq!(t.value(Slot::Named(n), false, 1), Value::Null);
        // A later call that omits the named one: its non-null default counts.
        assert!(t.record(&[Value::Null], &[]));
        assert_eq!(t.value(Slot::Named(n), false, 1), Value::Top);
        assert_eq!(t.value(Slot::Named(n), true, 1), Value::Null);
        // A value passed once is `Top` for good.
        assert!(t.record(&[Value::Top], &[(n, Value::Null)]));
        assert!(!t.record(&[Value::Null], &[(n, Value::Null)]));
        assert_eq!(t.value(Slot::Positional(0), true, 1), Value::Top);
    }

    #[test]
    fn extra_positional_arguments_reach_the_named_ones() {
        let n = sym(1);
        let mut t = Table::default();
        t.record(&[Value::Top, Value::Top], &[]);
        assert_eq!(t.value(Slot::Named(n), true, 2), Value::Null);
        assert_eq!(t.value(Slot::Named(n), true, 1), Value::Top);
    }

    #[test]
    fn poison_is_top_for_every_slot() {
        let mut t = Table::default();
        assert!(t.poison());
        assert!(!t.poison());
        assert!(!t.record(&[Value::Null], &[]));
        assert_eq!(t.value(Slot::Positional(0), true, 1), Value::Top);
    }
}
