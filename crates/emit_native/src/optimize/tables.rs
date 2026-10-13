//! Exceptions by tables (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13): the
//! pass that rewrites the already optimized HIR from the "pending state checked
//! after each call" model to the "the Dart call unwinds" model.
//!
//! **The contract of the mode.** A Dart function never returns to its caller with the
//! exception pending: it unwinds the stack (`@df.lancar`) up to the landing pad
//! (`landingpad`) of whoever handles it. The exception itself is still the
//! runtime's pending state — unwinding only transfers control —, so
//! the code of a handler (`catch`, `finally`), which reads and clears the pending state,
//! is the same in both modes. The runtime does not change protocol: an extern
//! that throws leaves the exception pending and returns, and the check after
//! it stays in the HIR.
//!
//! **What the pass does**, function by function, without touching the lowering nor the
//! other passes (which still see only the check shape):
//!
//! 1. finds each *site* — the instruction that calls Dart code that may
//!    unwind ([`is_site`]) — and decides the destination of the unwinding:
//!    * a direct call followed by the check, with a handler that only returns
//!      the default value ([`pure_exit`]): the check goes away and the call
//!      becomes an ordinary call, without a landing pad — the unwinding crosses the
//!      frame (the landing pad of the handler restores the top of the shadow stack);
//!    * a direct call followed by the check, with a real handler: the
//!      check goes away, the call becomes an `invoke` and the landing pad (a new block)
//!      branches to the handler;
//!    * a call that may also return with the exception pending (closure,
//!      selector: the invalid entry and the `noSuchMethod` one belong to the runtime)
//!      with a handler that only returns the default value: nothing changes (the check
//!      catches the pending state; the unwinding crosses the frame);
//!    * everything else: **exact emulation** — the call becomes an `invoke` and the landing pad
//!      continues at the point after the call, with the default value as the
//!      result and the exception pending, exactly what the call returned
//!      in the check model;
//! 2. computes, by data flow, whether the exception is pending at each
//!    `Return` ([`Pending`]): clear, return; set, unwind
//!    ([`ExceptionExit::Throws`]); maybe, check and decide
//!    ([`ExceptionExit::Guard`]).
//!
//! The result ([`FunctionTables`], in [`Module::tables`]) is what the
//! emitter reads (`llvm/mod.rs`). A call with a landing pad is the last instruction of
//! its block; the terminator of the block is
//! `CondBranch { cond: false, then: landing_pad, else: continuation }`, which keeps
//! the landing pad in the flow graph (liveness of the roots, reachable blocks) and
//! which the emitter writes as the branch to the continuation.

//! Under ARC, the shortcuts that let unwinding cross a frame without a landing pad
//! are disabled: the local edge is needed to insert cleanup.
//! The pass exposes that edge; it does not insert the owner cleanup yet.

use super::cfg::successors;
use super::effects::{is_check, instruction_throws, do_not_throw};
use super::operands::{default_constant, largest_ids, operands, terminator_operands, replace};
use super::simplify::remove_unreachable;
use crate::hir::*;
use crate::llvm::externs::effects_of;
use std::collections::{HashMap, HashSet};

/// Rewrites the module for exceptions by tables and writes the decision of each
/// function into [`Module::tables`]. Runs after [`super::optimize`] (and
/// also with the passes turned off: it only depends on the shape the lowering gives).
pub fn apply(module: &mut Module) {
    let plan = prepare(module);
    materialize(module, plan);
}

/// Inventory of the prepared sites, still without decisions about returns.
/// Between the steps, preserve the IDs and the order of the functions; CFG
/// transformations must update this inventory before materializing tables.
pub(super) struct ExceptionPlan {
    functions: Vec<(String, FunctionTables)>,
}

impl ExceptionPlan {
    /// Queries the sites before publishing tables, checking the identity
    /// of the function. The intermediate passes read the plan without enabling LLVM.
    pub(super) fn function_sites(&self, index: usize, f: &Function) -> &FunctionTables {
        let (symbol, sites) = self.functions.get(index)
            .expect("bug do compilador: função ausente do plano excepcional");
        assert_eq!(&f.symbol, symbol, "bug do compilador: ordem das funções mudou depois da preparação excepcional");
        sites
    }
}

/// Exposes landing pads and continuations in the CFG without publishing tables to the emitter.
pub(super) fn prepare(module: &mut Module) -> ExceptionPlan {
    assert!(!module.excecoes_por_tabelas, "bug do compilador: CFG excepcional já materializado");
    let nl = do_not_throw(module);
    // Without a proof of absence of owners, ARC needs a local exit
    // to insert cleanup even when there is no catch/finally in the source.
    let preserve_cleanup = module.arc_memory;
    // Published ARC preparation: closures and selectors go through the uniform
    // invocation of the runtime, whose ports return the pending exception; they keep
    // the check, without an invoke.
    let uniforms = !(module.arc_memory && crate::alvo::arc_prepared().unwrap_or(false));
    let mut functions = Vec::with_capacity(module.functions.len());
    for f in &mut module.functions {
        let sites = if super::valid(f) { from_function(f, &nl, preserve_cleanup, uniforms) } else { FunctionTables::default() };
        functions.push((f.symbol.clone(), sites));
    }
    ExceptionPlan { functions }
}

/// Rebuilds exit decisions over the final HIR and validates the sites before
/// publishing the tables. Does not run generic simplification on the landing pads.
pub(super) fn materialize(module: &mut Module, plan: ExceptionPlan) {
    assert_eq!(plan.functions.len(), module.functions.len(), "bug do compilador: funções mudaram depois da preparação excepcional");
    let nl = do_not_throw(module);
    let mut tables = Vec::with_capacity(module.functions.len());
    for (index, f) in module.functions.iter().enumerate() {
        let t = plan.function_sites(index, f);
        if super::valid(f) {
            check(f, t);
        }
    }
    for (f, (symbol, mut t)) in module.functions.iter_mut().zip(plan.functions) {
        assert_eq!(f.symbol, symbol, "bug do compilador: ordem das funções mudou depois da preparação excepcional");
        if super::valid(f) {
            exits(f, &nl, &mut t);
        }
        if let Some(c) = &t.foreign_cleanup {
            c.check(f, &t).expect("stale foreign cleanup in the materialization");
        }
        tables.push(t);
    }
    module.tables = tables;
    module.excecoes_por_tabelas = true;
}

/// Does the instruction call Dart code that may unwind?
///
/// * `CallStatic` of a function that may throw. Left out are those that do not
///   throw (`effects::do_not_throw`), the runtime symbols (`dartforge_…`,
///   which follow the pending protocol) and the getter of a type
///   (`df.rti.…`, `lower/rti.rs`), which the lowering calls without a check: if it
///   throws, the unwinding crosses the frame;
/// * the call of the typed body of a closure, that of a function value and the one by
///   selector.
fn is_site(inst: &Instruction, nl: &HashSet<String>, uniforms: bool) -> bool {
    match inst {
        Instruction::CallStatic { symbol, .. } => {
            !nl.contains(symbol) && !symbol.starts_with("df.rti.") && !symbol.starts_with("dartforge_")
        }
        Instruction::TypedCall { .. } => true,
        Instruction::CallClosure { .. }
        | Instruction::CallClosureForward { .. }
        | Instruction::CallSelector { .. }
        | Instruction::CallSelectorForward { .. } => uniforms,
        _ => false,
    }
}

/// Does the site only leave by unwinding (the target is always Dart code)? A
/// closure call and a call by selector may also return with the exception
/// pending (`@df_clo_invalido`, the `noSuchMethod` entry).
fn only_unwinds(inst: &Instruction) -> bool {
    matches!(inst, Instruction::CallStatic { .. } | Instruction::TypedCall { .. })
}

/// The shape that `FnBuilder::emit_call_with_check` gives a site at position
/// `i`: `[call][p = pending][c = p != 0]` at the end of the block and
/// `CondBranch(c, handler, continuation)`.
struct Check {
    p: ValueId,
    c: ValueId,
    handler: BlockId,
    continuation: BlockId,
}

fn site_check(b: &BasicBlock, i: usize) -> Option<Check> {
    if b.instructions.len() != i + 3 {
        return None;
    }
    let (p, inst_p, _) = &b.instructions[i + 1];
    if !is_check(inst_p) {
        return None;
    }
    let (c, inst_c, _) = &b.instructions[i + 2];
    let Instruction::ICmp(ICmpOp::Ne, Operand::Val(read_done), Operand::Constant(Constant::Int(0))) = inst_c else {
        return None;
    };
    if read_done != p {
        return None;
    }
    let Terminator::CondBranch { cond: Operand::Val(cond), then_block, else_block } = &b.terminator else {
        return None;
    };
    if cond != c || then_block == else_block {
        return None;
    }
    Some(Check { p: *p, c: *c, handler: *then_block, continuation: *else_block })
}

/// Does the handler only return the default value (the exception exit of a function
/// without a surrounding `catch` or `finally`)? Blocks without instructions besides `phi`,
/// chained by unconditional branches up to a `Return`. A join block
/// that only rechecks the pending state (`phi`, `p = pending`, `c = p != 0`,
/// `CondBranch(c, …)`; the shape the uniform entry `$ent` gives the join of the
/// normal path with the exception one) is crossed on the exception side:
/// coming from the handler, the pending state is set and the recheck always goes
/// there. Without this, every `$ent` got a landing pad that catches everything and returns the
/// pending state, and the dynamic call in tables mode cost more than in the
/// check mode.
fn pure_exit(f: &Function, pos: &HashMap<BlockId, usize>, de: BlockId) -> bool {
    let mut current = de;
    for _ in 0..64 {
        let Some(&k) = pos.get(&current) else { return false };
        let b = &f.blocks[k];
        let rest: Vec<&(ValueId, Instruction, Type)> = b.instructions.iter().filter(|(_, i, _)| !matches!(i, Instruction::Phi { .. })).collect();
        match (&rest[..], &b.terminator) {
            ([], Terminator::Return(_)) => return true,
            ([], Terminator::Branch(n)) => current = *n,
            ([(p, ip, _), (c, ic, _)], Terminator::CondBranch { cond: Operand::Val(cond), then_block, .. })
                if is_check(ip)
                    && cond == c
                    && matches!(ic, Instruction::ICmp(ICmpOp::Ne, Operand::Val(l), Operand::Constant(Constant::Int(0))) if l == p) =>
            {
                current = *then_block;
            }
            _ => return false,
        }
    }
    false
}

/// How many times each value is read (operands, `phi` entries,
/// terminators).
fn count_uses(f: &Function) -> HashMap<ValueId, u32> {
    let mut uses: HashMap<ValueId, u32> = HashMap::new();
    {
        let mut counts = |o: &Operand| {
            if let Operand::Val(v) = o {
                *uses.entry(*v).or_insert(0) += 1;
            }
        };
        for b in &f.blocks {
            for (_, inst, _) in &b.instructions {
                operands(inst, &mut counts);
            }
            terminator_operands(&b.terminator, &mut counts);
        }
    }
    uses
}

fn minus(uses: &mut HashMap<ValueId, u32>, v: ValueId) {
    if let Some(n) = uses.get_mut(&v) {
        *n = n.saturating_sub(1);
    }
}

fn unused(uses: &HashMap<ValueId, u32>, v: ValueId) -> bool {
    uses.get(&v).copied().unwrap_or(0) == 0
}

/// Removes the comparison and the read of the pending state that were left without a reader.
fn remove_check(b: &mut BasicBlock, uses: &mut HashMap<ValueId, u32>, cf: &Check) {
    if !unused(uses, cf.c) {
        return;
    }
    b.instructions.retain(|(v, _, _)| *v != cf.c);
    minus(uses, cf.p);
    if unused(uses, cf.p) {
        b.instructions.retain(|(v, _, _)| *v != cf.p);
    }
}

fn false_operand() -> Operand {
    Operand::Constant(Constant::Bool(false))
}

fn from_function(f: &mut Function, nl: &HashSet<String>, preserve_cleanup: bool, uniforms: bool) -> FunctionTables {
    let mut t = FunctionTables::default();
    t.checks_stack = f.blocks.iter().any(|b| b.instructions.iter().any(|(_, inst, _)| is_site(inst, nl, uniforms)));
    let (mut next_v, mut next_b) = largest_ids(f);
    let mut uses = count_uses(f);
    let mut pos: HashMap<BlockId, usize> = f.blocks.iter().enumerate().map(|(k, b)| (b.id, k)).collect();
    // One site per iteration: the first of the block. Either it is the last of the block (the
    // checked shape), or the rest of the block goes to a new block, at the end
    // of the list, visited later.
    let mut bi = 0;
    while bi < f.blocks.len() {
        let Some(i) = f.blocks[bi].instructions.iter().position(|(_, inst, _)| is_site(inst, nl, uniforms)) else {
            bi += 1;
            continue;
        };
        let origin = f.blocks[bi].id;
        let (result, typ, direct) = {
            let (v, inst, ty) = &f.blocks[bi].instructions[i];
            (*v, crate::llvm::LlvmEmitter::result_type(inst, *ty), only_unwinds(inst))
        };
        let checked = site_check(&f.blocks[bi], i);
        let pure = !preserve_cleanup && checked.as_ref().is_some_and(|cf| pure_exit(f, &pos, cf.handler));
        match checked {
            // The check goes away: the call unwinds and nobody here handles it.
            Some(cf) if direct && pure => {
                f.blocks[bi].terminator = Terminator::Branch(cf.continuation);
                minus(&mut uses, cf.c);
                if let Some(&kt) = pos.get(&cf.handler) {
                    for (_, inst, _) in &mut f.blocks[kt].instructions {
                        if let Instruction::Phi { incoming, .. } = inst {
                            incoming.retain(|(o, op)| {
                                if *o != origin {
                                    return true;
                                }
                                if let Operand::Val(v) = op {
                                    minus(&mut uses, *v);
                                }
                                false
                            });
                        }
                    }
                }
                remove_check(&mut f.blocks[bi], &mut uses, &cf);
            }
            // The check goes away: the call is an `invoke` and the landing pad goes to the
            // handler, with the exception pending — as the check did.
            Some(cf) if direct => {
                next_b += 1;
                let landing_pad = BlockId(next_b);
                f.blocks[bi].terminator =
                    Terminator::CondBranch { cond: false_operand(), then_block: landing_pad, else_block: cf.continuation };
                minus(&mut uses, cf.c);
                if let Some(&kt) = pos.get(&cf.handler) {
                    for (_, inst, _) in &mut f.blocks[kt].instructions {
                        let Instruction::Phi { incoming, ty } = inst else { continue };
                        for (o, op) in incoming.iter_mut() {
                            if *o != origin {
                                continue;
                            }
                            *o = landing_pad;
                            let read_done = match &*op {
                                Operand::Val(v) => Some(*v),
                                Operand::Constant(_) => None,
                            };
                            // In the landing pad neither the result of the call
                            // nor the check exist: the values they would have
                            // (the default; pending; true).
                            if read_done == Some(result) {
                                *op = default_constant(*ty);
                            } else if read_done == Some(cf.p) {
                                minus(&mut uses, cf.p);
                                *op = Operand::Constant(Constant::Int(1));
                            } else if read_done == Some(cf.c) {
                                minus(&mut uses, cf.c);
                                *op = Operand::Constant(Constant::Bool(true));
                            }
                        }
                    }
                }
                pos.insert(landing_pad, f.blocks.len());
                f.blocks.push(BasicBlock {
                    id: landing_pad,
                    instructions: Vec::new(),
                    terminator: Terminator::Branch(cf.handler),
                });
                remove_check(&mut f.blocks[bi], &mut uses, &cf);
                t.invocations.insert(result, landing_pad);
                t.landing_pads.insert(landing_pad);
            }
            // May return pending and the handler only returns the default: the
            // check stays and the unwinding crosses the frame.
            Some(_) if pure => {}
            // Exact emulation: the landing pad continues after the call, with the
            // default value and the exception pending.
            _ => {
                next_b += 2;
                let landing_pad = BlockId(next_b - 1);
                let rest = BlockId(next_b);
                let tail = f.blocks[bi].instructions.split_off(i + 1);
                let terminator = std::mem::replace(
                    &mut f.blocks[bi].terminator,
                    Terminator::CondBranch { cond: false_operand(), then_block: landing_pad, else_block: rest },
                );
                // The successors of the block become successors of the rest.
                for s in successors(&terminator) {
                    let Some(&k) = pos.get(&s) else { continue };
                    for (_, inst, _) in &mut f.blocks[k].instructions {
                        if let Instruction::Phi { incoming, .. } = inst {
                            for (o, _) in incoming.iter_mut() {
                                if *o == origin {
                                    *o = rest;
                                }
                            }
                        }
                    }
                }
                pos.insert(landing_pad, f.blocks.len());
                f.blocks.push(BasicBlock { id: landing_pad, instructions: Vec::new(), terminator: Terminator::Branch(rest) });
                pos.insert(rest, f.blocks.len());
                f.blocks.push(BasicBlock { id: rest, instructions: tail, terminator: terminator });
                if typ != Type::Void {
                    // The result, from here on, is that of the call or the
                    // default (what the throwing function returned).
                    next_v += 1;
                    let new = ValueId(next_v);
                    replace(f, &|v| (v == result).then_some(Operand::Val(new)));
                    let k = pos[&rest];
                    f.blocks[k].instructions.insert(
                        0,
                        (
                            new,
                            Instruction::Phi {
                                incoming: vec![(origin, Operand::Val(result)), (landing_pad, default_constant(typ))],
                                ty: typ,
                            },
                            typ,
                        ),
                    );
                }
                t.invocations.insert(result, landing_pad);
                t.landing_pads.insert(landing_pad);
            }
        }
        bi += 1;
    }
    // The handlers that were left without anyone reaching them.
    remove_unreachable(f);
    let live_set: HashSet<BlockId> = f.blocks.iter().map(|b| b.id).collect();
    t.landing_pads.retain(|b| live_set.contains(b));
    t.invocations.retain(|_, b| live_set.contains(&*b));
    check(f, &t);
    t
}

/// The invariants the emitter assumes, checked in every function (a defect
/// here would become IR that LLVM rejects, or a landing pad reached by an ordinary
/// branch):
///
/// * the block with an `invoke` has only one, and ends in
///   `CondBranch { cond: false, then: its landing_pad, else: continuation }`;
/// * no other branch leads to a landing pad, and each landing pad has exactly one
///   `invoke`;
/// * the landing pad has no instructions and branches unconditionally.
fn check(f: &Function, t: &FunctionTables) {
    super::arc::check_resumes(f, t).expect("bug do compilador: Retoma inválida");
    let mut arrivals: HashMap<BlockId, u32> = HashMap::new();
    for b in &f.blocks {
        let invoked: Vec<BlockId> = b.instructions.iter().filter_map(|(v, _, _)| t.invocations.get(v).copied()).collect();
        let branch = match &b.terminator {
            Terminator::CondBranch { cond: Operand::Constant(Constant::Bool(false)), then_block, else_block }
                if t.landing_pads.contains(then_block) && then_block != else_block =>
            {
                Some(*then_block)
            }
            _ => None,
        };
        let coherent = match (invoked.as_slice(), branch) {
            ([], None) => true,
            ([p], Some(d)) => *p == d,
            _ => false,
        };
        assert!(
            coherent,
            "bug do compilador (exceções por tabelas): em {}, o bloco b{} não casa o `invoke` com o pouso",
            f.symbol,
            b.id.0
        );
        if branch.is_some() {
            assert!(
                b.instructions.last().is_some_and(|(v, _, _)| t.invocations.contains_key(v)),
                "bug do compilador (exceções por tabelas): em {}, o bloco b{} tem instrução depois do `invoke`",
                f.symbol,
                b.id.0
            );
        }
        for s in successors(&b.terminator) {
            if !t.landing_pads.contains(&s) {
                continue;
            }
            assert!(
                branch == Some(s),
                "bug do compilador (exceções por tabelas): em {}, o bloco b{} desvia ao pouso b{} sem `invoke`",
                f.symbol,
                b.id.0,
                s.0
            );
            *arrivals.entry(s).or_insert(0) += 1;
        }
        if t.landing_pads.contains(&b.id) {
            assert!(
                (b.instructions.is_empty() && matches!(b.terminator, Terminator::Branch(_)))
                    || (t.exits.get(&b.id) == Some(&ExceptionExit::Resume)
                        && b.instructions.iter().all(|(_, i, _)| super::arc::resume_instruction(i))
                        && matches!(b.terminator, Terminator::Return(_))),
                "bug do compilador (exceções por tabelas): em {}, o pouso b{} não é só um desvio",
                f.symbol,
                b.id.0
            );
        }
    }
    for p in &t.landing_pads {
        assert!(
            arrivals.get(p) == Some(&1),
            "bug do compilador (exceções por tabelas): em {}, o pouso b{} não tem exatamente um `invoke`",
            f.symbol,
            p.0
        );
    }
}

/// Is the exception pending at this point?
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pending {
    Cleans,
    Linked,
    Maybe,
}

/// What an instruction does with the pending state.
enum Effect {
    Nada,
    /// May leave the exception pending (and never clears it).
    MayLink,
    Cleans,
    Links,
}

fn effect(inst: &Instruction, nl: &HashSet<String>) -> Effect {
    match inst {
        Instruction::CallRuntime { name, .. } => match name.as_str() {
            // The emitter unwinds right after a clear that did not clear (the
            // isolate's unwinding): on the normal path, it is clear.
            "dartforge_exception_clear" => Effect::Cleans,
            "dartforge_exception_throw" => Effect::Links,
            "dartforge_exception_pending"
            | "dartforge_exception_peek_ref"
            | "dartforge_exception_capturavel"
            | "dartforge_stack_trace_get" => Effect::Nada,
            _ => {
                let e = effects_of(name);
                if e.throws || e.calls_dart { Effect::MayLink } else { Effect::Nada }
            }
        },
        // A Dart function that returns returns without a pending exception (one that
        // throws unwinds); a runtime symbol follows the pending state.
        Instruction::CallStatic { symbol, .. } => {
            if symbol.starts_with("dartforge_") && !nl.contains(symbol) { Effect::MayLink } else { Effect::Nada }
        }
        Instruction::TypedCall { .. } => Effect::Nada,
        // Emitted as LLVM instructions, without a call: they do not touch the
        // pending state (`effects::instruction_throws` counts them through their surroundings).
        Instruction::SDiv(..) | Instruction::SRem(..) | Instruction::DoubleToInt(_) | Instruction::CheckNotNull(_) => {
            Effect::Nada
        }
        another => {
            if instruction_throws(another, nl) { Effect::MayLink } else { Effect::Nada }
        }
    }
}

fn after(e: Pending, ef: &Effect) -> Pending {
    match ef {
        Effect::Nada => e,
        Effect::Cleans => Pending::Cleans,
        Effect::Links => Pending::Linked,
        Effect::MayLink => {
            if e == Pending::Linked { Pending::Linked } else { Pending::Maybe }
        }
    }
}

/// The pending state at the end of the block, given that at the entry, and — when the block
/// ends by branching on a check that nothing after it changes — the
/// successors `(with the exception pending, without it)`.
fn walk(b: &BasicBlock, entry: Pending, nl: &HashSet<String>) -> (Pending, Option<(BlockId, BlockId)>) {
    let mut e = entry;
    let mut check: Option<ValueId> = None;
    for (v, inst, _) in &b.instructions {
        if is_check(inst) {
            check = Some(*v);
            continue;
        }
        let ef = effect(inst, nl);
        if !matches!(ef, Effect::Nada) {
            check = None;
        }
        e = after(e, &ef);
    }
    let branch = match (&b.terminator, check) {
        (Terminator::CondBranch { cond: Operand::Val(c), then_block, else_block }, Some(p)) if then_block != else_block => {
            b.instructions.iter().find(|(v, _, _)| v == c).and_then(|(_, inst, _)| match inst {
                Instruction::ICmp(ICmpOp::Ne, Operand::Val(x), Operand::Constant(Constant::Int(0))) if *x == p => {
                    Some((*then_block, *else_block))
                }
                Instruction::ICmp(ICmpOp::Eq, Operand::Val(x), Operand::Constant(Constant::Int(0))) if *x == p => {
                    Some((*else_block, *then_block))
                }
                _ => None,
            })
        }
        _ => None,
    };
    (e, branch)
}

fn join(a: Option<Pending>, b: Pending) -> Pending {
    match a {
        None => b,
        Some(x) if x == b => b,
        Some(_) => Pending::Maybe,
    }
}

/// How each `Return` exits ([`FunctionTables::exits`]): the fixpoint of the
/// pending state at the entry of each block. The function starts without a pending exception
/// (callers only call with it clear) and a landing pad starts with it set (the
/// unwinding only happens with the exception pending).
fn exits(f: &mut Function, nl: &HashSet<String>, t: &mut FunctionTables) {
    let n = f.blocks.len();
    if n == 0 {
        return;
    }
    let pos: HashMap<BlockId, usize> = f.blocks.iter().enumerate().map(|(k, b)| (b.id, k)).collect();
    let mut entry: Vec<Option<Pending>> = vec![None; n];
    entry[0] = Some(Pending::Cleans);
    for (k, b) in f.blocks.iter().enumerate() {
        if t.landing_pads.contains(&b.id) {
            entry[k] = Some(Pending::Linked);
        }
    }
    let mut queue: Vec<usize> = (0..n).filter(|k| entry[*k].is_some()).collect();
    while let Some(k) = queue.pop() {
        let Some(e) = entry[k] else { continue };
        let b = &f.blocks[k];
        let (exit, branch) = walk(b, e, nl);
        let edges: Vec<(BlockId, Pending)> = match branch {
            Some((pending, clean)) => vec![(pending, Pending::Linked), (clean, Pending::Cleans)],
            None => successors(&b.terminator).into_iter().map(|s| (s, exit)).collect(),
        };
        for (alvo, state) in edges {
            // The landing pad is only reached by unwinding: always set.
            if t.landing_pads.contains(&alvo) {
                continue;
            }
            let Some(&j) = pos.get(&alvo) else { continue };
            let new = join(entry[j], state);
            if entry[j] != Some(new) {
                entry[j] = Some(new);
                queue.push(j);
            }
        }
    }
    for (k, b) in f.blocks.iter().enumerate() {
        let Some(e) = entry[k] else { continue };
        if !matches!(b.terminator, Terminator::Return(_)) {
            continue;
        }
        if t.exits.get(&b.id) == Some(&ExceptionExit::Resume) { continue; }
        match walk(b, e, nl).0 {
            Pending::Cleans => {}
            Pending::Linked => {
                t.exits.insert(b.id, ExceptionExit::Throws);
            }
            Pending::Maybe => {
                t.exits.insert(b.id, ExceptionExit::Guard);
            }
        }
    }
    // Clearing the pending state where it is provably clear does
    // nothing: it goes away (the lowering puts one before each `return` and each jump, and
    // in the check model only a function that never throws got rid of
    // them). Removing it does not change the pending state anywhere, so the exits
    // computed above stay valid.
    let mut without_effect: HashSet<ValueId> = HashSet::new();
    for (k, b) in f.blocks.iter().enumerate() {
        let Some(mut e) = entry[k] else { continue };
        for (v, inst, _) in &b.instructions {
            if is_check(inst) {
                continue;
            }
            let ef = effect(inst, nl);
            if matches!(ef, Effect::Cleans) && e == Pending::Cleans {
                without_effect.insert(*v);
            }
            e = after(e, &ef);
        }
    }
    if !without_effect.is_empty() {
        for b in &mut f.blocks {
            b.instructions.retain(|(v, _, _)| !without_effect.contains(v));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materialization_preserves_resume_and_checks_cleanup_before_publishing() {
        let mut m = module();
        m.functions[0].blocks[0].terminator = Terminator::CondBranch {
            cond: Operand::Constant(Constant::Bool(false)), then_block: BlockId(2), else_block: BlockId(1),
        };
        m.functions[0].blocks.extend([
            BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) },
            BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
        ]);
        let t = FunctionTables {
            invocations: [(ValueId(0), BlockId(2))].into(), landing_pads: [BlockId(2)].into(),
            exits: [(BlockId(2), ExceptionExit::Resume)].into(), ..Default::default()
        };
        let plan = ExceptionPlan { functions: vec![(m.functions[0].symbol.clone(), t.clone())] };
        materialize(&mut m, plan);
        assert_eq!(m.tables[0].exits[&BlockId(2)], ExceptionExit::Resume);
        m.functions[0].blocks[2].terminator = Terminator::Return(Some(Operand::Val(ValueId(0))));
        assert!(std::panic::catch_unwind(|| check(&m.functions[0], &t)).is_err());
    }

    fn module() -> Module {
        Module {
            functions: vec![Function {
                symbol: "chamador".into(),
                name: "chamador".into(),
                debug_info: None,
                params: Vec::new(),
                return_ty: Type::Ref,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(0),
                        Instruction::CallStatic { symbol: "pode_lancar".into(), args: Vec::new(), ret_ty: Type::Ref },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
                }],
            }],
            ..Default::default()
        }
    }

    #[test]
    fn prepares_edges_without_publishing_tables() {
        let mut m = module();
        let plan = prepare(&mut m);
        assert!(!m.excecoes_por_tabelas && m.tables.is_empty());
        let t = plan.function_sites(0, &m.functions[0]);
        let landing_pad = t.invocations[&ValueId(0)];
        assert!(t.exits.is_empty());
        assert!(matches!(m.functions[0].blocks[0].terminator,
            Terminator::CondBranch { then_block, .. } if then_block == landing_pad));
        let ret = m.functions[0].blocks.iter().find(|b| matches!(b.terminator, Terminator::Return(_))).unwrap().id;
        materialize(&mut m, plan);
        assert_eq!(m.tables[0].exits[&ret], ExceptionExit::Guard);
    }

    #[test]
    fn arc_exposes_local_unwind_even_without_source_handler() {
        for direct in [true, false] {
            for arc in [false, true] {
                let mut m = module();
                m.arc_memory = arc;
                let f = &mut m.functions[0];
                if !direct {
                    f.params.push((ValueId(3), "alvo".into(), Type::Ref));
                    f.blocks[0].instructions[0].1 = Instruction::CallClosure {
                        closure: Operand::Val(ValueId(3)), args: vec![], names: vec![], ret_ty: Type::Ref,
                        tuple_types: Operand::Constant(Constant::Null),
                    };
                }
                f.blocks[0].instructions.extend([
                    (ValueId(1), Instruction::CallRuntime {
                        name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8,
                    }, Type::I8),
                    (ValueId(2), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(1)), Operand::Constant(Constant::Int(0))), Type::I1),
                ]);
                f.blocks[0].terminator = Terminator::CondBranch {
                    cond: Operand::Val(ValueId(2)), then_block: BlockId(1), else_block: BlockId(2),
                };
                f.blocks.extend([
                    BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
                    BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) },
                ]);
                assert!(super::super::valid(f));
                let plan = prepare(&mut m);
                let t = plan.function_sites(0, &m.functions[0]);
                assert_eq!(t.invocations.contains_key(&ValueId(0)), arc);
                if arc {
                    let landing_pad = t.invocations[&ValueId(0)];
                    assert!(t.landing_pads.contains(&landing_pad));
                    assert!(super::super::valid(&m.functions[0]));
                }
                materialize(&mut m, plan);
                if arc { assert!(m.tables[0].exits.contains_key(&BlockId(1))); }
            }
        }
    }

    #[test]
    #[should_panic(expected = "ordem das funções mudou")]
    fn query_rejects_swapped_function_before_materialization() {
        let mut m = module();
        let plan = prepare(&mut m);
        m.functions[0].symbol = "outra_funcao".into();
        plan.function_sites(0, &m.functions[0]);
    }

    #[test]
    fn materialization_recomputes_pending_after_changing_continuation() {
        let mut m = module();
        let plan = prepare(&mut m);
        let ret = m.functions[0].blocks.iter_mut().find(|b| matches!(b.terminator, Terminator::Return(_))).unwrap();
        ret.instructions.push((ValueId(100), Instruction::CallRuntime {
            name: "dartforge_exception_clear".into(), args: Vec::new(), ret_ty: Type::Void,
        }, Type::Void));
        materialize(&mut m, plan);
        assert!(m.tables[0].exits.is_empty(), "the continuation now clears the pending flag on both paths");
    }

    #[test]
    #[should_panic(expected = "tem instrução depois do `invoke`")]
    fn rejects_site_that_stopped_terminating_block() {
        let mut m = module();
        let plan = prepare(&mut m);
        m.functions[0].blocks[0].instructions.push((ValueId(100), Instruction::Alloca(Type::Ref), Type::Ptr));
        materialize(&mut m, plan);
    }
}
