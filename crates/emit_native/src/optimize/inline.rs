//! Inlining of small functions of the module.
//!
//! An exception is a pending state of the runtime that the caller checks right
//! after the call (`FnBuilder::emit_call_with_check`): whoever throws leaves the
//! exception pending and **returns**. That is why the copied body need not be
//! rewired to the caller's `catch`/`finally`: the exceptional exit of the
//! copy is a `return` like the others, which becomes a branch to the continuation,
//! and the check that followed the call — which stays in the continuation — takes the
//! exception to the caller's handler, as before. The body runs the same, with the
//! same runtime state; only the call boundary goes away.
//!
//! The arguments take the place of the parameters; each `return` becomes a
//! branch to the continuation, with a `phi` if there is more than one. The
//! `dartforge_exception_clear` of the `return` (which discards a pending exception
//! when leaving a `finally`) stays in the copy of a function that may throw; in the
//! copy of one that does not throw it goes away (there is nothing to discard), and the check after
//! the call has already been folded (`simplify::dead_checks`). The
//! `Throw` terminator (throws and does not come back) is not copied.

use super::mem2reg::compatible;
use super::operands::*;
use crate::hir::*;
use std::collections::HashMap;

/// Instructions of a function that gets copied.
pub const BODY_LIMIT: usize = 40;
/// Maximum size of the function receiving the copies.
pub const CALLER_LIMIT: usize = 4000;

fn size(f: &Function) -> usize {
    f.blocks.iter().map(|b| b.instructions.len() + 1).sum()
}

/// Can the function be copied in place of the call?
#[cfg(test)]
pub fn copyable(f: &Function) -> bool {
    copyable_with_policy(f, None)
}

/// Applies declaration requests without bypassing recursion or structural checks.
pub fn copyable_with_policy(f: &Function, policy: Option<InlinePolicy>) -> bool {
    let limit = match policy {
        Some(InlinePolicy::Never) => return false,
        Some(InlinePolicy::Prefer) => BODY_LIMIT * 4,
        None => BODY_LIMIT,
    };
    size(f) <= limit
        && f.blocks.first().is_some_and(|b| b.id.0 == 0)
        && f.blocks.iter().all(|b| {
            !matches!(b.terminator, Terminator::Throw(_))
                && b.instructions.iter().all(|(_, i, _)| match i {
                    // A local would end up in a block that may be in a loop.
                    Instruction::Alloca(_) => false,
                    // The function itself (recursion).
                    Instruction::CallStatic { symbol, .. } => *symbol != f.symbol,
                    // The adapters of the uniform convention read the
                    // parameters of their own entry.
                    Instruction::LoadIndexed { .. } => false,
                    // The lazy getter of a global or of an enum
                    // value (writes the value on the first read): copying it
                    // duplicates the whole initialization, which runs once, into
                    // every read. In a table of 16 thousand `Cat.lu` (the `bidi`
                    // of `pdf_plus` in new_sali/backend) it was ~135 lines of
                    // IR per element — 97 MB in a constant getter
                    // (docs/NATIVO-PROJETOS-REAIS.md, C8). The VM also reads the
                    // static field and only calls the initializer the first
                    // time (`LoadStaticField` + `InitStaticField`).
                    Instruction::StoreGlobal { .. } => false,
                    _ => true,
                })
        })
}

/// Copies, in `func`, the calls to functions of `copies` (with whether the function
/// may throw). Returns whether anything changed.
pub fn inlining(func: &mut Function, copies: &HashMap<String, (Function, bool)>) -> bool {
    inlining_with_origins(func, copies, &HashMap::new(), &mut HashMap::new())
}

pub(super) fn inlining_with_origins(
    func: &mut Function, copies: &HashMap<String, (Function, bool)>,
    sources: &HashMap<String, HashMap<ValueId, crate::optimize::arc::analysis::model::ArcSite>>,
    sites: &mut HashMap<ValueId, crate::optimize::arc::analysis::model::ArcSite>,
) -> bool {
    let mut changed = false;
    let mut budget = 64;
    while budget > 0 && size(func) < CALLER_LIMIT {
        let types = function_types(func);
        let found = func.blocks.iter().enumerate().find_map(|(bi, b)| {
            b.instructions.iter().enumerate().find_map(|(ii, (_, inst, ty))| {
                let Instruction::CallStatic { symbol, args, .. } = inst else { return None };
                let (target, _) = copies.get(symbol)?;
                if *symbol == func.symbol || target.params.len() != args.len() {
                    return None;
                }
                let arguments_ok = target.params.iter().zip(args).all(|((_, _, tp), a)| {
                    operand_type(a, &types).is_some_and(|ta| ta == *tp || (compatible(*tp, ta) && *tp != Type::Ref))
                });
                let target_types = function_types(target);
                let return_ok = *ty == Type::Void
                    || target.blocks.iter().all(|b| match &b.terminator {
                        Terminator::Return(Some(r)) => operand_type(r, &target_types)
                            .is_some_and(|tr| tr == *ty || (compatible(*ty, tr) && *ty != Type::Ref)),
                        Terminator::Return(None) => *ty == Type::Ref,
                        _ => true,
                    });
                (arguments_ok && return_ok).then_some((bi, ii))
            })
        });
        let Some((bi, ii)) = found else { break };
        copy(func, bi, ii, copies, sources, sites);
        changed = true;
        budget -= 1;
    }
    changed
}

fn copy(func: &mut Function, bi: usize, ii: usize, copies: &HashMap<String, (Function, bool)>,
    sources: &HashMap<String, HashMap<ValueId, crate::optimize::arc::analysis::model::ArcSite>>,
    sites: &mut HashMap<ValueId, crate::optimize::arc::analysis::model::ArcSite>,
) {
    let (mut next_v, mut next_b) = largest_ids(func);
    let (call, args, ty) = match &func.blocks[bi].instructions[ii] {
        (v, Instruction::CallStatic { symbol, args, .. }, ty) => (*v, (symbol.clone(), args.clone()), *ty),
        _ => unreachable!("expected static call"),
    };
    let (sym_name, args) = args;
    let (target, throws) = &copies[&sym_name];
    let throws = *throws;

    // New ids for the values and blocks of the copy.
    let mut values: HashMap<ValueId, Operand> = HashMap::new();
    for ((p, _, _), a) in target.params.iter().zip(&args) {
        values.insert(*p, a.clone());
    }
    for b in &target.blocks {
        for (v, _, _) in &b.instructions {
            next_v += 1;
            values.insert(*v, Operand::Val(ValueId(next_v)));
        }
    }
    // The identity belongs to the origin; only the SSA index of the copy changes.
    // Contexts are joined conservatively, without inferring a singleton per clone.
    if let Some(origins) = sources.get(&sym_name) {
        for (v, site) in origins {
            if let Some(Operand::Val(new)) = values.get(v) {
                sites.insert(*new, site.clone());
            }
        }
    }
    sites.remove(&call);
    let mut blocks: HashMap<BlockId, BlockId> = HashMap::new();
    for b in &target.blocks {
        next_b += 1;
        blocks.insert(b.id, BlockId(next_b));
    }
    next_b += 1;
    let continuation = BlockId(next_b);
    let origin = func.blocks[bi].id;

    // Splits the block of the call.
    let after: Vec<(ValueId, Instruction, Type)> = func.blocks[bi].instructions.split_off(ii + 1);
    func.blocks[bi].instructions.pop();
    let terminator = std::mem::replace(&mut func.blocks[bi].terminator, Terminator::Branch(blocks[&target.blocks[0].id]));
    // The successors of the original block now come from the continuation.
    for s in super::cfg::successors(&terminator) {
        if let Some(b) = func.blocks.iter_mut().find(|b| b.id == s) {
            for (_, inst, _) in &mut b.instructions {
                if let Instruction::Phi { incoming, .. } = inst {
                    for (o, _) in incoming.iter_mut() {
                        if *o == origin {
                            *o = continuation;
                        }
                    }
                }
            }
        }
    }

    let mut returns: Vec<(BlockId, Operand)> = Vec::new();
    let mut new_set: Vec<BasicBlock> = Vec::new();
    for b in &target.blocks {
        let id = blocks[&b.id];
        let mut instructions = Vec::with_capacity(b.instructions.len());
        for (v, inst, t) in &b.instructions {
            if !throws && matches!(inst, Instruction::CallRuntime { name, .. } if name == "dartforge_exception_clear") {
                continue;
            }
            let mut inst = inst.clone();
            operands_mut(&mut inst, &mut |o| {
                if let Operand::Val(x) = o
                    && let Some(n) = values.get(x)
                {
                    *o = n.clone();
                }
            });
            if let Instruction::Phi { incoming, .. } = &mut inst {
                for (o, _) in incoming.iter_mut() {
                    *o = blocks[o];
                }
            }
            let Operand::Val(nv) = values[v] else { unreachable!("new value") };
            instructions.push((nv, inst, *t));
        }
        let mut term = b.terminator.clone();
        terminator_operands_mut(&mut term, &mut |o| {
            if let Operand::Val(x) = o
                && let Some(n) = values.get(x)
            {
                *o = n.clone();
            }
        });
        super::operands::successors_mut(&mut term, &mut |s| *s = blocks[s]);
        if let Terminator::Return(r) = &term {
            returns.push((id, r.clone().unwrap_or(Operand::Constant(Constant::Null))));
            term = Terminator::Branch(continuation);
        }
        new_set.push(BasicBlock { id, instructions: instructions, terminator: term });
    }

    // The symbolic stack trace (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.14):
    // the copied instructions keep the position in the copied function and the
    // inlining context, so that its frame appears the way the VM shows
    // inlined frames. It stays out of the line-table positions (their
    // file is that of this function). A caller without positions (the uniform
    // entry of a closure) gets a hidden debug info: only the copied
    // frames appear.
    if let Some(dinfo) = target.debug_info.as_deref()
        && (func.debug_info.is_some() || crate::target::symbolic_stack_trace().unwrap_or(false))
    {
        let dc = func.debug_info.get_or_insert_with(|| {
            Box::new(FunctionDebugInfo {
                file: dinfo.file.clone(),
                url: dinfo.url.clone(),
                line: dinfo.line,
                marks: stack_trace_marks::HIDDEN,
                ..Default::default()
            })
        });
        let (call_position, parent) = match dc.embedded_positions.get(&call) {
            Some(&(l, c, j)) => ((l, c), Some(j)),
            None => (dc.positions.get(&call).copied().unwrap_or((dc.line, 0)), None),
        };
        let k = dc.embedded_set.len() as u32;
        dc.embedded_set.push(Embedded { name: target.stack_trace_name(), url: dinfo.url.clone(), call: call_position, parent, marks: dinfo.marks });
        for e in &dinfo.embedded_set {
            dc.embedded_set.push(Embedded { parent: Some(e.parent.map_or(k, |p| k + 1 + p)), ..e.clone() });
        }
        for b in &target.blocks {
            for (v, _, _) in &b.instructions {
                let Some(Operand::Val(new)) = values.get(v) else { continue };
                let p = match (dinfo.embedded_positions.get(v), dinfo.positions.get(v)) {
                    (Some(&(l, c, j)), _) => (l, c, k + 1 + j),
                    (None, Some(&(l, c))) => (l, c, k),
                    (None, None) => continue,
                };
                dc.embedded_positions.insert(*new, p);
            }
        }
    }

    // The result: the single return, or a `phi` in the continuation.
    let mut start = Vec::new();
    let result = match returns.len() {
        0 => None,
        1 => Some(returns[0].1.clone()),
        _ if ty == Type::Void => None,
        _ => {
            next_v += 1;
            let phi = ValueId(next_v);
            start.push((phi, Instruction::Phi { incoming: returns.clone(), ty }, ty));
            Some(Operand::Val(phi))
        }
    };
    start.extend(after);
    let cont = BasicBlock { id: continuation, instructions: start, terminator: terminator };
    let pos = bi + 1;
    let n = new_set.len();
    func.blocks.splice(pos..pos, new_set);
    func.blocks.insert(pos + n, cont);
    let r = result.unwrap_or_else(|| default_constant(ty));
    replace(func, &|v| (v == call).then(|| r.clone()));
}

#[cfg(test)]
mod policy_tests {
    use super::*;

    fn candidate(size: usize) -> Function {
        Function {
            symbol: "candidate".into(), name: "candidate".into(), debug_info: None,
            params: vec![], return_ty: Type::I64,
            blocks: vec![BasicBlock { id: BlockId(0),
                instructions: (0..size).map(|i| (ValueId(i as u32),
                    Instruction::Const(Constant::Int(i as i64)), Type::I64)).collect(),
                terminator: Terminator::Return(Some(Operand::Constant(Constant::Int(7)))) }],
        }
    }

    #[test]
    fn inline_policy_keeps_safety_and_bounded_preferences() {
        let small = candidate(1);
        assert!(copyable(&small));
        assert!(!copyable_with_policy(&small, Some(InlinePolicy::Never)));
        let larger = candidate(BODY_LIMIT + 1);
        assert!(!copyable(&larger));
        assert!(copyable_with_policy(&larger, Some(InlinePolicy::Prefer)));
        assert!(!copyable_with_policy(&candidate(BODY_LIMIT * 4), Some(InlinePolicy::Prefer)));
        let mut recursive = small;
        recursive.blocks[0].instructions.push((ValueId(2), Instruction::CallStatic {
            symbol: recursive.symbol.clone(), args: vec![], ret_ty: Type::I64,
        }, Type::I64));
        assert!(!copyable_with_policy(&recursive, Some(InlinePolicy::Prefer)));
        recursive.blocks[0].instructions.pop();
        recursive.blocks[0].instructions.push((ValueId(2), Instruction::Alloca(Type::I64), Type::Ptr));
        assert!(!copyable_with_policy(&recursive, Some(InlinePolicy::Prefer)));
    }
}
