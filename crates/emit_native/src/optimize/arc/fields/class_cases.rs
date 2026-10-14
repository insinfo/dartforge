use super::*;
use crate::optimize::cfg::{Cfg, successors};

#[derive(Default)]
pub(super) struct Cases {
    positions: HashMap<BlockId, usize>,
    idom: Vec<usize>,
    witnesses: Vec<(ValueId, usize, u32)>,
    allocations: HashSet<ValueId>,
}

fn dominates(idom: &[usize], ancestor: usize, mut block: usize) -> bool {
    if idom[ancestor] == usize::MAX || idom[block] == usize::MAX {
        return false;
    }
    loop {
        if ancestor == block {
            return true;
        }
        let parent = idom[block];
        if parent == block {
            return false;
        }
        block = parent;
    }
}

impl Cases {
    pub(super) fn new(f: &Function, canonical_runtime: bool) -> Self {
        if !canonical_runtime
            || f.blocks.is_empty()
            || !f
                .blocks
                .iter()
                .any(|b| matches!(b.terminator, Terminator::Switch { .. }))
            || f.blocks.len() > 512
            || f.blocks.iter().any(
                |b| matches!(&b.terminator, Terminator::Switch { cases, .. } if cases.len() > 512),
            )
            || f.blocks.iter().map(|b| b.instructions.len()).sum::<usize>() > 8192
        {
            return Self::default();
        }
        let mut positions = HashMap::new();
        for (position, block) in f.blocks.iter().enumerate() {
            if positions.insert(block.id, position).is_some() {
                return Self::default();
            }
        }
        if f.blocks.iter().any(|b| {
            successors(&b.terminator)
                .iter()
                .any(|id| !positions.contains_key(id))
        }) {
            return Self::default();
        }
        let mut definitions = HashMap::new();
        for (v, _, ty) in &f.params {
            if definitions.insert(*v, (*ty, None)).is_some() {
                return Self::default();
            }
        }
        let mut queries = HashMap::new();
        let mut allocations = HashSet::new();
        for (b, block) in f.blocks.iter().enumerate() {
            for (position, (v, instruction, ty)) in block.instructions.iter().enumerate() {
                if definitions.insert(*v, (*ty, Some((b, position)))).is_some() {
                    return Self::default();
                }
                match instruction {
                    Instruction::CallRuntime {
                        name,
                        args,
                        ret_ty: Type::I64,
                    } if *ty == Type::I64 && name == "dartforge_value_class" => {
                        if let [(Operand::Val(receiver), Type::Ref)] = args.as_slice() {
                            queries.insert(*v, (*receiver, b, position));
                        }
                    }
                    Instruction::AllocObject { class_id, .. }
                        if *ty == Type::Ref && i32::try_from(*class_id).is_ok() =>
                    {
                        allocations.insert(*v);
                    }
                    Instruction::CallRuntime {
                        name,
                        args,
                        ret_ty: Type::Ref,
                    } if *ty == Type::Ref
                        && matches!(
                            name.as_str(),
                            "dartforge_object_new" | "dartforge_arc_object_owned_v1"
                        ) =>
                    {
                        if let [
                            (Operand::Constant(Constant::Int(cid)), Type::I64),
                            (Operand::Constant(Constant::Int(count)), Type::I64),
                        ] = args.as_slice()
                            && (0..=i64::from(i32::MAX)).contains(cid)
                            && usize::try_from(*count).is_ok()
                            && (name == "dartforge_object_new" || u16::try_from(*count).is_ok())
                        {
                            allocations.insert(*v);
                        }
                    }
                    _ => {}
                }
            }
        }
        if queries.is_empty() {
            return Self::default();
        }
        let cfg = Cfg::new(f);
        if !cfg.predecessors[0].is_empty() {
            return Self::default();
        }
        let mut witnesses = Vec::new();
        for (switch, block) in f.blocks.iter().enumerate() {
            let Terminator::Switch {
                val: Operand::Val(query),
                default,
                cases,
            } = &block.terminator
            else {
                continue;
            };
            let Some(&(receiver, query_block, query_position)) = queries.get(query) else {
                continue;
            };
            if !dominates(&cfg.idom, query_block, switch) {
                continue;
            }
            let Some((Type::Ref, definition)) = definitions.get(&receiver) else {
                continue;
            };
            if let Some((b, position)) = definition
                && (!dominates(&cfg.idom, *b, query_block)
                    || (*b == query_block && *position >= query_position))
            {
                continue;
            }
            let mut seen_cids = HashSet::new();
            if cases.iter().any(|(cid, _)| !seen_cids.insert(*cid)) {
                continue;
            }
            for (cid, target) in cases {
                let Ok(cid) = u32::try_from(*cid) else {
                    continue;
                };
                if i32::try_from(cid).is_err()
                    || target == default
                    || cases.iter().filter(|(_, other)| other == target).count() != 1
                {
                    continue;
                }
                let entry = positions[target];
                if cfg.predecessors[entry].as_slice() != [switch] || !cfg.reachable(entry) {
                    continue;
                }
                witnesses.push((receiver, entry, cid));
            }
        }
        Self {
            positions,
            idom: cfg.idom,
            witnesses,
            allocations,
        }
    }

    pub(super) fn refine(
        &self,
        origin: &mut Origin,
        receiver: &Operand,
        block: BlockId,
        classes: &HashMap<ValueId, u32>,
    ) {
        let Operand::Val(receiver) = receiver else {
            return;
        };
        let Some(&block) = self.positions.get(&block) else {
            return;
        };
        for &(value, entry, cid) in &self.witnesses {
            if value != *receiver || !dominates(&self.idom, entry, block) {
                continue;
            }
            // Formal receiver hints describe ancestry, not a physical allocation.
            // Unknown candidates keep their runtime guard and ownership contract.
            origin.sites.retain(|site| {
                !self.allocations.contains(site)
                    || classes.get(site).is_none_or(|class| *class == cid)
            });
        }
    }
}

#[cfg(test)]
mod tests;
