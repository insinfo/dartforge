use super::*;
use crate::optimize::cfg::{Cfg, successors};

#[derive(Default)]
pub(super) struct Cases<'a> {
    positions: HashMap<BlockId, usize>,
    idom: Vec<usize>,
    witnesses: Vec<(ValueId, usize, Restriction)>,
    allocations: HashSet<ValueId>,
    ancestry: Option<&'a PhysicalAncestry>,
}

#[derive(Clone, Copy)]
enum Restriction {
    Exact(u32),
    Subtype(u32),
}

type Definitions = HashMap<ValueId, (Type, Option<(usize, usize)>)>;

fn available(
    definitions: &Definitions,
    idom: &[usize],
    value: ValueId,
    consumer: (usize, usize),
) -> bool {
    match definitions.get(&value) {
        Some((_, None)) => true,
        Some((_, Some((block, position)))) => {
            dominates(idom, *block, consumer.0) && (*block != consumer.0 || *position < consumer.1)
        }
        None => false,
    }
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

impl<'a> Cases<'a> {
    #[cfg(test)]
    pub(super) fn new(f: &Function, canonical_runtime: bool) -> Self {
        Self::with_ancestry(f, canonical_runtime, None)
    }

    pub(super) fn with_ancestry(
        f: &Function,
        canonical_runtime: bool,
        ancestry: Option<&'a PhysicalAncestry>,
    ) -> Self {
        let ancestry = ancestry.filter(|ancestry| !ancestry.complete.is_empty());
        if !canonical_runtime
            || f.blocks.is_empty()
            || !f.blocks.iter().any(|b| {
                matches!(b.terminator, Terminator::Switch { .. })
                    || (ancestry.is_some() && matches!(b.terminator, Terminator::CondBranch { .. }))
            })
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
        let mut instructions = HashMap::new();
        let mut allocations = HashSet::new();
        for (b, block) in f.blocks.iter().enumerate() {
            for (position, (v, instruction, ty)) in block.instructions.iter().enumerate() {
                if definitions.insert(*v, (*ty, Some((b, position)))).is_some() {
                    return Self::default();
                }
                instructions.insert(*v, instruction);
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
        queries.retain(|_, query| {
            let (receiver, block, position) = *query;
            definitions
                .get(&receiver)
                .is_some_and(|(ty, _)| *ty == Type::Ref)
                && available(&definitions, &cfg.idom, receiver, (block, position))
        });
        let mut subtype_queries = HashMap::new();
        if ancestry.is_some() {
            for (value, instruction) in &instructions {
                let Instruction::CallRuntime {
                    name,
                    args,
                    ret_ty: Type::I8,
                } = instruction
                else {
                    continue;
                };
                if name != "dartforge_is_subclass" || definitions[value].0 != Type::I8 {
                    continue;
                }
                let [
                    (Operand::Val(query), Type::I64),
                    (Operand::Constant(Constant::Int(target)), Type::I64),
                ] = args.as_slice()
                else {
                    continue;
                };
                let Some(&(receiver, _, _)) = queries.get(query) else {
                    continue;
                };
                let Ok(target) = u32::try_from(*target) else {
                    continue;
                };
                let Some(location) = definitions[value].1 else {
                    continue;
                };
                if i32::try_from(target).is_ok()
                    && available(&definitions, &cfg.idom, *query, location)
                {
                    subtype_queries.insert(*value, (receiver, target));
                }
            }
        }
        let mut witnesses = Vec::new();
        for (switch, block) in f.blocks.iter().enumerate() {
            if let Terminator::CondBranch {
                cond,
                then_block,
                else_block,
            } = &block.terminator
                && then_block != else_block
                && ancestry.is_some()
            {
                let entry = positions[then_block];
                if cfg.predecessors[entry].as_slice() == [switch] && cfg.reachable(entry) {
                    let predicates = predicates::positive(
                        cond,
                        (switch, block.instructions.len()),
                        &definitions,
                        &instructions,
                        &cfg.idom,
                        &subtype_queries,
                    );
                    witnesses.extend(
                        predicates.into_iter().map(|(receiver, target)| {
                            (receiver, entry, Restriction::Subtype(target))
                        }),
                    );
                }
            }
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
            if !dominates(&cfg.idom, query_block, switch)
                || !available(
                    &definitions,
                    &cfg.idom,
                    *query,
                    (switch, block.instructions.len()),
                )
            {
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
                witnesses.push((receiver, entry, Restriction::Exact(cid)));
            }
        }
        Self {
            positions,
            idom: cfg.idom,
            witnesses,
            allocations,
            ancestry,
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
        for &(value, entry, restriction) in &self.witnesses {
            if value != *receiver || !dominates(&self.idom, entry, block) {
                continue;
            }
            // Formal receiver hints describe ancestry, not a physical allocation.
            // Unknown candidates keep their runtime guard and ownership contract.
            origin.sites.retain(|site| {
                !self.allocations.contains(site)
                    || classes.get(site).is_none_or(|class| match restriction {
                        Restriction::Exact(cid) => *class == cid,
                        Restriction::Subtype(target) => !self
                            .ancestry
                            .is_some_and(|ancestry| ancestry.excludes(*class, target)),
                    })
            });
        }
    }
}

#[cfg(test)]
mod tests;

mod predicates;

#[cfg(test)]
mod ancestry_tests;
