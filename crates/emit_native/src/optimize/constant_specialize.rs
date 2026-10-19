//! Bounded static-call specialization without changing parameter ABI or evaluation order.
use super::{mem2reg, operands::*, representation_constants, scc, simplify};
use crate::hir::*;
use std::collections::{BTreeMap, HashMap, HashSet};

mod enum_dispatch;

const MAX_CLONES: usize = 32;
const MAX_PER_TARGET: usize = 4;
const MAX_GROWTH: usize = 2048;
// Analysis may inspect a larger input than the inliner admits. Only a proven
// reduced clone is published, still subject to the independent growth budget.
const MAX_INPUT_BODY: usize = 8192;
const PREFIX: &str = "df.specialized.";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Scalar {
    Int(i64),
    Double(u64),
    Bool(bool),
}
impl Scalar {
    fn read(value: &Constant, ty: Type) -> Option<Self> {
        match (value, ty) {
            (Constant::Int(value), Type::I64) => Some(Self::Int(*value)),
            (Constant::Double(value), Type::F64) => Some(Self::Double(value.to_bits())),
            (Constant::Bool(value), Type::I1) => Some(Self::Bool(*value)),
            _ => None,
        }
    }
    fn operand(&self) -> Operand {
        Operand::Constant(match self {
            Self::Int(value) => Constant::Int(*value),
            Self::Double(bits) => Constant::Double(f64::from_bits(*bits)),
            Self::Bool(value) => Constant::Bool(*value),
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
enum Argument {
    Scalar(Scalar),
    Enum(super::enum_constants::EnumToken),
}
impl Argument {
    fn key(&self) -> String {
        match self {
            Self::Scalar(value) => format!("scalar:{value:?}"),
            Self::Enum(value) => format!("enum:{}", value.key()),
        }
    }
}

pub(super) fn is_specialized_symbol(symbol: &str) -> bool {
    symbol.starts_with(PREFIX)
}

fn size(function: &Function) -> usize {
    function
        .blocks
        .iter()
        .map(|b| b.instructions.len() + 1)
        .sum()
}
fn branches(function: &Function) -> usize {
    function
        .blocks
        .iter()
        .filter(|b| {
            matches!(
                b.terminator,
                Terminator::CondBranch { .. } | Terminator::Switch { .. }
            )
        })
        .count()
}
fn types(function: &Function) -> Option<HashMap<ValueId, Type>> {
    if function.blocks.first()?.id != BlockId(0) {
        return None;
    }
    let mut values = HashMap::new();
    for (value, _, ty) in &function.params {
        if values.insert(*value, *ty).is_some() {
            return None;
        }
    }
    let mut blocks = HashSet::new();
    for block in &function.blocks {
        if !blocks.insert(block.id) {
            return None;
        }
        for (value, instruction, ty) in &block.instructions {
            if values
                .insert(
                    *value,
                    crate::llvm::LlvmEmitter::result_type(instruction, *ty),
                )
                .is_some()
            {
                return None;
            }
        }
    }
    let mut valid = true;
    for block in &function.blocks {
        valid &= super::cfg::successors(&block.terminator)
            .iter()
            .all(|id| blocks.contains(id));
        let mut check = |operand: &Operand| {
            if let Operand::Val(value) = operand {
                valid &= values.contains_key(value);
            }
        };
        for (_, instruction, _) in &block.instructions {
            operands(instruction, &mut check);
        }
        terminator_operands(&block.terminator, &mut check);
    }
    valid.then_some(values)
}
fn literals(function: &Function, types: &HashMap<ValueId, Type>) -> HashMap<ValueId, Scalar> {
    let mut facts = HashMap::new();
    for (value, instruction, ty) in function.blocks.iter().flat_map(|b| &b.instructions) {
        if types.get(value) != Some(ty) {
            continue;
        }
        if let Instruction::Const(constant) = instruction
            && let Some(value_fact) = Scalar::read(constant, *ty)
        {
            facts.insert(*value, value_fact);
        }
    }
    loop {
        let before = facts.len();
        for (value, instruction, ty) in function.blocks.iter().flat_map(|b| &b.instructions) {
            if facts.contains_key(value) || types.get(value) != Some(ty) {
                continue;
            }
            if let Instruction::FNeg(operand) = instruction {
                let fact = match operand {
                    Operand::Constant(constant) => Scalar::read(constant, Type::F64),
                    Operand::Val(value) if types.get(value) == Some(&Type::F64) => {
                        facts.get(value).cloned()
                    }
                    _ => None,
                };
                if *ty == Type::F64
                    && let Some(Scalar::Double(bits)) = fact
                {
                    facts.insert(*value, Scalar::Double(bits ^ (1 << 63)));
                }
                continue;
            }
            let Instruction::Phi {
                incoming,
                ty: phi_ty,
            } = instruction
            else {
                continue;
            };
            if phi_ty != ty || incoming.is_empty() {
                continue;
            }
            let read = |operand: &Operand| match operand {
                Operand::Constant(constant) => Scalar::read(constant, *ty),
                Operand::Val(id) => facts.get(id).and_then(|fact| {
                    let Operand::Constant(constant) = fact.operand() else {
                        return None;
                    };
                    Scalar::read(&constant, *ty)
                }),
            };
            let Some(first) = read(&incoming[0].1) else {
                continue;
            };
            if incoming
                .iter()
                .all(|(_, operand)| read(operand).as_ref() == Some(&first))
            {
                facts.insert(*value, first);
            }
        }
        if before == facts.len() {
            break;
        }
    }
    facts
}
fn cleanup(function: &mut Function) {
    mem2reg::promote(function);
    representation_constants::propagate_literals(function);
    simplify::fold_constants(function);
    simplify::remove_unreachable(function);
}

/// Appends profitable context-specific bodies and retargets only their proved callsites.
pub(super) fn rewrite(module: &mut Module) -> usize {
    if module.arc_memory
        || module.arc_prepared
        || module.arc_aot_ports.is_some()
        || module.sdk_library
        || module.dwarf
        || module.table_exceptions
        || std::env::var("DARTFORGE_CONSTANT_SPECIALIZATION").as_deref() != Ok("1")
    {
        return 0;
    }
    rewrite_enabled(module)
}
fn rewrite_enabled(module: &mut Module) -> usize {
    if module.arc_memory
        || module.arc_prepared
        || module.arc_aot_ports.is_some()
        || module.sdk_library
        || module.dwarf
        || module.table_exceptions
    {
        return 0;
    }
    for function in &mut module.functions {
        if function.debug_info.is_none() && types(function).is_some() {
            cleanup(function);
        }
    }
    let mut index = HashMap::new();
    for (position, function) in module.functions.iter().enumerate() {
        index
            .entry(function.symbol.clone())
            .and_modify(|entry| *entry = None)
            .or_insert(Some(position));
    }
    let edges: Vec<Vec<usize>> = module
        .functions
        .iter()
        .map(|function| {
            function
                .blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .filter_map(|(_, instruction, _)| {
                    let Instruction::CallStatic { symbol, .. } = instruction else {
                        return None;
                    };
                    index.get(symbol).copied().flatten()
                })
                .collect()
        })
        .collect();
    let recursive: HashSet<usize> = scc::components(&edges)
        .into_iter()
        .filter(|group| group.len() > 1 || edges[group[0]].contains(&group[0]))
        .flatten()
        .collect();
    let inventory = super::enum_constants::Inventory::new(module);
    type Key = (String, Vec<Option<String>>);
    type Calls = Vec<(usize, usize, usize)>;
    let mut candidates: BTreeMap<Key, (Vec<Option<Argument>>, Calls)> = BTreeMap::new();
    for (caller_index, caller) in module.functions.iter().enumerate() {
        if caller.debug_info.is_some() {
            continue;
        }
        let Some(caller_types) = types(caller) else {
            continue;
        };
        let facts = literals(caller, &caller_types);
        for (block_index, block) in caller.blocks.iter().enumerate() {
            for (instruction_index, (_, instruction, output_ty)) in
                block.instructions.iter().enumerate()
            {
                let Instruction::CallStatic {
                    symbol,
                    args,
                    ret_ty,
                } = instruction
                else {
                    continue;
                };
                let Some(target_index) = index.get(symbol).copied().flatten() else {
                    continue;
                };
                let target = &module.functions[target_index];
                if recursive.contains(&target_index)
                    || is_specialized_symbol(symbol)
                    || target.debug_info.is_some()
                    || types(target).is_none()
                    || module.inline_policies.get(symbol) == Some(&InlinePolicy::Never)
                    || target.params.len() != args.len()
                    || target.return_ty != *ret_ty
                    || *output_ty != *ret_ty
                    || size(target) > MAX_INPUT_BODY
                    || module
                        .source_errors
                        .iter()
                        .any(|(name, errors)| name == symbol && !errors.is_empty())
                {
                    continue;
                }
                let mut compatible = true;
                let argument_facts: Vec<_> = args
                    .iter()
                    .zip(&target.params)
                    .map(|(argument, (_, _, ty))| match argument {
                        Operand::Constant(value) => {
                            let fact = Scalar::read(value, *ty).map(Argument::Scalar);
                            compatible &= fact.is_some()
                                || (*ty == Type::Ref
                                    && matches!(
                                        value,
                                        Constant::Null
                                            | Constant::String(_)
                                            | Constant::StringWtf8(_)
                                    ));
                            fact
                        }
                        Operand::Val(value) => {
                            compatible &= caller_types.get(value) == Some(ty);
                            facts.get(value).cloned().map(Argument::Scalar).or_else(|| {
                                (*ty == Type::Ref)
                                    .then(|| inventory.token_for_call(caller, *value))
                                    .flatten()
                                    .map(Argument::Enum)
                            })
                        }
                    })
                    .collect();
                if compatible && argument_facts.iter().any(Option::is_some) {
                    let key = (
                        symbol.clone(),
                        argument_facts
                            .iter()
                            .map(|fact| fact.as_ref().map(Argument::key))
                            .collect(),
                    );
                    let candidate = candidates
                        .entry(key)
                        .or_insert_with(|| (argument_facts.clone(), vec![]));
                    if candidate.0 == argument_facts {
                        candidate
                            .1
                            .push((caller_index, block_index, instruction_index));
                    } else {
                        candidate.1.clear();
                    }
                }
            }
        }
    }
    let mut total = module
        .functions
        .iter()
        .filter(|f| is_specialized_symbol(&f.symbol))
        .count();
    let mut growth: usize = module
        .functions
        .iter()
        .filter(|f| is_specialized_symbol(&f.symbol))
        .map(size)
        .sum();
    let mut per_target = HashMap::<String, usize>::new();
    let mut plans = Vec::new();
    let mut names: HashSet<_> = module.functions.iter().map(|f| f.symbol.clone()).collect();
    for ((symbol, _), (arguments, calls)) in candidates {
        if total >= MAX_CLONES {
            break;
        }
        if calls.is_empty() {
            continue;
        }
        let owner_prefix = format!("{PREFIX}{}.", blake3::hash(symbol.as_bytes()).to_hex());
        let existing = module
            .functions
            .iter()
            .filter(|f| f.symbol.starts_with(&owner_prefix))
            .count();
        if existing + per_target.get(&symbol).copied().unwrap_or(0) >= MAX_PER_TARGET {
            continue;
        }
        let target = &module.functions[index[&symbol].expect("unique target")];
        let mut clone = target.clone();
        let replacements: HashMap<_, _> = clone
            .params
            .iter()
            .zip(&arguments)
            .filter_map(|((value, _, _), fact)| match fact {
                Some(Argument::Scalar(fact)) => Some((*value, fact.operand())),
                _ => None,
            })
            .collect();
        let enum_formals: HashMap<_, _> = clone
            .params
            .iter()
            .zip(&arguments)
            .filter_map(|((value, _, _), fact)| match fact {
                Some(Argument::Enum(token)) => Some((*value, token.clone())),
                _ => None,
            })
            .collect();
        for block in &mut clone.blocks {
            let mut replace = |operand: &mut Operand| {
                if let Operand::Val(value) = operand
                    && let Some(literal) = replacements.get(value)
                {
                    *operand = literal.clone();
                }
            };
            for (_, instruction, _) in &mut block.instructions {
                operands_mut(instruction, &mut replace);
            }
            terminator_operands_mut(&mut block.terminator, &mut replace);
        }
        inventory.fold_identity_comparisons(&mut clone, &enum_formals);
        cleanup(&mut clone);
        enum_dispatch::guard_switches(&mut clone, &enum_formals, &target.symbol);
        cleanup(&mut clone);
        if branches(&clone) >= branches(target)
            || size(&clone) * 4 > size(target) * 3
            || growth + size(&clone) > MAX_GROWTH
        {
            continue;
        }
        let mut hash = blake3::Hasher::new();
        for argument in &arguments {
            let key = argument.as_ref().map(Argument::key).unwrap_or_default();
            hash.update(&(key.len() as u64).to_le_bytes());
            hash.update(key.as_bytes());
        }
        let clone_symbol = format!("{owner_prefix}{}", hash.finalize().to_hex());
        if !names.insert(clone_symbol.clone()) {
            continue;
        }
        clone.name = target.stack_trace_name();
        clone.symbol = clone_symbol;
        growth += size(&clone);
        *per_target.entry(symbol.clone()).or_default() += 1;
        total += 1;
        plans.push((symbol, clone, replacements, calls));
    }
    drop(inventory);
    let count = plans.len();
    for (symbol, clone, replacements, calls) in plans {
        let clone_symbol = clone.symbol.clone();
        if let Some(policy) = module.inline_policies.get(&symbol).copied() {
            module.inline_policies.insert(clone_symbol.clone(), policy);
        }
        if let Some(parameters) = module.dart_scalar_parameters.get(&symbol).cloned() {
            module
                .dart_scalar_parameters
                .insert(clone_symbol.clone(), parameters);
        }
        if let Some(parameters) = module.dart_rti_parameters.get(&symbol).cloned() {
            module
                .dart_rti_parameters
                .insert(clone_symbol.clone(), parameters);
        }
        if let Some(parameters) = module.dart_vector_parameters.get(&symbol).cloned() {
            module
                .dart_vector_parameters
                .insert(clone_symbol.clone(), parameters);
        }
        if module.dart_ref_returns.contains(&symbol) {
            module.dart_ref_returns.insert(clone_symbol.clone());
        }
        if let Some(&(receiver, class)) = module.pruning_this_receivers.get(&symbol)
            && !replacements.contains_key(&receiver)
        {
            module
                .pruning_this_receivers
                .insert(clone_symbol.clone(), (receiver, class));
        }
        for (caller, block, instruction) in calls {
            if let Instruction::CallStatic { symbol, .. } =
                &mut module.functions[caller].blocks[block].instructions[instruction].1
            {
                *symbol = clone_symbol.clone();
            }
        }
        module.functions.push(clone);
    }
    count
}

#[cfg(test)]
mod tests;
