//! Placement of the collector roots (G1, docs/NATIVO-PLANO.md §6.5) by
//! liveness: only a `Ref` value live at a collection point needs to be in the
//! roots frame, and two values that are never live together share the
//! same slot (Julia's root placement pass does the same).
//!
//! * A **collection point** is every instruction that may allocate (a Dart call, a call to
//!   an allocating runtime function, a box, a boxing conversion, a string constant…),
//!   decided by the emitter (`may_collect`), plus the `throw` and the end of a
//!   block that converts the entry of a `phi` (the conversion may box).
//! * **Rooted** is a value live at the ENTRY of a collection point —
//!   its operands included: the runtime relies on the caller to keep
//!   the arguments alive during the call (`dartforge_string_concat` allocates
//!   the result while reading both strings).
//! * Two rooted values **interfere** when one is live at the
//!   definition of the other: the `store` of the definition overwrites the slot, and the
//!   live value would lose its root. Parameters interfere with each other and with what
//!   is live at the entry; the `phi`s of a block, with each other and with what is live
//!   at its start.
//! * The `alloca`s of type `Ref` (mutable locals, G2) keep their own
//!   slot: the slot follows each write.
//!
//! A slot keeps, between uses, a value that already died — still a valid handle,
//! because it was rooted, and the collector would mark it (a loop variable kept the
//! whole tree of the previous iteration alive while the next one was built). So the
//! slot that may hold such a value and that no live value occupies is zeroed before
//! the next collection point ([`Roots::clear_at`]).

use crate::hir::*;
use crate::lower::async_sm::{successors, uses_of, terminator_uses};
use std::collections::HashMap;

/// A set of `Ref` values (dense indices).
#[derive(Clone, PartialEq, Eq)]
struct Set(Vec<u64>);

impl Set {
    fn empty(n: usize) -> Self {
        Set(vec![0; n.div_ceil(64)])
    }
    fn has(&self, i: usize) -> bool {
        self.0[i / 64] >> (i % 64) & 1 == 1
    }
    fn by(&mut self, i: usize) {
        self.0[i / 64] |= 1 << (i % 64);
    }
    fn take_out(&mut self, i: usize) {
        self.0[i / 64] &= !(1 << (i % 64));
    }
    fn join_set(&mut self, o: &Set) {
        for (a, b) in self.0.iter_mut().zip(&o.0) {
            *a |= b;
        }
    }
    fn elements(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.iter().enumerate().flat_map(|(k, &w)| {
            (0..64).filter(move |b| w >> b & 1 == 1).map(move |b| k * 64 + b)
        })
    }
}

/// The result of the roots analysis of a function.
#[derive(Default)]
pub struct Roots {
    /// The slots of the frame: `Ref` `alloca`s with their own slot, then the
    /// rooted SSA values, with a shared slot when they do not interfere.
    pub slots: HashMap<ValueId, usize>,
    /// The rooted SSA values (live at the entry of some collection
    /// point), in definition order.
    pub rooted_set: Vec<ValueId>,
    /// For each instruction that is a collection point, the `Ref` SSA values live
    /// at its entry (operands included: the runtime relies on the caller
    /// to keep them alive during the call).
    pub live_at: HashMap<ValueId, Vec<ValueId>>,
    /// The same for the collection point at the end of a block (the `throw`, the
    /// conversions of `phi` entries).
    pub live_at_end: HashMap<BlockId, Vec<ValueId>>,
    /// The rooted values live at the entry of each block (after its
    /// `phi`s); only the blocks that have any.
    pub live_at_entry_set: HashMap<BlockId, Vec<ValueId>>,
    /// The shared slots to zero before each collection point: the slot may
    /// still hold a value that died since it was written, and no value live at
    /// the point is in it. Without the zeroing, the collector would mark the dead
    /// value (a whole tree that a loop variable held the previous iteration)
    /// until the slot was reused or the frame closed.
    pub clear_at: HashMap<ValueId, Vec<usize>>,
    /// The same for the collection point at the end of a block.
    pub clear_at_end: HashMap<BlockId, Vec<usize>>,
}

/// The full analysis: the slots (the shadow stack) and the liveness at each
/// collection point (roots by maps,
/// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.8).
pub fn analyze(
    func: &Function,
    types: &HashMap<ValueId, Type>,
    may_collect: &dyn Fn(&Instruction) -> bool,
    phi_converting_blocks: &dyn Fn(BlockId) -> bool,
) -> Roots {
    analyze_with(func, types, may_collect, phi_converting_blocks, &|_| false)
}

/// [`analyze`] with the call rooting its own arguments:
/// `callee_roots(i)` says that collection point `i` is a call to a
/// Dart function with its own roots frame, without an allocating conversion between
/// the operands and the call. The callee puts in its frame the
/// parameter it still uses after one of its collection points; what only
/// lives until the call needs no root in the caller (the collector does not move
/// objects). There only what lives AFTER the call stays rooted. The runtime
/// still relies on the caller ([`analyze`]): its functions read the
/// arguments after allocating.
pub fn analyze_with(
    func: &Function,
    types: &HashMap<ValueId, Type>,
    may_collect: &dyn Fn(&Instruction) -> bool,
    phi_converting_blocks: &dyn Fn(BlockId) -> bool,
    callee_roots: &dyn Fn(&Instruction) -> bool,
) -> Roots {
    let mut slots = HashMap::new();
    for block in &func.blocks {
        for (vid, inst, _) in &block.instructions {
            if matches!(inst, Instruction::Alloca(Type::Ref)) {
                let n = slots.len();
                slots.insert(*vid, n);
            }
        }
    }
    let base = slots.len();

    // The candidate `Ref` SSA values, in definition order (the IR comes out
    // deterministic): parameters, then the instructions.
    let mut candidates: Vec<ValueId> = Vec::new();
    for (vid, _, ty) in &func.params {
        if *ty == Type::Ref {
            candidates.push(*vid);
        }
    }
    for block in &func.blocks {
        for (vid, inst, _) in &block.instructions {
            if types.get(vid) == Some(&Type::Ref) && !matches!(inst, Instruction::Const(Constant::Null) | Instruction::Alloca(_)) {
                candidates.push(*vid);
            }
        }
    }
    if candidates.is_empty() {
        return Roots { slots, ..Roots::default() };
    }
    let index: HashMap<ValueId, usize> = candidates.iter().enumerate().map(|(i, v)| (*v, i)).collect();
    let n = candidates.len();
    let idx = |v: &ValueId| index.get(v).copied();

    let position: HashMap<BlockId, usize> = func.blocks.iter().enumerate().map(|(i, b)| (b.id, i)).collect();
    // What each block reads from each predecessor through its `phi`s.
    let mut phi_uses: HashMap<(BlockId, BlockId), Vec<usize>> = HashMap::new();
    let mut phi_defs: Vec<Vec<usize>> = vec![Vec::new(); func.blocks.len()];
    for (bi, block) in func.blocks.iter().enumerate() {
        for (vid, inst, _) in &block.instructions {
            let Instruction::Phi { incoming, .. } = inst else { continue };
            if let Some(i) = idx(vid) {
                phi_defs[bi].push(i);
            }
            for (origin, op) in incoming {
                if let Operand::Val(v) = op
                    && let Some(i) = idx(v)
                {
                    phi_uses.entry((*origin, block.id)).or_default().push(i);
                }
            }
        }
    }

    // Liveness per block (fixpoint), from back to front.
    let live_at_exit_set = |b: usize, entry: &[Set]| -> Set {
        let mut s = Set::empty(n);
        let block = &func.blocks[b];
        for succ in successors(&block.terminator) {
            let Some(&p) = position.get(&succ) else { continue };
            let mut e = entry[p].clone();
            for &d in &phi_defs[p] {
                e.take_out(d);
            }
            s.join_set(&e);
            if let Some(u) = phi_uses.get(&(block.id, succ)) {
                for &i in u {
                    s.by(i);
                }
            }
        }
        s
    };
    // The pass over a block: starting from what is live at the exit, visits each
    // instruction (what is live after it, the definition, the uses) and returns what is
    // live at the entry of the block (after the `phi`s, which are not uses).
    let walk_block = |b: usize,
                  exit: Set,
                  visit: &mut dyn FnMut(Option<&Instruction>, Option<ValueId>, Option<usize>, &Set, &Set)|
     -> Set {
        let block = &func.blocks[b];
        let mut live_set = exit;
        // The terminator: uses; `throw` and the `phi` conversions at the end of the
        // block are collection points.
        let mut before = live_set.clone();
        for v in terminator_uses(&block.terminator) {
            if let Some(i) = idx(&v) {
                before.by(i);
            }
        }
        if matches!(block.terminator, Terminator::Throw(_)) || phi_converting_blocks(block.id) {
            visit(None, None, None, &live_set, &before);
        }
        live_set = before;
        for (vid, inst, _) in block.instructions.iter().rev() {
            if matches!(inst, Instruction::Phi { .. }) {
                continue;
            }
            let d = idx(vid);
            let mut before = live_set.clone();
            if let Some(d) = d {
                before.take_out(d);
            }
            for v in uses_of(inst) {
                if let Some(i) = idx(&v) {
                    before.by(i);
                }
            }
            visit(Some(inst), Some(*vid), d, &live_set, &before);
            live_set = before;
        }
        live_set
    };

    let nb = func.blocks.len();
    let mut entry = vec![Set::empty(n); nb];
    loop {
        let mut changed = false;
        for b in (0..nb).rev() {
            let exit = live_at_exit_set(b, &entry);
            let e = walk_block(b, exit, &mut |_, _, _, _, _| {});
            if e != entry[b] {
                entry[b] = e;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    // The rooted values and the interferences.
    let mut rooted = Set::empty(n);
    let mut interfere: Vec<Set> = vec![Set::empty(n); n];
    let mark = |a: usize, live_set: &Set, interfere: &mut Vec<Set>| {
        for b in live_set.elements() {
            if b != a {
                interfere[a].by(b);
                interfere[b].by(a);
            }
        }
    };
    // The collection points, with what is live at the entry of each one:
    // (the instruction, or `None` at the end of the block; the block; the live values).
    let mut points: Vec<(Option<ValueId>, usize, Set)> = Vec::new();
    for b in 0..nb {
        let exit = live_at_exit_set(b, &entry);
        let e = walk_block(b, exit, &mut |inst, who, d, after, before| {
            let collection = inst.is_none_or(may_collect);
            if collection {
                let roots = match inst {
                    Some(i) if callee_roots(i) => {
                        let mut r = after.clone();
                        if let Some(d) = d {
                            r.take_out(d);
                        }
                        r
                    }
                    _ => before.clone(),
                };
                rooted.join_set(&roots);
                points.push((who, b, roots));
            }
            if let Some(d) = d {
                mark(d, after, &mut interfere);
            }
        });
        // The `phi`s of the block: defined together at its start.
        let mut live_set = e;
        for &d in &phi_defs[b] {
            live_set.by(d);
        }
        for &d in &phi_defs[b] {
            mark(d, &live_set, &mut interfere);
        }
    }
    // The parameters: defined together at the entry.
    if let Some(first_one) = func.blocks.first() {
        let _ = first_one;
        let mut live_set = entry[0].clone();
        let params: Vec<usize> = func.params.iter().filter_map(|(v, _, _)| idx(v)).collect();
        for &p in &params {
            live_set.by(p);
        }
        for &p in &params {
            mark(p, &live_set, &mut interfere);
        }
    }

    // Colors: the smallest free slot among the already colored neighbors.
    let mut color: Vec<Option<usize>> = vec![None; n];
    for a in 0..n {
        if !rooted.has(a) {
            continue;
        }
        let mut occupied: Vec<bool> = Vec::new();
        for b in interfere[a].elements() {
            if let Some(c) = color[b] {
                if occupied.len() <= c {
                    occupied.resize(c + 1, false);
                }
                occupied[c] = true;
            }
        }
        let c = occupied.iter().position(|&o| !o).unwrap_or(occupied.len());
        color[a] = Some(c);
        slots.insert(candidates[a], base + c);
    }
    let values = |c: &Set| -> Vec<ValueId> { c.elements().filter(|&i| rooted.has(i)).map(|i| candidates[i]).collect() };
    let mut r = Roots { slots, rooted_set: values(&rooted), ..Roots::default() };
    for (who, b, live_set) in &points {
        let v = values(live_set);
        match who {
            Some(inst) => {
                r.live_at.insert(*inst, v);
            }
            None => {
                r.live_at_end.insert(func.blocks[*b].id, v);
            }
        }
    }
    for (b, e) in entry.iter().enumerate() {
        let v = values(e);
        if !v.is_empty() {
            r.live_at_entry_set.insert(func.blocks[b].id, v);
        }
    }
    dead_slots(func, &position, &mut r);
    r
}

/// The zeroing of dead slots ([`Roots::clear_at`]): a forward pass, per block
/// to the fixpoint, of the shared slots that may hold a value (written by the
/// definition of a rooted value, the `phi`s and the parameters included). At a
/// collection point, each one of those slots that no live value occupies is zeroed
/// (and holds nothing until written again). The frame is born zeroed.
fn dead_slots(func: &Function, position: &HashMap<BlockId, usize>, r: &mut Roots) {
    let Some(&first) = r.rooted_set.iter().filter_map(|v| r.slots.get(v)).min() else { return };
    let n = r.rooted_set.iter().filter_map(|v| r.slots.get(v)).max().map_or(0, |m| m + 1);
    let slot_of = |v: &ValueId| r.slots.get(v).copied().filter(|&s| s >= first);
    let nb = func.blocks.len();
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); nb];
    for (b, block) in func.blocks.iter().enumerate() {
        for succ in successors(&block.terminator) {
            if let Some(&p) = position.get(&succ) {
                preds[p].push(b);
            }
        }
    }
    let mut entry_state = Set::empty(n);
    for (v, _, _) in &func.params {
        if let Some(s) = slot_of(v) {
            entry_state.by(s);
        }
    }
    // What each live set occupies.
    let occupied = |live: &[ValueId]| {
        let mut o = Set::empty(n);
        for v in live {
            if let Some(s) = slot_of(v) {
                o.by(s);
            }
        }
        o
    };
    let live_at: HashMap<ValueId, Set> = r.live_at.iter().map(|(k, v)| (*k, occupied(v))).collect();
    let live_at_end: HashMap<BlockId, Set> = r.live_at_end.iter().map(|(k, v)| (*k, occupied(v))).collect();
    // The pass over a block: from the slots that may hold a value at its entry, those at its
    // exit; `record` receives the zeroings.
    let walk = |b: usize, mut state: Set, record: &mut dyn FnMut(Option<ValueId>, Vec<usize>)| -> Set {
        let block = &func.blocks[b];
        let clear = |state: &mut Set, live: &Set, who: Option<ValueId>, record: &mut dyn FnMut(Option<ValueId>, Vec<usize>)| {
            let dead: Vec<usize> = state.elements().filter(|&s| !live.has(s)).collect();
            for &s in &dead {
                state.take_out(s);
            }
            if !dead.is_empty() {
                record(who, dead);
            }
        };
        for (vid, _, _) in &block.instructions {
            if let Some(live) = live_at.get(vid) {
                clear(&mut state, live, Some(*vid), record);
            }
            if let Some(s) = slot_of(vid) {
                state.by(s);
            }
        }
        if let Some(live) = live_at_end.get(&block.id) {
            clear(&mut state, live, None, record);
        }
        state
    };
    let mut exit = vec![Set::empty(n); nb];
    let entry_of = |b: usize, exit: &[Set]| {
        let mut s = if b == 0 { entry_state.clone() } else { Set::empty(n) };
        for &p in &preds[b] {
            s.join_set(&exit[p]);
        }
        s
    };
    loop {
        let mut changed = false;
        for b in 0..nb {
            let e = walk(b, entry_of(b, &exit), &mut |_, _| {});
            if e != exit[b] {
                exit[b] = e;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    for b in 0..nb {
        let id = func.blocks[b].id;
        walk(b, entry_of(b, &exit), &mut |who, dead| match who {
            Some(v) => {
                r.clear_at.insert(v, dead);
            }
            None => {
                r.clear_at_end.insert(id, dead);
            }
        });
    }
}
