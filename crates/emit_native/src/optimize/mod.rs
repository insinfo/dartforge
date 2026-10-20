//! Optimizations over the HIR, before LLVM (docs/NATIVO-PLANO.md §8).
//!
//! LLVM only optimizes what it receives: an object created by the runtime and read by
//! opaque calls does not go away there. Here, with Dart semantics at hand, the
//! passes remove the work before it becomes a call:
//!
//! 1. `mem2reg`: locals become SSA values;
//! 2. exception summary (`effects`) and the check after a call
//!    that does not throw folded away (`simplify`);
//! 3. inlining of small functions (`inline`) — constructors, getters,
//!    operators —, also of those that throw (the check after the call
//!    takes the exception to the caller's handler);
//! 4. scalar replacement of objects that do not escape (`escape`), and
//!    `mem2reg` and the cleanups again.
//!
//! `DARTFORGE_OPTIMIZE_HIR=0` turns everything off (for comparison).

mod cfg;
pub mod arc;
mod effects;
pub(crate) mod scc;
mod escape;
mod inline;
mod direct_locals;
mod mem2reg;
pub(crate) mod operands;
mod simplify;
mod representation_constants;
mod tables;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod pre_prune_tests;
mod physical_devirtualize;
mod checked_double_roundtrip;
mod constant_args;
mod constant_specialize;
pub(crate) mod primitive_clamp;
pub(crate) mod enum_constants;
mod scalar_coalesce;
mod string_lengths;

use crate::hir::*;
use std::collections::HashMap;

/// Remove proven unreachable CFG edges before collecting symbol/selector roots.
/// This performs no inlining, ownership insertion, or exception-table lowering.
pub(crate) fn prepare_for_pruning(module: &mut Module) {
    if disabled() || std::env::var("DARTFORGE_NO_PRE_PRUNE_CFG").as_deref() == Ok("1") {
        return;
    }
    prune_constant_cfg(module);
}

/// Specialize bounded constant contexts after the first reachability pruning.
/// Dead native adapters must be removed before constructing enum identity proofs.
/// This runs before effect summaries can remove pending-exception checks.
/// The result counts the rewrites; above zero, the caller prunes again
/// (`lib.rs`): the constant arguments (`constant_args`) fold branches that
/// held whole backends alive.
pub(crate) fn specialize_constants(module: &mut Module, requested_table_exceptions: bool) -> usize {
    if disabled() {
        return 0;
    }
    let propagated = constant_args::propagate(module);
    if requested_table_exceptions {
        return propagated;
    }
    let specialized = constant_specialize::rewrite(module);
    propagated + specialized + primitive_clamp::rewrite(module)
}


fn prune_constant_cfg(module: &mut Module) {
    for function in &mut module.functions {
        loop {
            let mut changed = representation_constants::propagate_literals(function);
            changed |= constant_args::fold_string_equality(function);
            changed |= simplify::fold_constants(function);
            changed |= simplify::remove_unreachable(function);
            if !changed {
                break;
            }
        }
    }
}

/// Prepares the program and the SDK libraries through the same emission pipeline.
/// The memory model is defined before the passes; the materialization of
/// exceptions stays last, since its IDs describe the final HIR.
/// The insertion of ARC owners is not part of this pipeline yet (§20).
/// Its integration must stay conditioned on `arc_memory` (`--memory=arc`):
/// tracing gets no Owned factories, ARC retains or releases.
pub(crate) fn prepare_for_emission(module: &mut Module, arc_memory: bool, by_tables: bool) {
    module.arc_memory = arc_memory;
    optimize(module);
    if by_tables {
        let plan = tables::prepare(module);
        // The insertion/verification of ownership is expected to happen here, over
        // the prepared exceptional CFG, before publishing LLVM tables.
        tables::materialize(module, plan);
    }
}

/// Are the passes turned off by the environment?
pub(crate) fn disabled() -> bool {
    std::env::var("DARTFORGE_OPTIMIZE_HIR").is_ok_and(|v| v == "0")
}

fn clear(func: &mut Function, do_not_throw: &std::collections::HashSet<String>) {
    // In a function that does not throw, the exception is never pending: the
    // `dartforge_exception_clear` of the `return` (which discards the pending one when
    // leaving a `finally`) has nothing to clear.
    if do_not_throw.contains(&func.symbol) {
        for b in &mut func.blocks {
            b.instructions.retain(|(_, i, _)| !matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_exception_clear"));
        }
    }
    simplify::dead_checks(func, do_not_throw);
    // String constants may become visible through local CFG simplification or
    // inlining, even when no parameter remains for interprocedural propagation.
    constant_args::fold(func);
}

/// The entry is the first block in all the rest of the compiler.
fn valid(f: &Function) -> bool {
    f.blocks.first().is_some_and(|b| b.id.0 == 0)
}

/// How many `CallStatic` sites each symbol has in the module.
fn static_call_sites(module: &Module) -> HashMap<String, usize> {
    let mut sites: HashMap<String, usize> = HashMap::new();
    for inst in module.functions.iter().flat_map(|f| &f.blocks).flat_map(|b| &b.instructions) {
        if let Instruction::CallStatic { symbol, .. } = &inst.1 {
            *sites.entry(symbol.clone()).or_default() += 1;
        }
    }
    sites
}

/// How many threads the per-function passes use: the machine's, at most 4
/// (`DARTFORGE_THREADS_HIR` chooses; 1 = sequential).
fn threads() -> usize {
    std::env::var("DARTFORGE_THREADS_HIR")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get().min(4)))
        .max(1)
}

/// Applies `pass` to each valid function, in contiguous chunks per thread: the
/// per-function passes only read the rest of the module through what they receive ready
/// (`do_not_throw`, the inliner's copies), so the result is the same as the
/// sequential order. Returns whether some pass changed some function. In
/// new_sali/backend (~180 thousand functions) the passes took ~20 s on one thread.
fn in_parallel(functions: &mut [Function], pass: &(dyn Fn(&mut Function) -> bool + Sync)) -> bool {
    let n = threads();
    if n == 1 || functions.len() < 2048 {
        let mut changed = false;
        for f in functions.iter_mut().filter(|f| valid(f)) {
            changed |= pass(f);
        }
        return changed;
    }
    // Small chunks distributed on demand: the large functions (the
    // literals and the data tables) cluster together, and one chunk per thread
    // left one thread with almost all the work.
    let chunks: Vec<std::sync::Mutex<&mut [Function]>> =
        functions.chunks_mut(256).map(std::sync::Mutex::new).collect();
    let next_item = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        let tasks: Vec<_> = (0..n)
            .map(|_| {
                let (chunks, next_item) = (&chunks, &next_item);
                std::thread::Builder::new()
                    .stack_size(64 << 20)
                    .spawn_scoped(s, move || {
                        let mut changed = false;
                        loop {
                            let i = next_item.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some(p) = chunks.get(i) else { break };
                            let mut chunk = p.lock().unwrap_or_else(|e| e.into_inner());
                            for f in chunk.iter_mut().filter(|f| valid(f)) {
                                changed |= pass(f);
                            }
                        }
                        changed
                    })
                    .expect("HIR passes thread")
            })
            .collect();
        tasks.into_iter().fold(false, |acc, t| acc | t.join().expect("HIR pass panicked"))
    })
}

/// Switches the module to exceptions by tables (`--exceptions=tables`,
/// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13): the last pass over the HIR,
/// after [`optimize`] and right before emission. Without it the module stays in the
/// usual model (the pending state checked after each call).
pub fn table_exceptions(module: &mut Module) {
    tables::apply(module);
}

/// Optimizes the functions of the module.
pub fn optimize(module: &mut Module) {
    if disabled() {
        return;
    }
    in_parallel(&mut module.functions, &|f| {
        mem2reg::promote(f);
        false
    });
    physical_devirtualize::rewrite_getters(module);
    for _ in 0..3 {
        let do_not_throw = effects::do_not_throw(module);
        in_parallel(&mut module.functions, &|f| {
            clear(f, &do_not_throw);
            false
        });
        // The cleanup may free SSA IDs that the next copies reuse.
        // Do not let a dead origin stick to a new instruction with that ID.
        if module.arc_memory { arc::analysis::origins::prune(module); }
        let copies: HashMap<String, (Function, bool)> = module
            .functions
            .iter()
            // J05: a function compiled with debug information is not
            // inlined — the copied body would lose its position, and a breakpoint
            // in it would never stop.
            .filter(|f| f.debug_info.is_none() && inline::copyable_with_policy(f, module.inline_policies.get(&f.symbol).copied()))
            .map(|f| (f.symbol.clone(), (f.clone(), !do_not_throw.contains(&f.symbol))))
            .collect();
        // Cleanup may remove an allocation and a later copy may reuse its ID.
        // Only certificates still bound to the original allocation survive.
        module.async_frame_layouts.retain(|(owner, site, _, size)| {
            module.functions.iter().find(|f| &f.symbol == owner).is_some_and(|f| f.blocks.iter()
                .flat_map(|b| &b.instructions).any(|(id, inst, _)| id == site && matches!(inst,
                    Instruction::AllocObject {class_id, fields}
                    if *class_id == crate::lower::async_sm::ASYNC_FRAME_ID && fields.len() == *size)))
        });
        let mut frame_sources: HashMap<String, HashMap<ValueId, (String, usize)>> = HashMap::new();
        for (owner, site, body, size) in &module.async_frame_layouts {
            frame_sources.entry(owner.clone()).or_default().insert(*site, (body.clone(), *size));
        }
        let updated_frames = std::sync::Mutex::new(HashMap::new());
        let origins = &module.arc_sites;
        let updated = std::sync::Mutex::new(HashMap::new());
        let arc_memory = module.arc_memory;
        // The static call sites of each symbol, for the inliner's budget in the
        // arms of a devirtualization `switch` (`inline::ARM_BODY_LIMIT`).
        let call_sites = static_call_sites(module);
        let changed = in_parallel(&mut module.functions, &|f| {
            let mut sites = origins.get(&f.symbol).cloned().unwrap_or_default();
            let mut frames = frame_sources.get(&f.symbol).cloned().unwrap_or_default();
            let changed = inline::inlining_with_metadata(f, &copies, origins, &mut sites, &frame_sources, &mut frames, &call_sites);
            if arc_memory && !sites.is_empty() {
                updated.lock().unwrap().insert(f.symbol.clone(), sites);
            }
            if !frames.is_empty() {
                updated_frames.lock().unwrap().insert(f.symbol.clone(), frames);
            }
            // The inlined body of a direct local function leaves the local that
            // it received by address with only `Load`/`Store`: promotable.
            if changed {
                mem2reg::promote(f);
                clear(f, &do_not_throw);
            }
            changed
        });
        if arc_memory { module.arc_sites.extend(updated.into_inner().unwrap()); }
        module.async_frame_layouts = updated_frames.into_inner().unwrap().into_iter()
            .flat_map(|(owner, frames)| frames.into_iter().map(move |(site, (body, size))| (owner.clone(), site, body, size))).collect();
        if !changed {
            break;
        }
    }
    let do_not_throw = effects::do_not_throw(module);
    in_parallel(&mut module.functions, &|f| {
        if escape::replace_objects(f) {
            mem2reg::promote(f);
            clear(f, &do_not_throw);
        }
        false
    });
    scalar_coalesce::rewrite(module);
    checked_double_roundtrip::rewrite(module);
    string_lengths::rewrite(module);
    // Inlining and representation folding can expose literal class queries.
    // Fixed registration facts prove only the negative cases; the usual cleanup
    // removes their unreachable branches before tables or ownership plans exist.
    if arc::fold_fixed_subtype_tests(module) {
        let do_not_throw = effects::do_not_throw(module);
        in_parallel(&mut module.functions, &|f| {
            clear(f, &do_not_throw);
            false
        });
    }
    if module.arc_memory {
        direct_locals::prune(module);
        arc::analysis::origins::prune(module);
    }
}
