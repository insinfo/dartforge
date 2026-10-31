//! Flow model (`flow-analysis.md`): promotion chains, types of interest,
//! definite assignment and reachability.
//!
//! A [`Flow`] is the specification's `FlowModel`; each local variable has a
//! [`VarModel`] (`PromotionModel`). Joins follow `join`/`joinPM`; assignment
//! follows `demote` + `toi_promote`; promotion by a test follows `promote`.

use super::BodyInferrer;
use super::body::{Base, Body};
use crate::field_promotion::WhyNotPromoted;
use crate::resolved::LocalId;
use crate::table::{Type, TypeId};
use dartforge_diagnostics::Span;
use dartforge_elements::model::FunctionElementId;
use dartforge_intern::SymbolId;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

/// `NonPromotionReason`: why a type is not (or no longer) promoted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NonPromotionReason {
    /// `DemoteViaExplicitWrite`: the variable's name and the write's node (the
    /// assignment, the `++`/`--`, the `for-in` identifier, the pattern).
    Write { name: SymbolId, span: Span },
    /// `PropertyNotPromotedForInherentReason` (with the reason) or
    /// `PropertyNotPromotedForNonInherentReason` (without): the name, the member
    /// (`propertyMember`) and whether field promotion applies in the body's
    /// library (`fieldPromotionEnabled`).
    Property {
        name: SymbolId,
        member: FunctionElementId,
        inherent: Option<WhyNotPromoted>,
        enabled: bool,
    },
    /// `ThisNotPromoted`.
    This,
}

/// `NonPromotionHistory`: the linked list (most recent first) of the types
/// that a write undid.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct History {
    pub ty: TypeId,
    pub reason: NonPromotionReason,
    pub previous: Option<Rc<History>>,
}

/// Write versions (the SSA of `flow-analysis.md`): each write and each join
/// of different versions gets a new number; only equality matters.
static NEXT_VERSION: AtomicU32 = AtomicU32::new(1);

fn new_version() -> u32 {
    NEXT_VERSION.fetch_add(1, Ordering::Relaxed)
}

/// State of a variable at a program point (`PromotionModel`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VarModel {
    /// Promotion chain: the last one is the current type.
    pub chain: Vec<TypeId>,
    /// Types of interest (tested on some path).
    pub tested: Vec<TypeId>,
    pub assigned: bool,
    pub unassigned: bool,
    /// Written by a closure: no longer promotes.
    pub captured: bool,
    /// Version of the last write (0: declared and never written).
    pub version: u32,
    /// `nonPromotionHistory`.
    pub history: Option<Rc<History>>,
}

/// Flow state at a point (`FlowModel`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Flow {
    pub reachable: bool,
    pub vars: Vec<Option<VarModel>>,
}

impl Flow {
    pub fn reachable() -> Self {
        Flow {
            reachable: true,
            vars: Vec::new(),
        }
    }

    pub fn unreachable(&self) -> Self {
        let mut f = self.clone();
        f.reachable = false;
        f
    }

    fn slot(&mut self, id: LocalId) -> &mut Option<VarModel> {
        let i = id.0 as usize;
        if self.vars.len() <= i {
            self.vars.resize(i + 1, None);
        }
        &mut self.vars[i]
    }

    pub fn declare(&mut self, id: LocalId) {
        *self.slot(id) = Some(VarModel {
            chain: Vec::new(),
            tested: Vec::new(),
            assigned: false,
            unassigned: true,
            captured: false,
            version: 0,
            history: None,
        });
    }

    /// Declaration with a value (initializer, parameter, loop variable).
    pub fn initialize(&mut self, id: LocalId) {
        if let Some(m) = self.slot(id) {
            m.assigned = true;
            m.unassigned = false;
            m.version = new_version();
        }
    }

    /// Current write version of a variable.
    pub fn version(&self, id: LocalId) -> Option<u32> {
        self.model(id).map(|m| m.version)
    }

    pub fn model(&self, id: LocalId) -> Option<&VarModel> {
        self.vars.get(id.0 as usize).and_then(|m| m.as_ref())
    }

    pub fn assigned(&self, id: LocalId) -> bool {
        self.model(id).is_some_and(|m| m.assigned)
    }

    pub fn unassigned(&self, id: LocalId) -> bool {
        self.model(id).is_some_and(|m| m.unassigned)
    }

    /// Current (promoted) type of a variable.
    pub fn current_type(&self, id: LocalId, declared: TypeId) -> TypeId {
        self.model(id)
            .and_then(|m| m.chain.last().copied())
            .unwrap_or(declared)
    }

    /// A write in a closure: no promotion and, as in the `conservativeJoin` of
    /// `functionExpression_begin`/`_end`, never definitely unassigned again.
    pub fn capture(&mut self, id: LocalId) {
        if let Some(m) = self.slot(id) {
            // `writeCapture`: no promotions, no tested types and no history.
            m.captured = true;
            m.chain.clear();
            m.tested.clear();
            m.unassigned = false;
            m.version = new_version();
            m.history = None;
        }
    }

    /// Forgets the promotions of the given variables (conservative join of
    /// loops).
    pub fn conservative_join(&mut self, writes: &[LocalId], captured: &[LocalId]) {
        // `discardPromotionsAndMarkNotUnassigned` (the new model does not carry
        // the history).
        for &id in captured {
            if let Some(m) = self.slot(id) {
                m.chain.clear();
                m.unassigned = false;
                m.captured = true;
                m.version = new_version();
                m.history = None;
            }
        }
        for &id in writes {
            if let Some(m) = self.slot(id) {
                m.chain.clear();
                m.unassigned = false;
                m.version = new_version();
                m.history = None;
            }
        }
    }
}

impl<'a> BodyInferrer<'a> {
    /// `join(M1, M2)`. A variable whose two write versions differ gets a new
    /// version, and the promotable properties read on both paths carry over to
    /// it with their joined models (`SsaNode._join`, 3.6.2
    /// `flow_analysis.dart:3932-4016`).
    pub(crate) fn join(&mut self, cx: &mut Body, a: &Flow, b: &Flow) -> Flow {
        if !a.reachable {
            return b.clone();
        }
        if !b.reachable {
            return a.clone();
        }
        if a == b {
            return a.clone();
        }
        let n = a.vars.len().min(b.vars.len());
        let mut vars = Vec::with_capacity(n);
        let mut rejoined = Vec::new();
        for i in 0..n {
            vars.push(match (&a.vars[i], &b.vars[i]) {
                (Some(x), Some(y)) => {
                    let m = join_model(x, y);
                    if x.version != y.version {
                        rejoined.push((LocalId(i as u32), x.version, y.version, m.version));
                    }
                    Some(m)
                }
                _ => None,
            });
        }
        let mut r = Flow {
            reachable: true,
            vars,
        };
        if !rejoined.is_empty() && !cx.fields.is_empty() {
            let properties: HashSet<LocalId> = cx.fields.values().copied().collect();
            for (id, va, vb, v) in rejoined {
                if properties.contains(&id) {
                    continue;
                }
                let base = Base::Local(id);
                join_properties(cx, a, b, &mut r, (base, va), (base, vb), (base, v));
            }
        }
        r
    }

    /// Joins a list of models (empty → unreachable starting from `base`).
    pub(crate) fn join_all(&mut self, cx: &mut Body, base: &Flow, list: &[Flow]) -> Flow {
        let mut it = list.iter();
        let Some(p) = it.next() else {
            return base.unreachable();
        };
        let mut acc = p.clone();
        for f in it {
            acc = self.join(cx, &acc, f);
        }
        acc
    }

    /// `attachFinally` (3.6.2 `flow_analysis.dart:2090-2234`): the state after a
    /// `try`/`finally`. `after_try` is the state after the `try` and its
    /// `catch`es, `before_try` the one before the statement, and
    /// `before_finally`/`after_finally` those around the `finally` block. A
    /// variable the `finally` does not write keeps the `try`'s promotions with
    /// the `finally`'s rebased onto them; one it writes takes the `finally`'s.
    pub(crate) fn attach_finally(
        &mut self,
        cx: &mut Body,
        before_try: &Flow,
        after_try: &Flow,
        before_finally: &Flow,
        after_finally: &Flow,
    ) -> Flow {
        if before_finally == after_finally {
            return after_try.clone();
        }
        if before_finally == before_try && before_try == after_try {
            return after_finally.clone();
        }
        let mut result = after_try.clone();
        result.reachable = after_try.reachable && after_finally.reachable;
        let mut reapply = Vec::new();
        let n = after_try.vars.len().max(after_finally.vars.len());
        for i in 0..n {
            let id = LocalId(i as u32);
            let (v1, v3) = (after_try.model(id), after_finally.model(id));
            if v1 == v3 {
                continue;
            }
            let (v1, v3) = match (v1, v3) {
                (Some(v1), Some(v3)) => (v1, v3),
                (None, Some(v3)) => {
                    *flow_slot(&mut result, id) = Some(v3.clone());
                    continue;
                }
                _ => continue,
            };
            let v2 = before_finally.model(id);
            let (chain, version) = if v2.is_none_or(|v2| v2.version == v3.version) {
                if v1.version != v3.version {
                    reapply.push((id, v1.version, v3.version));
                }
                (self.rebase_promotions(&v1.chain, &v3.chain), v1.version)
            } else {
                (v3.chain.clone(), v3.version)
            };
            let mut m = VarModel {
                chain,
                tested: v3.tested.clone(),
                assigned: v1.assigned || v3.assigned,
                unassigned: v3.unassigned,
                captured: v3.captured,
                version,
                history: None,
            };
            // `_identicalOrNew`.
            if same_model(&m, v1) {
                m.history = v1.history.clone();
            } else if same_model(&m, v3) {
                m.history = v3.history.clone();
            }
            *flow_slot(&mut result, id) = Some(m);
        }
        for (id, to, from) in reapply {
            if after_finally.model(id).is_some_and(|m| m.captured) {
                continue;
            }
            let base = Base::Local(id);
            self.apply_property_promotions(
                cx,
                (base, to),
                (base, from),
                after_finally,
                &mut result,
            );
        }
        result
    }

    /// `_applyPropertyPromotions` (3.6.2 `flow_analysis.dart:3860-3922`): the
    /// promotions of the properties of `from` (the base's version in the
    /// `finally`) at the end of the `finally`, rebased onto those of the same
    /// properties of `to` (its version after the `try`).
    fn apply_property_promotions(
        &mut self,
        cx: &mut Body,
        to: (Base, u32),
        from: (Base, u32),
        after_finally: &Flow,
        result: &mut Flow,
    ) {
        let mut names: Vec<(SymbolId, LocalId)> = cx
            .fields
            .iter()
            .filter(|((b, v, _), _)| (*b, *v) == from)
            .map(|((_, _, name), &id)| (*name, id))
            .collect();
        names.sort_by_key(|&(_, id)| id.0);
        for (name, finally_property) in names {
            let after_try_property = match cx.fields.get(&(to.0, to.1, name)) {
                Some(&id) => id,
                None => {
                    let local = cx.local(finally_property).clone();
                    let id = LocalId(cx.locals.len() as u32);
                    cx.locals.push(local);
                    cx.fields.insert((to.0, to.1, name), id);
                    id
                }
            };
            self.apply_property_promotions(
                cx,
                (Base::Local(after_try_property), 0),
                (Base::Local(finally_property), 0),
                after_finally,
                result,
            );
            let Some(after_finally_model) = after_finally.model(finally_property) else {
                continue;
            };
            let new_model = match result.model(after_try_property) {
                Some(m) => m.clone(),
                None => {
                    result.declare(after_try_property);
                    result.initialize(after_try_property);
                    result.model(after_try_property).cloned().unwrap()
                }
            };
            let rebased = self.rebase_promotions(&after_finally_model.chain, &new_model.chain);
            if rebased != new_model.chain {
                *flow_slot(result, after_try_property) = Some(VarModel {
                    chain: rebased,
                    tested: new_model.tested,
                    assigned: true,
                    unassigned: false,
                    captured: new_model.captured,
                    version: new_model.version,
                    history: None,
                });
            }
        }
    }

    /// `rebasePromotedTypes(this, base)`: `base` followed by the first suffix of
    /// `this` that starts strictly below the last type of `base`.
    fn rebase_promotions(&mut self, this: &[TypeId], base: &[TypeId]) -> Vec<TypeId> {
        let Some(&other) = base.last() else {
            return this.to_vec();
        };
        for (i, &t) in this.iter().enumerate() {
            if t != other && self.sub(t, other) {
                let mut r = base.to_vec();
                r.extend_from_slice(&this[i..]);
                return r;
            }
        }
        base.to_vec()
    }

    /// `promote(x, T)`: promotion by a type test (`is`, `as`, `!= null`).
    pub(crate) fn promote(&mut self, flow: &mut Flow, id: LocalId, declared: TypeId, t: TypeId) {
        self.promote_tested(flow, id, declared, t, t);
    }

    /// Promotes to `t` recording `the_tested` as a type of interest (the false
    /// branch of `x is T` tests `T` and promotes to `factor(S, T)`; the
    /// analyzer's `_finishTypeTest`).
    pub(crate) fn promote_tested(
        &mut self,
        flow: &mut Flow,
        id: LocalId,
        declared: TypeId,
        t: TypeId,
        the_tested: TypeId,
    ) {
        // Also in unreachable code: the model's `tryPromoteForTypeCheck` does
        // not look at reachability (`flow_analysis.dart`), and dead code stays
        // resolved with the promotions.
        let Some(m) = flow.model(id).cloned() else {
            return;
        };
        if m.captured {
            return;
        }
        let s = m.chain.last().copied().unwrap_or(declared);
        // `tryPromoteForTypeCheck` (3.6.2 `flow_analysis.dart:2534-2542`):
        // `tryPromoteToType` gives `T` when `T <: S`, and only does not promote
        // when the result is the previous type itself. Mutual and different
        // subtypes (`dynamic Function()` and `void Function()`) promote.
        if t == s {
            // The same type: does not promote, but records the type of interest.
            if !m.tested.contains(&the_tested)
                && let Some(Some(mm)) = flow.vars.get_mut(id.0 as usize)
            {
                mm.tested.push(the_tested);
            }
            return;
        }
        let t1 = if self.sub(t, s) {
            Some(t)
        } else {
            // S is X (bound R) or X & R: promotes to X & T if T <: R.
            match self.table.get(s).clone() {
                // `X?` too, when `T` has no `?` (`_promotedTypeParameterTypeNullability`,
                // 6.11 `type_system.dart:2104-2121`: the intersection with a
                // non-nullable type is non-nullable). With `T?` the result would
                // be `(X & T)?`, which the table does not represent: no promotion.
                Type::TypeParameter { param, nullable }
                    if param != self.core.unknown_param
                        && (!nullable || !self.table.get(t).is_declared_nullable()) =>
                {
                    let b = self.table.param(param).bound;
                    if self.sub(t, b) {
                        Some(self.table.intern(Type::Intersection { param, bound: t }))
                    } else {
                        None
                    }
                }
                Type::Intersection { param, bound } => {
                    if self.sub(t, bound) {
                        Some(self.table.intern(Type::Intersection { param, bound: t }))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        };
        if let Some(Some(mm)) = flow.vars.get_mut(id.0 as usize) {
            if !mm.tested.contains(&the_tested) {
                mm.tested.push(the_tested);
            }
            if let Some(t1) = t1 {
                mm.chain.push(t1);
            }
        }
        if let Some(t1) = t1
            && matches!(self.table.get(t1), Type::Never)
        {
            flow.reachable = false;
        }
    }

    /// The false branch of `x is T` (`tryPromoteForTypeCheck`, 3.6.2
    /// `flow_analysis.dart:2544-2557`, with the `_finishTypeTest` of `:2633-2664`):
    /// the factored type goes straight into the chain, without the true branch's
    /// `tryPromoteToType` (`FutureOr<void>` minus `Future` is `void`, which is a
    /// supertype), and `T` is recorded as tested.
    pub(crate) fn promote_factored(
        &mut self,
        flow: &mut Flow,
        id: LocalId,
        factored: Option<TypeId>,
        the_tested: TypeId,
    ) {
        let Some(m) = flow.model(id) else { return };
        if m.captured {
            return;
        }
        if let Some(Some(mm)) = flow.vars.get_mut(id.0 as usize) {
            if !mm.tested.contains(&the_tested) {
                mm.tested.push(the_tested);
            }
            if let Some(t) = factored {
                mm.chain.push(t);
            }
        }
    }

    /// `promoteToNonNull(x)`.
    pub(crate) fn promote_non_null(&mut self, flow: &mut Flow, id: LocalId, declared: TypeId) {
        let current = flow.current_type(id, declared);
        let nn = self.non_null_promotion(current);
        if nn != current {
            self.promote(flow, id, declared, nn);
        }
    }

    /// `tryMarkNonNullable` of a reference whose type is `previous` (a
    /// property's or `this`'s unpromoted type): appends **NonNull**(`previous`)
    /// to the chain without comparing it with the current promotion, records no
    /// tested type and does not change reachability (`_finishTypeTest`).
    pub(crate) fn mark_non_nullable(&mut self, flow: &mut Flow, id: LocalId, previous: TypeId) {
        let nn = self.non_null_promotion(previous);
        if nn == previous {
            return;
        }
        let Some(Some(m)) = flow.vars.get_mut(id.0 as usize) else {
            return;
        };
        if !m.captured && m.chain.last() != Some(&nn) {
            m.chain.push(nn);
        }
    }

    /// **NonNull**(`T`) for promotion (`T?` of a type variable becomes `T`).
    pub(crate) fn non_null_promotion(&mut self, t: TypeId) -> TypeId {
        match self.table.get(t).clone() {
            Type::Null => self.core.never,
            Type::FutureOr {
                arg,
                nullable: true,
            } => self.table.intern(Type::FutureOr {
                arg,
                nullable: false,
            }),
            // NonNull(X) = X & NonNull(B) when the bound B is nullable (the
            // analyzer's promoteToNonNull): T? becomes T & Object.
            Type::TypeParameter { param, .. } if param != self.core.unknown_param => {
                let b = self.table.param(param).bound;
                let nb = self.non_null_promotion(b);
                if nb != b && !matches!(self.table.get(b), Type::Dynamic) {
                    self.table.intern(Type::Intersection { param, bound: nb })
                } else {
                    self.table.intern(Type::TypeParameter {
                        param,
                        nullable: false,
                    })
                }
            }
            _ => self.non_null(t),
        }
    }

    /// `rebaseForward` (§7.10): the promotions stored in a condition variable
    /// apply again over the current state, for the variables that were not
    /// written since then (same version).
    pub(crate) fn reapply(&mut self, current: &Flow, stored: &Flow) -> Flow {
        if !stored.reachable {
            return current.unreachable();
        }
        let mut r = current.clone();
        for (i, g) in stored.vars.iter().enumerate() {
            let Some(g) = g else { continue };
            let Some(Some(c)) = r.vars.get(i) else {
                continue;
            };
            if g.version != c.version || c.captured {
                continue;
            }
            let mut c = c.clone();
            let before = (c.chain.len(), c.tested.len());
            for &t in &g.chain {
                if !c.chain.contains(&t) && c.chain.last().is_none_or(|&l| self.sub(t, l)) {
                    c.chain.push(t);
                }
            }
            for &t in &g.tested {
                if !c.tested.contains(&t) {
                    c.tested.push(t);
                }
            }
            // `_identicalOrNew`: a new model does not carry the history.
            if (c.chain.len(), c.tested.len()) != before {
                c.history = None;
            }
            r.vars[i] = Some(c);
        }
        r
    }

    /// `assign(x, T)`: demotes and applies promotion by type of interest.
    /// `reason`: the write's `DemoteViaExplicitWrite`, which goes into the
    /// history of each undone type.
    pub(crate) fn assign_flow(
        &mut self,
        flow: &mut Flow,
        id: LocalId,
        declared: TypeId,
        written: TypeId,
        reason: Option<NonPromotionReason>,
    ) {
        self.write_flow(flow, id, declared, written, true, reason);
    }

    /// A write with or without promotion by type of interest (the
    /// initialization of a `final` variable or one without a written type does
    /// not promote, `_initialize`).
    pub(crate) fn write_flow(
        &mut self,
        flow: &mut Flow,
        id: LocalId,
        declared: TypeId,
        written: TypeId,
        toi: bool,
        reason: Option<NonPromotionReason>,
    ) {
        let Some(m) = flow.model(id).cloned() else {
            return;
        };
        let mut m = m;
        let assigned_before = m.assigned;
        m.assigned = true;
        m.unassigned = false;
        m.version = new_version();
        if m.captured {
            m.history = None;
        } else {
            let chain_before = m.chain.clone();
            // A written `InvalidType` is a subtype of everything (it promotes to the
            // non-nullable of the declared type like a `Never`); writing `dynamic`
            // to a typed local is an implicit cast: the effective written type is
            // the declared one.
            let written = if self.table.is_invalid(written) && !self.is_dynamic(declared) {
                self.core.never
            } else if self.is_dynamic(written) && !self.is_dynamic(declared) {
                declared
            } else {
                written
            };
            // demote (`_demoteViaAssignment`): the chain is decreasing, so what
            // remains is a prefix; each undone type goes into the history, from
            // the last to the first (the first undone ends up in front).
            let mut chain: Vec<TypeId> = Vec::new();
            for &t in &m.chain {
                if self.sub(written, t) {
                    chain.push(t);
                }
            }
            let mut history = m.history.clone();
            if let Some(reason) = &reason {
                for &t in m.chain[chain.len()..].iter().rev() {
                    history = Some(Rc::new(History {
                        ty: t,
                        reason: reason.clone(),
                        previous: history,
                    }));
                }
            }
            // toi_promote
            if !toi {
                if !self.sub(written, declared) {
                    chain.clear();
                }
            } else if self.sub(written, declared) {
                let provisional = chain.last().copied().unwrap_or(declared);
                if written != provisional {
                    let mut interesting: Vec<TypeId> = Vec::new();
                    let nn = self.non_null_promotion(declared);
                    if nn != declared {
                        interesting.push(nn);
                    }
                    for &t in &m.tested {
                        interesting.push(t);
                        let nt = self.non_null_promotion(t);
                        interesting.push(nt);
                    }
                    interesting.retain(|t| *t != provisional);
                    interesting.dedup();
                    if interesting.contains(&written) {
                        chain.push(written);
                    } else {
                        let mut p3: Vec<TypeId> = Vec::new();
                        for &t in &interesting {
                            if self.sub(written, t) && self.sub(t, provisional) && !p3.contains(&t)
                            {
                                p3.push(t);
                            }
                        }
                        let mut best = None;
                        for &t in &p3 {
                            if p3.iter().all(|&u| self.sub(t, u)) {
                                best = Some(t);
                                break;
                            }
                        }
                        if let Some(t) = best {
                            chain.push(t);
                        }
                    }
                }
            } else {
                chain.clear();
            }
            // `PromotionModel.write`: with no change in the chain of an already
            // assigned variable, the new model does not carry history; otherwise it
            // carries the demotion's. The tested types are dropped when the
            // promotion goes away entirely.
            if chain == chain_before && assigned_before {
                m.history = None;
            } else {
                if chain.is_empty() && !chain_before.is_empty() {
                    m.tested.clear();
                }
                m.history = history;
            }
            m.chain = chain;
        }
        *flow_slot(flow, id) = Some(m);
    }
}

fn flow_slot(f: &mut Flow, id: LocalId) -> &mut Option<VarModel> {
    let i = id.0 as usize;
    if f.vars.len() <= i {
        f.vars.resize(i + 1, None);
    }
    &mut f.vars[i]
}

/// The joined properties of a base (`_joinProperties`): each promotable
/// property read on both paths (under `first` in `a`, `second` in `b`) gets a
/// new synthetic under `joined`, with the join of the two models when both
/// paths have one, and the same for its own properties.
fn join_properties(
    cx: &mut Body,
    a: &Flow,
    b: &Flow,
    r: &mut Flow,
    first: (Base, u32),
    second: (Base, u32),
    joined: (Base, u32),
) {
    let mut names: Vec<(SymbolId, LocalId, LocalId)> = Vec::new();
    for (&(base, version, name), &x) in cx.fields.iter() {
        if (base, version) == first
            && let Some(&y) = cx.fields.get(&(second.0, second.1, name))
        {
            names.push((name, x, y));
        }
    }
    names.sort_by_key(|&(_, x, _)| x.0);
    for (name, x, y) in names {
        let local = cx.local(x).clone();
        let id = LocalId(cx.locals.len() as u32);
        cx.locals.push(local);
        cx.fields.insert((joined.0, joined.1, name), id);
        if let (Some(mx), Some(my)) = (a.model(x), b.model(y)) {
            *flow_slot(r, id) = Some(join_model(mx, my));
        }
        join_properties(
            cx,
            a,
            b,
            r,
            (Base::Local(x), 0),
            (Base::Local(y), 0),
            (Base::Local(id), 0),
        );
    }
}

/// The same model, apart from the history.
fn same_model(x: &VarModel, r: &VarModel) -> bool {
    x.chain == r.chain
        && x.tested == r.tested
        && x.assigned == r.assigned
        && x.unassigned == r.unassigned
        && x.captured == r.captured
        && x.version == r.version
}

/// `joinPM`: chains by the longest common subsequence, tested types by the
/// union. The history is only kept when the result is one of the two models
/// (`PromotionModel.join` returns `first` or `second` unchanged).
fn join_model(a: &VarModel, b: &VarModel) -> VarModel {
    let mut r = join_model_without_history(a, b);
    r.history = if same_model(a, &r) {
        a.history.clone()
    } else if same_model(b, &r) {
        b.history.clone()
    } else {
        None
    };
    r
}

fn join_model_without_history(a: &VarModel, b: &VarModel) -> VarModel {
    let chain: Vec<TypeId> = a
        .chain
        .iter()
        .copied()
        .filter(|t| b.chain.contains(t))
        .collect();
    let mut tested = a.tested.clone();
    for t in &b.tested {
        if !tested.contains(t) {
            tested.push(*t);
        }
    }
    VarModel {
        chain,
        tested,
        assigned: a.assigned && b.assigned,
        unassigned: a.unassigned && b.unassigned,
        captured: a.captured || b.captured,
        version: if a.version == b.version {
            a.version
        } else {
            new_version()
        },
        history: None,
    }
}
