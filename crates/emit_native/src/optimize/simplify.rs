//! CFG cleanups after the other passes: the exception check after a call
//! that does not throw, constant comparisons and branches,
//! unreachable blocks, chained blocks and unused pure values.

use super::cfg::{Cfg, successors};
use super::effects::{checks_all, is_check};
use super::operands::*;
use crate::hir::*;
use std::collections::{HashMap, HashSet};

/// Replaces with 0 the exception check right after a call to a
/// function that does not throw.
pub fn dead_checks(func: &mut Function, do_not_throw: &HashSet<String>) -> bool {
    if !checks_all(func, do_not_throw) {
        return false;
    }
    let mut dead: HashSet<ValueId> = HashSet::new();
    for b in &func.blocks {
        for par in b.instructions.windows(2) {
            if let Instruction::CallStatic { symbol, .. } = &par[0].1
                && do_not_throw.contains(symbol)
                && is_check(&par[1].1)
            {
                dead.insert(par[1].0);
            }
        }
    }
    if dead.is_empty() {
        return false;
    }
    for b in &mut func.blocks {
        b.instructions.retain(|(v, _, _)| !dead.contains(v));
    }
    replace(func, &|v| dead.contains(&v).then_some(Operand::Constant(Constant::Int(0))));
    true
}

fn integer(o: &Operand) -> Option<i64> {
    match o {
        Operand::Constant(Constant::Int(n)) => Some(*n),
        Operand::Constant(Constant::Bool(b)) => Some(i64::from(*b)),
        _ => None,
    }
}

/// Folds integer comparisons and constant extensions, and branches on a
/// constant condition.
pub fn fold_constants(func: &mut Function) -> bool {
    let mut changed = false;
    loop {
        let mut boxes: HashMap<ValueId, (Operand, Type)> = HashMap::new();
        for b in &func.blocks {
            for (v, inst, _) in &b.instructions {
                if let Instruction::Box { op, from } = inst
                    && matches!(from, Type::I64 | Type::F64 | Type::I1)
                {
                    boxes.insert(*v, (op.clone(), *from));
                }
            }
        }
        // The header class is immutable. Only explicit allocations with a
        // Ref result prove this fact; bits, parameters and opaque calls do not.
        let classes: HashMap<_, _> = func.blocks.iter().flat_map(|b| &b.instructions).filter_map(|(v, inst, ty)| {
            if *ty != Type::Ref { return None; }
            let cid = match inst {
                Instruction::AllocObject { class_id, .. } => i64::from(*class_id),
                Instruction::CallRuntime { name, args, ret_ty: Type::Ref }
                    if matches!(name.as_str(), "dartforge_object_new" | "dartforge_arc_objeto_owned_v1") => match args.as_slice() {
                        [(Operand::Constant(Constant::Int(cid)), Type::I64), (_, Type::I64)] => *cid,
                        _ => return None,
                    },
                _ => return None,
            };
            i32::try_from(cid).ok().map(|_| (*v, cid))
        }).collect();
        let mut values: HashMap<ValueId, Operand> = HashMap::new();
        for b in &func.blocks {
            for (v, inst, _) in &b.instructions {
                let r = match inst {
                    Instruction::CallRuntime { name, args, ret_ty: Type::I64 }
                        if name == "dartforge_value_class" => match args.as_slice() {
                            [(Operand::Val(obj), Type::Ref)] => classes.get(obj).map(|&cid| Operand::Constant(Constant::Int(cid))),
                            _ => None,
                        },
                    // `x == null` with `x` also constant: the null handle is 0.
                    Instruction::ICmp(op @ (ICmpOp::Eq | ICmpOp::Ne), a, c)
                        if matches!((a, c), (Operand::Constant(Constant::Null), _) | (_, Operand::Constant(Constant::Null)))
                            && [a, c].iter().all(|o| matches!(o, Operand::Constant(Constant::Null)) || integer(o).is_some()) =>
                    {
                        let bits = |o: &Operand| integer(o).unwrap_or(0);
                        let equal = bits(a) == bits(c);
                        Some(Operand::Constant(Constant::Bool(if *op == ICmpOp::Eq { equal } else { !equal })))
                    }
                    Instruction::ICmp(op, a, c) => match (integer(a), integer(c)) {
                        (Some(x), Some(y)) => Some(match op {
                            ICmpOp::Eq => x == y,
                            ICmpOp::Ne => x != y,
                            ICmpOp::Slt => x < y,
                            ICmpOp::Sle => x <= y,
                            ICmpOp::Sgt => x > y,
                            ICmpOp::Sge => x >= y,
                            ICmpOp::Ult => (x as u64) < (y as u64),
                        })
                        .map(|b| Operand::Constant(Constant::Bool(b))),
                        _ => None,
                    },
                    Instruction::ZExt { op, from: Type::I1 | Type::I8, .. } => {
                        integer(op).map(|n| Operand::Constant(Constant::Int(n & 0xFF)))
                    }
                    // The undone box: `Unbox(Box(x))` is `x` (the `int`, the
                    // `double` and the `bool` come back exact; nothing throws).
                    Instruction::Unbox { op: Operand::Val(b), to } => match boxes.get(b) {
                        Some((x, de)) if de == to || (matches!(de, Type::I1 | Type::I8) && matches!(to, Type::I1)) => {
                            Some(x.clone())
                        }
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(r) = r {
                    values.insert(*v, r);
                }
            }
        }
        // Chained replacements: `v178 = Unbox(Box(v49))` becomes `v49`, but
        // `v49 = Unbox(Box(v46))` itself also goes away in this round — without following
        // the chain, users of `v178` were left with the already removed `v49` ("use
        // of undefined value" in LLVM: `a = aa = x` with `int?`, pointycastle's
        // `RIPEMD128Digest` in new_sali/backend;
        // docs/NATIVO-PROJETOS-REAIS.md, C10). The chain is acyclic: each
        // replacement points to a value defined earlier.
        let keys: Vec<ValueId> = values.keys().copied().collect();
        for k in keys {
            let mut alvo = values[&k].clone();
            let mut steps = 0;
            while let Operand::Val(x) = &alvo
                && let Some(next) = values.get(x)
                && steps < values.len()
            {
                alvo = next.clone();
                steps += 1;
            }
            values.insert(k, alvo);
        }
        let mut changed_here = false;
        if !values.is_empty() {
            for b in &mut func.blocks {
                b.instructions.retain(|(v, _, _)| !values.contains_key(v));
            }
            replace(func, &|v| values.get(&v).cloned());
            changed_here = true;
        }
        // Desvios constantes.
        let ids: Vec<BlockId> = func.blocks.iter().map(|b| b.id).collect();
        let mut removed_edges: Vec<(BlockId, BlockId)> = Vec::new();
        for (i, b) in func.blocks.iter_mut().enumerate() {
            if let Terminator::CondBranch { cond, then_block, else_block } = &b.terminator
                && let Some(c) = integer(cond)
            {
                let (goes, stays) = if c != 0 { (*then_block, *else_block) } else { (*else_block, *then_block) };
                if goes != stays {
                    removed_edges.push((ids[i], stays));
                }
                b.terminator = Terminator::Branch(goes);
                changed_here = true;
            } else if let Terminator::Switch { val, default, cases } = &b.terminator
                && let Some(c) = integer(val)
            {
                let goes = cases.iter().find(|(n, _)| *n == c).map(|(_, d)| *d).unwrap_or(*default);
                for dest in successors(&b.terminator) {
                    if dest != goes { removed_edges.push((ids[i], dest)); }
                }
                b.terminator = Terminator::Branch(goes);
                changed_here = true;
            }
        }
        for (origin, dest) in removed_edges {
            if let Some(b) = func.blocks.iter_mut().find(|b| b.id == dest) {
                remove_entries(b, origin);
            }
        }
        if !changed_here {
            return changed;
        }
        changed = true;
    }
}

fn remove_entries(b: &mut BasicBlock, origin: BlockId) {
    for (_, inst, _) in &mut b.instructions {
        if let Instruction::Phi { incoming, .. } = inst {
            incoming.retain(|(o, _)| *o != origin);
        }
    }
}

/// Removes unreachable blocks (and the `phi` entries coming from them).
pub fn remove_unreachable(func: &mut Function) -> bool {
    let cfg = Cfg::new(func);
    let dead_set: HashSet<BlockId> =
        func.blocks.iter().enumerate().filter(|(i, _)| !cfg.reachable(*i)).map(|(_, b)| b.id).collect();
    if dead_set.is_empty() {
        return false;
    }
    func.blocks.retain(|b| !dead_set.contains(&b.id));
    for b in &mut func.blocks {
        for (_, inst, _) in &mut b.instructions {
            if let Instruction::Phi { incoming, .. } = inst {
                incoming.retain(|(o, _)| !dead_set.contains(o));
            }
        }
    }
    true
}

/// Merges `a -> b` when `b` has only `a` as predecessor and `a` only goes to
/// `b` (with no `phi` in `b`, or with single-entry `phi`s resolved).
pub fn join_blocks(func: &mut Function) -> bool {
    let mut changed = false;
    loop {
        let mut preds: HashMap<BlockId, usize> = HashMap::new();
        for b in &func.blocks {
            for s in successors(&b.terminator) {
                *preds.entry(s).or_default() += 1;
            }
        }
        let alvo = func.blocks.iter().enumerate().find_map(|(i, b)| match b.terminator {
            Terminator::Branch(s) if s != b.id && s.0 != 0 && preds.get(&s) == Some(&1) => Some((i, s)),
            _ => None,
        });
        let Some((i, s)) = alvo else { return changed };
        let j = func.blocks.iter().position(|b| b.id == s).expect("sucessor existe");
        let mut child = func.blocks.remove(j);
        let i = if j < i { i - 1 } else { i };
        let parent = func.blocks[i].id;
        // Single-entry `phi`s become their value.
        let mut swap: HashMap<ValueId, Operand> = HashMap::new();
        child.instructions.retain(|(v, inst, _)| match inst {
            Instruction::Phi { incoming, .. } => {
                let value = incoming.first().map(|(_, o)| o.clone()).unwrap_or(Operand::Constant(Constant::Null));
                swap.insert(*v, value);
                false
            }
            _ => true,
        });
        func.blocks[i].instructions.extend(child.instructions);
        func.blocks[i].terminator = child.terminator;
        // The successors of the child now come from the parent.
        for b in &mut func.blocks {
            for (_, inst, _) in &mut b.instructions {
                if let Instruction::Phi { incoming, .. } = inst {
                    for (o, _) in incoming.iter_mut() {
                        if *o == s {
                            *o = parent;
                        }
                    }
                }
            }
        }
        if !swap.is_empty() {
            replace(func, &|v| swap.get(&v).cloned());
        }
        changed = true;
    }
}

/// Does the instruction have no effect beyond its result (removable if nobody uses it)?
fn pure(inst: &Instruction) -> bool {
    match inst {
        Instruction::Const(_)
        | Instruction::Add(..)
        | Instruction::Sub(..)
        | Instruction::Mul(..)
        | Instruction::Shl(..)
        | Instruction::AShr(..)
        | Instruction::LShr(..)
        | Instruction::And(..)
        | Instruction::Or(..)
        | Instruction::Xor(..)
        | Instruction::Neg(_)
        | Instruction::Not(_)
        | Instruction::FAdd(..)
        | Instruction::FSub(..)
        | Instruction::FMul(..)
        | Instruction::FDiv(..)
        | Instruction::FNeg(_)
        | Instruction::ICmp(..)
        | Instruction::FCmp(..)
        | Instruction::LNot(_)
        | Instruction::IntToDouble(_)
        | Instruction::ZExt { .. }
        | Instruction::Trunc { .. }
        | Instruction::Bitcast { .. }
        | Instruction::Box { .. }
        | Instruction::GetField { .. }
        | Instruction::Load { .. }
        | Instruction::Phi { .. }
        | Instruction::IsClass { .. }
        | Instruction::EnvGet { .. }
        | Instruction::CellGet { .. } => true,
        Instruction::Unbox { to, .. } => to.is_vector(),
        Instruction::Simd { op, .. } => *op != OpSimd::Writes,
        Instruction::CallRuntime { name, .. } => {
            matches!(name.as_str(), "dartforge_object_new" | "dartforge_exception_pending" | "dartforge_value_class")
                || pure_helper(name)
        }
        _ => false,
    }
}

/// A `@df.*` helper of the unified space (docs/NATIVO-ESPACO-UNIFICADO.md
/// §3.5) with no effect beyond its result: it is in one of the four effect
/// tables (`llvm/ir_strings.rs`, `ir_boxes.rs`, `ir_lists.rs`,
/// `ir_typed.rs`, read by `externs::effects_of`), does not throw and does not write
/// to the heap (the `*_gravar*` ones write, even those that return a value). Those that
/// allocate (boxes, strings, SIMD) are removable like an object allocation;
/// `@df.texto_hash` only fills the hash cache.
fn pure_helper(name: &str) -> bool {
    use crate::llvm::externs::{CONSERVATIVE, effects_of};
    if !name.starts_with("df.") || name.contains("gravar") {
        return false;
    }
    let e = effects_of(name);
    e != CONSERVATIVE && !e.throws
}

/// Removes the pure values that nobody uses.
pub fn remove_dead(func: &mut Function) -> bool {
    let mut changed = false;
    loop {
        let mut used_set: HashSet<ValueId> = HashSet::new();
        for b in &func.blocks {
            for (v, inst, _) in &b.instructions {
                operands(inst, &mut |o| {
                    if let Operand::Val(u) = o
                        && u != v
                    {
                        used_set.insert(*u);
                    }
                });
            }
            terminator_operands(&b.terminator, &mut |o| {
                if let Operand::Val(u) = o {
                    used_set.insert(*u);
                }
            });
        }
        let mut removed_done = false;
        for b in &mut func.blocks {
            let before = b.instructions.len();
            b.instructions.retain(|(v, inst, _)| used_set.contains(v) || !pure(inst));
            removed_done |= b.instructions.len() != before;
        }
        if !removed_done {
            return changed;
        }
        changed = true;
    }
}
