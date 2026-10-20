//! Where a function opens its roots frame (the shadow stack, `RootFrame`).
//!
//! The frame used to open in the entry block of every function that has slots. A
//! function whose fast path keeps nothing alive across a collection point paid
//! for it anyway: the leaf of a recursive tree builder (`Node(null, null)`, half
//! of the calls) zeroed and chained a frame it never used. Here the frame opens in
//! the nearest common dominator of the blocks that touch a slot, when that block
//! is outside every cycle (it must open once per call) and starts without `phi`s
//! (nothing may precede the push). Every return restores the top of the shadow
//! stack read at the entry, which is right whether or not the frame opened.
//!
//! A slot is only written by the block that defines its value or by a block that
//! clears it before a collection point, and a value is live only where its
//! definition dominates, so the frame is open at every collection point where a
//! slot holds a live value. `DARTFORGE_NO_FRAME_PLACEMENT=1` at compile time
//! keeps the frame in the entry block (measurement).

use crate::hir::*;
use crate::optimize::Cfg;

/// The placement is allowed by the environment.
pub(super) fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var_os("DARTFORGE_NO_FRAME_PLACEMENT").is_none_or(|v| v.is_empty() || v == "0")
    })
}

/// The block (index in `func.blocks`) where the frame opens, given the blocks
/// that touch a slot: `None` when none does (the frame never opens), the entry
/// when no later block qualifies.
pub(super) fn frame_block(func: &Function, needs: &[usize]) -> Option<usize> {
    let (&first, rest) = needs.split_first()?;
    let cfg = Cfg::new(func);
    if needs.iter().any(|&b| !cfg.reachable(b)) {
        return Some(0);
    }
    let mut order = vec![usize::MAX; cfg.idom.len()];
    for (k, &b) in cfg.rpo.iter().enumerate() {
        order[b] = k;
    }
    let mut d = first;
    for &b in rest {
        d = common_dominator(&cfg, &order, d, b);
    }
    let starts_with_phi = func.blocks[d]
        .instructions
        .first()
        .is_some_and(|(_, i, _)| matches!(i, Instruction::Phi { .. }));
    if d == 0 || starts_with_phi || in_cycle(&cfg, d) {
        return Some(0);
    }
    Some(d)
}

fn common_dominator(cfg: &Cfg, order: &[usize], mut a: usize, mut b: usize) -> usize {
    while a != b {
        while order[a] > order[b] {
            a = cfg.idom[a];
        }
        while order[b] > order[a] {
            b = cfg.idom[b];
        }
    }
    a
}

/// Some path leaves `b` and comes back to it.
fn in_cycle(cfg: &Cfg, b: usize) -> bool {
    let mut seen = vec![false; cfg.successors.len()];
    let mut stack: Vec<usize> = cfg.successors[b].clone();
    while let Some(x) = stack.pop() {
        if x == b {
            return true;
        }
        if !std::mem::replace(&mut seen[x], true) {
            stack.extend(cfg.successors[x].iter().copied());
        }
    }
    false
}
