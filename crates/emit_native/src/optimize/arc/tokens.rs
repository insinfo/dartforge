//! Linear availability of tokens in the prepared CFG, with Phi per edge.
//! The inventory and the effects are semantic contracts supplied by the producer.
//! Barriers of derived borrows are conservative; externs, slot aliases,
//! regions, keepalive and versions/pins still require producers.

use super::super::{
    cfg::Cfg,
    operands::{operands, terminator_operands},
};
use super::{OwnerOrigin, Ownership};
use crate::hir::*;
use std::collections::{HashMap, HashSet, VecDeque};

/// Explicit consumption of the arguments of an ordinary instruction.
///
/// The lists preserve multiplicity: consuming the same token twice
/// requires two distinct copies. There is no implicit contract for a missing call.
///
/// ```
/// use dartforge_emit_native::optimize::arc::TokenEffect;
/// let effect = TokenEffect::default();
/// assert!(!effect.may_fail && effect.error.is_empty());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TokenEffect {
    /// Consumption both on success and on failure, before the result.
    pub always: Vec<ValueId>,
    /// Consumption only on success.
    pub success: Vec<ValueId>,
    /// Consumption only on the failure of an invoke or of a runtime pending check.
    pub error: Vec<ValueId>,
    /// Requires an exceptional exit in an invoke or in the pending map of the plan.
    pub may_fail: bool,
}

/// Contract for transferring the result to the caller.
///
/// ```
/// use dartforge_emit_native::optimize::arc::TokenReturn;
/// assert_ne!(TokenReturn::Owned, TokenReturn::Borrowed);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TokenReturn {
    /// A result without a managed token (void functions included).
    #[default]
    Trivial,
    /// Transfers an owned token; null requires no physical count.
    Owned,
    /// A borrow sustained by the caller, never by a local owner.
    Borrowed,
}

/// Contracts on the same version of the CFG and of the ownership inventory.
///
/// Supply one entry per ordinary instruction, also without consumption; the
/// ARC and Phi operations have their own rules and do not accept overrides.
/// A missing plan does not mean that the arguments are borrowed.
///
/// ```
/// use dartforge_emit_native::optimize::arc::TokenPlan;
/// assert!(TokenPlan::default().instructions.is_empty());
/// assert!(TokenPlan::default().pendings.is_empty());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TokenPlan {
    /// Exact effects before the result and on the invoke/pending exits.
    pub instructions: HashMap<ValueId, TokenEffect>,
    /// Convention of the return, not deduced from the width of the representation.
    pub ret: TokenReturn,
    /// A pending runtime call and the error block after an explicit check.
    /// They are not LLVM landing pads. Requires the exception_pending/ICmp Ne zero suffix and
    /// a CondBranch to error/success, with no other operation between the call and the test.
    pub pendings: HashMap<ValueId, BlockId>,
    /// Native roles of frame/index pairs of a verified closed family.
    /// It is neither a Ref owner nor a replacement for the foreign-unwind certificate.
    pub shared_frames: Option<super::SharedFramesCfgContract>,
    /// `Ptr` parameters of a direct local function that receive the address of a
    /// scalar local of its declarer (capture by address), with its
    /// type; initialized at the entry (`locals::produce_addresses`).
    pub received_addresses: HashMap<ValueId, Type>,
    /// A call to a direct local function → position and type of the addresses of scalar
    /// locals it yields to the callee.
    pub yielded_addresses: HashMap<ValueId, Vec<(usize, Type)>>,
}

fn own(inst: &Instruction) -> bool {
    matches!(
        inst,
        Instruction::ArcCopy { .. }
            | Instruction::ArcMove { .. }
            | Instruction::ArcDrop { .. }
            | Instruction::ArcLoadStrong { .. }
            | Instruction::ArcStoreStrong { .. }
            | Instruction::Phi { .. }
    )
}

fn check(
    v: ValueId,
    classes: &HashMap<ValueId, Ownership>,
    active_set: &HashSet<ValueId>,
) -> Result<(), String> {
    let mut current = v;
    loop {
        match classes[&current] {
            Ownership::Trivial => return Ok(()),
            Ownership::Owned if active_set.contains(&current) => return Ok(()),
            Ownership::Owned => {
                return Err(format!(
                    "v{} depends on the unavailable token v{}",
                    v.0, current.0
                ));
            }
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                ..
            } => return Ok(()),
            Ownership::Borrowed {
                owner: OwnerOrigin::Value(owner),
                ..
            } => current = owner,
        }
    }
}

fn use_value(
    op: &Operand,
    classes: &HashMap<ValueId, Ownership>,
    active_set: &HashSet<ValueId>,
) -> Result<(), String> {
    if let Operand::Val(v) = op {
        check(*v, classes, active_set)?;
    }
    Ok(())
}

fn consume(
    v: ValueId,
    classes: &HashMap<ValueId, Ownership>,
    active_set: &mut HashSet<ValueId>,
) -> Result<(), String> {
    if classes[&v] != Ownership::Owned {
        return Err(format!("consumption of v{} requires Owned", v.0));
    }
    if !active_set.remove(&v) {
        return Err(format!("token v{} already consumed or unavailable", v.0));
    }
    Ok(())
}

fn consume_operand(
    op: &Operand,
    classes: &HashMap<ValueId, Ownership>,
    active_set: &mut HashSet<ValueId>,
) -> Result<(), String> {
    match op {
        Operand::Val(v) => consume(*v, classes, active_set),
        Operand::Constant(Constant::Null) => Ok(()),
        _ => Err("consumption requires an SSA token or null".into()),
    }
}

fn apply(
    ids: &[ValueId],
    classes: &HashMap<ValueId, Ownership>,
    active_set: &mut HashSet<ValueId>,
) -> Result<(), String> {
    for &v in ids {
        consume(v, classes, active_set)?;
    }
    Ok(())
}

fn produce(
    v: ValueId,
    classes: &HashMap<ValueId, Ownership>,
    active_set: &mut HashSet<ValueId>,
) -> Result<(), String> {
    if classes[&v] == Ownership::Owned && !active_set.insert(v) {
        return Err(format!(
            "new production of v{} with the previous token still available",
            v.0
        ));
    }
    check(v, classes, active_set)
}

fn of_caller(mut v: ValueId, classes: &HashMap<ValueId, Ownership>) -> bool {
    loop {
        match classes[&v] {
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                ..
            } => return true,
            Ownership::Borrowed {
                owner: OwnerOrigin::Value(owner),
                ..
            } => v = owner,
            _ => return false,
        }
    }
}

/// The inventory already excluded cycles; borrowed aliases keep the root.
fn owner_root(mut v: ValueId, classes: &HashMap<ValueId, Ownership>) -> Option<OwnerOrigin> {
    loop {
        match classes[&v] {
            Ownership::Trivial => return None,
            Ownership::Owned => return Some(OwnerOrigin::Value(v)),
            Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                ..
            } => return Some(OwnerOrigin::Caller),
            Ownership::Borrowed {
                owner: OwnerOrigin::Value(next_item),
                ..
            } => v = next_item,
        }
    }
}

/// Inherited lexical limits must also survive the creation of an alias.
fn owner_scopes(mut v: ValueId, classes: &HashMap<ValueId, Ownership>) -> HashSet<u32> {
    let mut scopes = HashSet::new();
    while let Ownership::Borrowed { owner, scope } = classes[&v] {
        if scope != 0 {
            scopes.insert(scope);
        }
        match owner {
            OwnerOrigin::Caller => break,
            OwnerOrigin::Value(next_item) => v = next_item,
        }
    }
    scopes
}

fn path(f: &Function, parents: &[Option<usize>], mut b: usize) -> Vec<u32> {
    let mut r = vec![f.blocks[b].id.0];
    while let Some(parent) = parents[b] {
        b = parent;
        r.push(f.blocks[b].id.0);
    }
    r.reverse();
    r
}

fn flow_error(f: &Function, parents: &[Option<usize>], b: usize, message: String) -> String {
    format!(
        "tokens in {} b{}: {message}; path {:?}",
        f.symbol,
        f.blocks[b].id.0,
        path(f, parents, b)
    )
}

/// Checks availability, transfer and single consumption in the prepared CFG.
///
/// Requires a valid CFG/SSA and representations, a complete semantic inventory and
/// contracts previously certified by the producer. An invoke only produces a token
/// on success. An owned Phi consumes its entries simultaneously on the edge, including
/// swaps on backedges; null produces a logical obligation without a physical RC.
/// Joins require equal inventories, without a union that hides conditional consumption.
/// Exits must consume every local token, transferring the owned return.
/// Borrowed may only return when its chain ends at the caller.
/// Rebuilds the contracts of private scalar locals on the current body, including
/// absence of escape, typing, initialization and Trivial written values.
/// Also checks the covered pure operations, their type, empty effect and
/// the Trivial origin of the scalar operands, by the same rules as the producer.
///
/// Does not insert operations, certify the other contracts, verify lexical scopes,
/// slot aliases/invalidation, regions or the cleanup of suspended states.
/// Rejects aliases derived after barriers and as reentry arguments;
/// does not insert keepalive and depends on the supplied classification/provenance.
/// These obligations remain necessary to accept the whole ARC pipeline.
///
/// # Errors
/// Returns the symbol, the point and the path for an unavailable use/consumption, a token leak,
/// an incompatible join, an invalid return or missing/stale metadata. Also
/// propagates coverage, dependency and exceptional shape diagnostics.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "f".into(), name: "f".into(), debug_info: None,
///     params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
///         (ValueId(1), Instruction::ArcCopy { value: Operand::Val(ValueId(0)) }, Type::Ref),
///         (ValueId(2), Instruction::ArcDrop { value: Operand::Val(ValueId(1)) }, Type::Void)],
///         terminator: Terminator::Return(None) }] };
/// let classes = HashMap::from([(ValueId(0), Ownership::Borrowed {
///     owner: OwnerOrigin::Caller, scope: 0 }), (ValueId(1), Ownership::Owned),
///     (ValueId(2), Ownership::Trivial)]);
/// verify_tokens(&f, &classes, &FunctionTables::default(), &TokenPlan::default())?;
/// # Ok::<(), String>(())
/// ```
pub fn verify_tokens(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    tables: &FunctionTables,
    plan: &TokenPlan,
) -> Result<(), String> {
    if plan.shared_frames != tables.shared_frames {
        return Err("ARC tokens: shared frames contracts diverge from the tables".into());
    }
    analyze_tokens(f, classes, tables, plan, false, false).map(|_| ())
}

/// Checks the same flow, admitting only remaining tokens at Return.
/// Does not relax double consumption, availability, convention or joins.
pub(super) fn cleanup_exits(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    tables: &FunctionTables,
    plan: &TokenPlan,
) -> Result<HashMap<BlockId, Vec<ValueId>>, String> {
    analyze_tokens(f, classes, tables, plan, true, false).map(|flow| flow.exits)
}

// The planning mode only admits discarding dead tokens per edge.
// The public verifier still requires identical inventories without discarding.
pub(super) fn cleanup_edges(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    tables: &FunctionTables,
    plan: &TokenPlan,
) -> Result<HashMap<(BlockId, BlockId), Vec<ValueId>>, String> {
    analyze_tokens(f, classes, tables, plan, true, true).map(|flow| flow.edges)
}

/// Hands over the validated linear inventory at the entry of each reachable landing pad.
///
/// Applies the consumptions of the error edge and excludes results exclusive to
/// success. Includes Phi tokens transferred through the edge; sorts by ValueId.
/// Does not generate foreign cleanup nor replace the check of the scopes.
/// Must be recomputed after any change of the CFG or of the contracts.
///
/// # Errors
/// A token verification failure or a landing pad without a reachable entry.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "empty".into(), name: "empty".into(), debug_info: None,
///     params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
///     id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
/// assert!(tokens_at_landing_pad_entry(&f, &HashMap::new(),
///     &FunctionTables::default(), &TokenPlan::default())?.is_empty());
/// # Ok::<(), String>(())
/// ```
pub fn tokens_at_landing_pad_entry(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    tables: &FunctionTables,
    plan: &TokenPlan,
) -> Result<HashMap<BlockId, Vec<ValueId>>, String> {
    let entries = analyze_tokens(f, classes, tables, plan, false, false)?.entries;
    let mut landing_pads: Vec<_> = tables.landing_pads.iter().copied().collect();
    landing_pads.sort_by_key(|b| b.0);
    let mut result = HashMap::new();
    for b in landing_pads {
        let owners = entries.get(&b).ok_or_else(|| {
            format!(
                "tokens in {}: landing pad b{} without a reachable entry",
                f.symbol, b.0
            )
        })?;
        result.insert(b, owners.clone());
    }
    Ok(result)
}

pub(super) fn check_exceptional_exits(
    f: &Function,
    tables: &FunctionTables,
    require_canonical: bool,
) -> Result<(), String> {
    if tables.exits.is_empty() {
        return Ok(());
    }
    let blocks: HashMap<_, _> = f.blocks.iter().map(|b| (b.id, b)).collect();
    let dests: HashSet<_> = tables.invocations.values().copied().collect();
    let mut exits: Vec<_> = tables.exits.iter().collect();
    exits.sort_by_key(|(b, _)| b.0);
    for (id, mode) in exits {
        let b = blocks
            .get(id)
            .ok_or_else(|| format!("stale exceptional exit b{}", id.0))?;
        let Terminator::Return(op) = &b.terminator else {
            return Err(format!("exceptional exit b{} is not a Return", id.0));
        };
        if *mode == ExceptionExit::Guard {
            return Err(format!(
                "Guard exit b{} requires an explicit CFG before ARC",
                id.0
            ));
        }
        if *mode == ExceptionExit::Resume {
            if !tables.landing_pads.contains(id)
                || !dests.contains(id)
                || f.blocks.first().is_some_and(|entry| entry.id == *id)
            {
                return Err(format!("Resume exit b{} requires an invoke landing pad", id.0));
            }
            if b.instructions
                .iter()
                .any(|(_, i, _)| !super::resume_instruction(i))
            {
                return Err(format!(
                    "Resume exit b{} only admits ARC drops and closing of local frames",
                    id.0
                ));
            }
        }
        let expected = if f.return_ty == Type::Void {
            None
        } else {
            Some(super::super::operands::default_constant(f.return_ty))
        };
        if require_canonical && *op != expected {
            return Err(format!(
                "{mode:?} exit b{} requires a canonical return without transfer",
                id.0
            ));
        }
    }
    Ok(())
}

// Only normalizes temporary bodies: the caller publishes after verifying everything.
pub(super) fn normalize_exits_throws(
    f: &mut Function,
    tables: &FunctionTables,
) -> Result<(), String> {
    super::ssa::verify(f)?;
    check_exceptional_exits(f, tables, false)?;
    for b in &mut f.blocks {
        if matches!(
            tables.exits.get(&b.id),
            Some(ExceptionExit::Throws | ExceptionExit::Resume)
        ) {
            b.terminator = Terminator::Return(if f.return_ty == Type::Void {
                None
            } else {
                Some(super::super::operands::default_constant(f.return_ty))
            });
        }
    }
    Ok(())
}

struct TokenFlow {
    exits: HashMap<BlockId, Vec<ValueId>>,
    entries: HashMap<BlockId, Vec<ValueId>>,
    edges: HashMap<(BlockId, BlockId), Vec<ValueId>>,
}

fn analyze_tokens(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    tables: &FunctionTables,
    plan: &TokenPlan,
    allow_cleanup: bool,
    clear_edges: bool,
) -> Result<TokenFlow, String> {
    super::locals::verify(f, classes, plan)?;
    super::pure_set::verify(f, classes, plan)?;
    check_exceptional_exits(f, tables, true)?;
    super::check_resumes(f, tables)?;
    let live_set = super::classification::liveness_with_exits(f, classes, tables, &plan.pendings)?;
    let mut edges = HashMap::new();
    let meta_error = |m: String| format!("tokens in {}: {m}", f.symbol);
    let mut expected_set = HashSet::new();
    for b in &f.blocks {
        for (v, inst, _) in &b.instructions {
            if own(inst) {
                if let Instruction::ArcLoadStrong {
                    slot:
                        StrongSlot::Frame {
                            frame: Operand::Val(q),
                            ..
                        },
                }
                | Instruction::ArcStoreStrong {
                    slot:
                        StrongSlot::Frame {
                            frame: Operand::Val(q),
                            ..
                        },
                    ..
                } = inst
                {
                    if classes[q] != Ownership::Trivial {
                        return Err(meta_error(format!(
                            "v{}: frame ID must be Trivial",
                            v.0
                        )));
                    }
                }
                let class = if matches!(
                    inst,
                    Instruction::ArcDrop { .. } | Instruction::ArcStoreStrong { .. }
                ) {
                    Some(Ownership::Trivial)
                } else if matches!(
                    inst,
                    Instruction::ArcCopy { .. }
                        | Instruction::ArcMove { .. }
                        | Instruction::ArcLoadStrong { .. }
                ) {
                    Some(Ownership::Owned)
                } else {
                    None
                };
                if class.is_some_and(|c| classes[v] != c) || tables.invocations.contains_key(v) {
                    return Err(meta_error(format!(
                        "v{}: contract incompatible with ARC/Phi operation",
                        v.0
                    )));
                }
                continue;
            }
            expected_set.insert(*v);
            let e = plan
                .instructions
                .get(v)
                .ok_or_else(|| meta_error(format!("v{} without a consumption contract", v.0)))?;
            let unbox_bool = super::contracts::unbox_bool_call(inst);
            let call = unbox_bool.as_ref().unwrap_or(inst);
            if let Instruction::CallRuntime { name, .. } = call {
                if super::contract(name).is_ok() {
                    let c = super::runtime_call_contract(call).map_err(meta_error)?;
                    if classes[v] != c.result
                        || e.always != c.effect.always
                        || e.success != c.effect.success
                        || e.error != c.effect.error
                        || e.may_fail != c.effect.may_fail
                    {
                        return Err(meta_error(format!(
                            "v{}: plan incompatible with ownership.tsv for {name}",
                            v.0
                        )));
                    }
                }
            }
            // An invoke of a callee that does not fail (the joint analysis knows more
            // than that of the module that put the landing pad) only leaves a dead landing pad.
            let invoke = tables.invocations.contains_key(v);
            let pending_state = plan.pendings.contains_key(v);
            if (e.may_fail && !invoke && !pending_state)
                || (!e.may_fail && pending_state)
                || (!e.may_fail && !e.error.is_empty())
            {
                let name_str = match inst {
                    Instruction::CallRuntime { name, .. } | Instruction::CallStatic { symbol: name, .. } => name.clone(),
                    another => format!("{another:?}").chars().take(80).collect(),
                };
                return Err(meta_error(format!(
                    "v{}: contract does not match the exceptional exits {name_str} (may fail: {})",
                    v.0, e.may_fail
                )));
            }
            let mut args = HashSet::new();
            operands(inst, &mut |o| {
                if let Operand::Val(v) = o {
                    args.insert(*v);
                }
            });
            for &c in e.always.iter().chain(&e.success).chain(&e.error) {
                if !args.contains(&c) {
                    return Err(meta_error(format!(
                        "v{}: consumption of v{} is not an argument",
                        v.0, c.0
                    )));
                }
            }
        }
    }
    let mut extras: Vec<_> = plan
        .instructions
        .keys()
        .filter(|v| !expected_set.contains(v))
        .copied()
        .collect();
    extras.sort_by_key(|v| v.0);
    if let Some(v) = extras.first() {
        return Err(meta_error(format!(
            "v{}: stale contract or overwrites an ARC/Phi rule",
            v.0
        )));
    }
    if f.blocks.is_empty() {
        return Err(meta_error("function without an entry".into()));
    }
    let cfg = Cfg::new(f);
    let mut exits = HashMap::new();
    let mut entries: Vec<Option<HashSet<ValueId>>> = vec![None; f.blocks.len()];
    entries[0] = Some(
        f.params
            .iter()
            .filter(|(v, _, _)| classes[v] == Ownership::Owned)
            .map(|(v, _, _)| *v)
            .collect(),
    );
    let mut parents = vec![None; f.blocks.len()];
    let mut queue = VecDeque::from([0]);
    while let Some(i) = queue.pop_front() {
        let b = &f.blocks[i];
        let mut active_set = entries[i].clone().unwrap();
        let mut invoke = None;
        for (v, inst, _) in &b.instructions {
            let result = (|| -> Result<(), String> {
                if matches!(inst, Instruction::Phi { .. }) {
                    return check(*v, classes, &active_set);
                }
                let mut failure = None;
                operands(inst, &mut |o| {
                    if failure.is_none() {
                        failure = use_value(o, classes, &active_set).err();
                    }
                });
                if let Some(m) = failure {
                    return Err(m);
                }
                match inst {
                    Instruction::ArcCopy { .. } | Instruction::ArcLoadStrong { .. } => {
                        produce(*v, classes, &mut active_set)
                    }
                    Instruction::ArcStoreStrong { value, mode, .. } => {
                        if *mode == StrongStoreMode::Move {
                            consume_operand(value, classes, &mut active_set)
                        } else {
                            Ok(())
                        }
                    }
                    Instruction::ArcMove { value } => {
                        consume_operand(value, classes, &mut active_set)?;
                        produce(*v, classes, &mut active_set)
                    }
                    Instruction::ArcDrop { value } => {
                        consume_operand(value, classes, &mut active_set)
                    }
                    _ => {
                        let e = &plan.instructions[v];
                        apply(&e.always, classes, &mut active_set)?;
                        // An invoke (even of a callee that does not fail) only
                        // produces the result on the success edge.
                        if e.may_fail || tables.invocations.contains_key(v) {
                            invoke = Some(*v);
                            Ok(())
                        } else {
                            apply(&e.success, classes, &mut active_set)?;
                            produce(*v, classes, &mut active_set)
                        }
                    }
                }
            })();
            result.map_err(|m| flow_error(f, &parents, i, format!("v{}: {m}", v.0)))?;
        }
        let mut failure = None;
        terminator_operands(&b.terminator, &mut |o| {
            if failure.is_none() {
                failure = use_value(o, classes, &active_set).err();
            }
        });
        if let Some(m) = failure {
            return Err(flow_error(f, &parents, i, m));
        }
        if let Terminator::Return(val_ref) = &b.terminator
            && !matches!(
                tables.exits.get(&b.id),
                Some(ExceptionExit::Throws | ExceptionExit::Resume)
            )
        {
            match (plan.ret, val_ref) {
                (TokenReturn::Owned, Some(v)) => consume_operand(v, classes, &mut active_set)
                    .map_err(|m| flow_error(f, &parents, i, m))?,
                (TokenReturn::Borrowed, Some(Operand::Val(v))) if of_caller(*v, classes) => {}
                (TokenReturn::Borrowed, Some(Operand::Constant(Constant::Null))) => {}
                (TokenReturn::Trivial, None) => {}
                (TokenReturn::Trivial, Some(Operand::Val(v)))
                    if classes[v] == Ownership::Trivial => {}
                (TokenReturn::Trivial, Some(Operand::Constant(c)))
                    if !matches!(c, Constant::String(_) | Constant::StringWtf8(_)) => {}
                _ => {
                    let class = match val_ref {
                        Some(Operand::Val(v)) => format!("v{} {:?}", v.0, classes.get(v)),
                        other => format!("{other:?}"),
                    };
                    return Err(flow_error(
                        f,
                        &parents,
                        i,
                        format!("return does not satisfy the token convention {:?} ({class})", plan.ret),
                    ));
                }
            }
        }
        // `Unreachable` does not continue (the dead path after a throw,
        // the end of whoever terminates the process): a live token there does not leak.
        if cfg.successors[i].is_empty() && !active_set.is_empty() && !matches!(b.terminator, Terminator::Unreachable) {
            if allow_cleanup && matches!(b.terminator, Terminator::Return(_)) {
                let mut remaining: Vec<_> = active_set.iter().copied().collect();
                remaining.sort_unstable_by_key(|v| v.0);
                exits.insert(b.id, remaining);
                continue;
            }
            let mut ids: Vec<_> = active_set.iter().map(|v| v.0).collect();
            ids.sort_unstable();
            return Err(flow_error(
                f,
                &parents,
                i,
                format!("tokens not consumed at the exit {ids:?}"),
            ));
        }
        for &s in &cfg.successors[i] {
            let dest = &f.blocks[s];
            let next_one = (|| -> Result<HashSet<ValueId>, String> {
                let mut r = active_set.clone();
                if let Some(v) = invoke {
                    let e = &plan.instructions[&v];
                    if tables
                        .invocations
                        .get(&v)
                        .or_else(|| plan.pendings.get(&v))
                        == Some(&dest.id)
                    {
                        apply(&e.error, classes, &mut r)?;
                    } else {
                        apply(&e.success, classes, &mut r)?;
                        produce(v, classes, &mut r)?;
                    }
                }
                let mut consumed = Vec::new();
                let mut produced = Vec::new();
                for (v, inst, _) in &dest.instructions {
                    if let Instruction::Phi { incoming, .. } = inst {
                        for (of, op) in incoming {
                            if *of != b.id {
                                continue;
                            }
                            use_value(op, classes, &r)?;
                            if matches!(classes[v], Ownership::Borrowed { .. }) {
                                let compatible = match op {
                                    Operand::Constant(Constant::Null) => true,
                                    Operand::Val(entry) => {
                                        classes[entry] == Ownership::Trivial
                                            || (owner_root(*entry, classes)
                                                == owner_root(*v, classes)
                                                && owner_scopes(*entry, classes)
                                                    .is_subset(&owner_scopes(*v, classes)))
                                    }
                                    _ => false,
                                };
                                if !compatible {
                                    return Err(format!(
                                        "borrowed Phi v{}: incompatible owner or scope at the entry",
                                        v.0
                                    ));
                                }
                            }
                            if classes[v] == Ownership::Owned {
                                match op {
                                    Operand::Val(entry)
                                        if classes[entry] == Ownership::Owned =>
                                    {
                                        consumed.push(*entry)
                                    }
                                    Operand::Constant(Constant::Null) => {}
                                    _ => {
                                        return Err(format!(
                                            "Phi v{} requires an owned or null entry",
                                            v.0
                                        ));
                                    }
                                }
                                produced.push(*v);
                            }
                        }
                    }
                }
                apply(&consumed, classes, &mut r)?;
                // Phi results are born at the destination. Transferred tokens already
                // left r; neither receives a drop in the predecessor.
                // The dead ones leave before the productions: the old value of a
                // loop Phi that does not come back to it (it was copied on the edge)
                // is dead here, and the new one is born with the same ID.
                let error = invoke.is_some_and(|v| tables.invocations.get(&v)
                    .or_else(|| plan.pendings.get(&v)) == Some(&dest.id));
                if clear_edges && !error && !tables.landing_pads.contains(&dest.id) {
                    let mut used_set = live_set.entry[&dest.id].clone();
                    for (_, inst, _) in &dest.instructions {
                        if let Instruction::Phi { incoming, .. } = inst {
                            for (of, op) in incoming {
                                if *of != b.id { continue; }
                                if let Operand::Val(v) = op {
                                    let mut current = *v;
                                    let mut seen_set = HashSet::new();
                                    while seen_set.insert(current) {
                                        used_set.insert(current);
                                        if let Some(Ownership::Borrowed { owner: OwnerOrigin::Value(p), .. }) = classes.get(&current) {
                                            current = *p;
                                        } else { break; }
                                    }
                                }
                            }
                        }
                    }
                    let mut dead_set: Vec<_> = r.iter().copied()
                        .filter(|v| !used_set.contains(v) && (!produced.contains(v) || !consumed.contains(v))).collect();
                    dead_set.sort_unstable_by_key(|v| v.0);
                    for v in &dead_set { r.remove(v); }
                    if !dead_set.is_empty() { edges.insert((b.id, dest.id), dead_set); }
                }
                for &v in &produced {
                    produce(v, classes, &mut r)?;
                }
                Ok(r)
            })()
            .map_err(|m| {
                flow_error(
                    f,
                    &parents,
                    i,
                    format!("edge b{} -> b{}: {m}", b.id.0, dest.id.0),
                )
            })?;
            match &entries[s] {
                Some(previous) if *previous != next_one => {
                    let mut only_before: Vec<_> = previous.difference(&next_one).map(|v| v.0).collect();
                    let mut only_now: Vec<_> = next_one.difference(previous).map(|v| v.0).collect();
                    only_before.sort_unstable();
                    only_now.sort_unstable();
                    return Err(flow_error(
                        f,
                        &parents,
                        i,
                        format!(
                            "join b{} with different tokens (only before {only_before:?}, only now {only_now:?}); previous path {:?}",
                            dest.id.0,
                            path(f, &parents, s)
                        ),
                    ));
                }
                Some(_) => {}
                None => {
                    entries[s] = Some(next_one);
                    parents[s] = Some(i);
                    queue.push_back(s);
                }
            }
        }
    }
    let entries = f
        .blocks
        .iter()
        .zip(entries)
        .filter_map(|(b, owners)| {
            owners.map(|owners| {
                let mut owners: Vec<_> = owners.into_iter().collect();
                owners.sort_by_key(|v| v.0);
                (b.id, owners)
            })
        })
        .collect();
    super::borrows::verify(f, classes)?;
    Ok(TokenFlow { exits, entries, edges })
}

#[cfg(test)]
mod tests;
