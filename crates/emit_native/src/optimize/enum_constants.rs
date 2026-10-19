//! Successful canonical enum handles for symbolic argument specialization.
//!
//! A declaration is only a premise. The inventory audits the current lazy getter,
//! every global writer and the caller's pending-check success dominance. Tokens
//! never replace real Ref operands or remove lazy getter evaluation/exception checks.
use super::cfg::Cfg;
use super::operands::{operands, terminator_operands};
use crate::hir::*;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct EnumToken {
    pub(crate) library_uri: String,
    pub(crate) unit_uri: String,
    pub(crate) owner: String,
    pub(crate) declaration: u32,
    pub(crate) index: usize,
    pub(crate) getter: String,
}
impl EnumToken {
    pub(crate) fn key(&self) -> String {
        format!(
            "{:?}",
            (
                &self.library_uri,
                &self.unit_uri,
                &self.owner,
                self.declaration,
                self.index,
                &self.getter
            )
        )
    }
}

pub(crate) struct Inventory<'a> {
    getters: HashMap<String, EnumToken>,
    callers: HashMap<String, &'a Function>,
    cached_values: HashMap<String, EnumToken>,
    calls: HashMap<String, HashMap<ValueId, EnumToken>>,
}
impl<'a> Inventory<'a> {
    pub(crate) fn new(module: &'a Module) -> Self {
        let mut result = Self {
            getters: HashMap::new(),
            callers: HashMap::new(),
            cached_values: HashMap::new(),
            calls: HashMap::new(),
        };
        if !module.arc_fixed_ancestry
            || module.arc_memory
            || module.arc_prepared
            || module.arc_aot_ports.is_some()
            || !crate::pruning::receiver_provenance::registry::closed(&[module])
        {
            return result;
        }
        let mut functions = HashMap::new();
        let mut duplicates = HashSet::new();
        let mut writers: HashMap<&str, Vec<&Function>> = HashMap::new();
        let mut exposed = false;
        for function in &module.functions {
            if functions
                .insert(function.symbol.as_str(), function)
                .is_some()
            {
                duplicates.insert(function.symbol.as_str());
            }
            for (_, instruction, _) in function.blocks.iter().flat_map(|block| &block.instructions)
            {
                match instruction {
                    Instruction::StoreGlobal { symbol, .. } => {
                        writers.entry(symbol).or_default().push(function)
                    }
                    Instruction::NativeCall { .. }
                    | Instruction::CompositeNativeCall { .. }
                    | Instruction::ArcStoreStrong { .. } => exposed = true,
                    Instruction::CallRuntime { name, .. }
                    | Instruction::CallStatic { symbol: name, .. }
                        if matches!(
                            name.as_str(),
                            "dartforge_globals_area"
                                | "dartforge_globals_area_id"
                                | "dartforge_ffi_native_symbol"
                        ) =>
                    {
                        exposed = true
                    }
                    _ => {}
                }
            }
        }
        if exposed
            || [
                "dartforge_object_new",
                "dartforge_object_set",
                "dartforge_mark_constant",
                "dartforge_exception_pending",
            ]
            .iter()
            .any(|name| functions.contains_key(name))
        {
            return result;
        }
        for (getter, premise) in &module.simple_enum_constants {
            if premise.getter != *getter
                || duplicates.contains(getter.as_str())
                || module
                    .pruning_ambiguous_class_ids
                    .contains(&premise.class_id)
            {
                continue;
            }
            let Some(&function) = functions.get(getter.as_str()) else {
                continue;
            };
            if module
                .classes
                .iter()
                .filter(|class| class.id == premise.class_id && class.field_count == 2)
                .count()
                != 1
                || module
                    .classes
                    .iter()
                    .filter(|class| class.id == premise.class_id)
                    .count()
                    != 1
            {
                continue;
            }
            let flag = format!("{}$ok", premise.global);
            if module
                .globals
                .iter()
                .filter(|(root, ty, global)| {
                    *root == premise.root && *ty == Type::Ref && *global == premise.global
                })
                .count()
                != 1
                || module
                    .globals
                    .iter()
                    .filter(|(root, ty, global)| {
                        (*root == premise.root && *ty == Type::Ref) || *global == premise.global
                    })
                    .count()
                    != 1
                || module
                    .globals
                    .iter()
                    .any(|(_, ty, global)| *global == flag && *ty != Type::I8)
            {
                continue;
            }
            if ![premise.global.as_str(), flag.as_str()]
                .iter()
                .all(|global| {
                    writers
                        .get(global)
                        .is_some_and(|w| w.len() == 1 && std::ptr::eq(w[0], function))
                })
            {
                continue;
            }
            if !getter_body(function, premise) {
                continue;
            }
            result.getters.insert(
                getter.clone(),
                EnumToken {
                    library_uri: premise.library_uri.clone(),
                    unit_uri: premise.unit_uri.clone(),
                    owner: premise.owner.clone(),
                    declaration: premise.declaration,
                    index: premise.index,
                    getter: getter.clone(),
                },
            );
        }
        for (getter, token) in &result.getters {
            result.cached_values.insert(
                module.simple_enum_constants[getter].global.clone(),
                token.clone(),
            );
        }
        for function in &module.functions {
            if duplicates.contains(function.symbol.as_str()) {
                continue;
            }
            result.calls.insert(
                function.symbol.clone(),
                successful_values(function, &result.getters, &result.cached_values),
            );
            result.callers.insert(function.symbol.clone(), function);
        }
        result
    }
    pub(crate) fn token_for_call(&self, function: &Function, value: ValueId) -> Option<EnumToken> {
        self.callers
            .get(&function.symbol)
            .filter(|body| std::ptr::eq(**body, function))?;
        self.calls.get(&function.symbol)?.get(&value).cloned()
    }
    /// Fold only existing Ref identity instructions. Calls and ABI parameters
    /// stay present; generic equality and field/argument validation remain untouched.
    pub(crate) fn fold_identity_comparisons(
        &self,
        function: &mut Function,
        formals: &HashMap<ValueId, EnumToken>,
    ) -> usize {
        let mut facts = successful_values(function, &self.getters, &self.cached_values);
        for (value, token) in formals {
            if function
                .params
                .iter()
                .any(|(parameter, _, ty)| parameter == value && *ty == Type::Ref)
                && self.getters.get(&token.getter) == Some(token)
            {
                facts.insert(*value, token.clone());
            }
        }
        let mut changed = 0;
        for (_, instruction, _) in function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
        {
            if let Instruction::ICmp(
                operation @ (ICmpOp::Eq | ICmpOp::Ne),
                Operand::Val(left),
                Operand::Val(right),
            ) = instruction
                && let (Some(left), Some(right)) = (facts.get(left), facts.get(right))
            {
                let equal = left == right;
                *instruction = Instruction::Const(Constant::Bool(if *operation == ICmpOp::Eq {
                    equal
                } else {
                    !equal
                }));
                changed += 1;
            }
        }
        changed
    }
}

fn getter_body(function: &Function, premise: &SimpleEnumConstant) -> bool {
    if !function.params.is_empty() || function.return_ty != Type::Ref || function.blocks.len() != 3
    {
        return false;
    }
    let ids: HashSet<_> = function.blocks.iter().map(|block| block.id).collect();
    let values: Vec<_> = function
        .blocks
        .iter()
        .flat_map(|block| block.instructions.iter().map(|(value, _, _)| *value))
        .collect();
    if ids.len() != 3 || values.iter().collect::<HashSet<_>>().len() != values.len() {
        return false;
    }
    let flag = format!("{}$ok", premise.global);
    let entry = &function.blocks[0];
    let [
        (
            loaded,
            Instruction::LoadGlobal {
                symbol,
                ty: Type::I8,
            },
            Type::I8,
        ),
        (
            condition,
            Instruction::ICmp(
                ICmpOp::Ne,
                Operand::Val(flag_value),
                Operand::Constant(Constant::Int(0)),
            ),
            Type::I1,
        ),
    ] = entry.instructions.as_slice()
    else {
        return false;
    };
    if symbol != &flag || loaded != flag_value {
        return false;
    }
    let Terminator::CondBranch {
        cond: Operand::Val(branch),
        then_block,
        else_block,
    } = &entry.terminator
    else {
        return false;
    };
    if branch != condition {
        return false;
    }
    let Some(warm) = function.blocks.iter().find(|b| b.id == *then_block) else {
        return false;
    };
    let Some(cold) = function.blocks.iter().find(|b| b.id == *else_block) else {
        return false;
    };
    let [
        (
            value,
            Instruction::LoadGlobal {
                symbol,
                ty: Type::Ref,
            },
            Type::Ref,
        ),
    ] = warm.instructions.as_slice()
    else {
        return false;
    };
    if symbol != &premise.global
        || !matches!(&warm.terminator, Terminator::Return(Some(Operand::Val(returned))) if returned == value)
    {
        return false;
    }
    let instructions = &cold.instructions;
    if instructions.len() != 7
        || [0, 2, 3, 4, 6]
            .iter()
            .any(|index| instructions[*index].2 != Type::Void)
    {
        return false;
    }
    if !matches!(&instructions[0].1, Instruction::StoreGlobal { symbol, val: Operand::Constant(Constant::Int(1)), ty: Type::I8, root: None } if symbol == &flag)
    {
        return false;
    }
    let object = Operand::Val(instructions[1].0);
    if !runtime(
        &instructions[1].1,
        "dartforge_object_new",
        &[
            (
                Operand::Constant(Constant::Int(i64::from(premise.class_id))),
                Type::I64,
            ),
            (Operand::Constant(Constant::Int(2)), Type::I64),
        ],
        Type::Ref,
    ) || instructions[1].2 != Type::Ref
    {
        return false;
    }
    if !matches!(&instructions[2].1, Instruction::StoreGlobal { symbol, val, ty: Type::Ref, root: Some(root) } if symbol == &premise.global && val == &object && *root == premise.root)
    {
        return false;
    }
    if !runtime(
        &instructions[3].1,
        "dartforge_mark_constant",
        &[
            (object.clone(), Type::Ref),
            (
                Operand::Constant(Constant::Function(function.symbol.clone())),
                Type::I64,
            ),
        ],
        Type::Void,
    ) {
        return false;
    }
    if !runtime(
        &instructions[4].1,
        "dartforge_object_set",
        &[
            (object.clone(), Type::Ref),
            (Operand::Constant(Constant::Int(0)), Type::I64),
            (
                Operand::Constant(Constant::Int(premise.index as i64)),
                Type::I64,
            ),
            (Operand::Constant(Constant::Int(0)), Type::I8),
        ],
        Type::Void,
    ) {
        return false;
    }
    if !matches!(&instructions[5].1, Instruction::Const(Constant::String(name)) if name == &premise.name)
        || instructions[5].2 != Type::Ref
    {
        return false;
    }
    if !runtime(
        &instructions[6].1,
        "dartforge_object_set",
        &[
            (object.clone(), Type::Ref),
            (Operand::Constant(Constant::Int(1)), Type::I64),
            (Operand::Val(instructions[5].0), Type::I64),
            (Operand::Constant(Constant::Int(1)), Type::I8),
        ],
        Type::Void,
    ) {
        return false;
    }
    matches!(&cold.terminator, Terminator::Return(Some(value)) if value == &object)
}
fn runtime(instruction: &Instruction, name: &str, args: &[(Operand, Type)], ty: Type) -> bool {
    matches!(instruction, Instruction::CallRuntime { name: actual, args: observed, ret_ty } if actual == name && observed == args && *ret_ty == ty)
}

fn dominates(cfg: &Cfg, dominator: usize, mut block: usize) -> bool {
    if !cfg.reachable(block) {
        return false;
    }
    loop {
        if block == dominator {
            return true;
        }
        let parent = cfg.idom[block];
        if parent == block || parent == usize::MAX {
            return false;
        }
        block = parent;
    }
}
fn successful_values(
    function: &Function,
    getters: &HashMap<String, EnumToken>,
    cached_values: &HashMap<String, EnumToken>,
) -> HashMap<ValueId, EnumToken> {
    let mut result = HashMap::new();
    if function.blocks.is_empty() || function.blocks.len() > 4096 {
        return result;
    }
    let positions: HashMap<_, _> = function
        .blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id, i))
        .collect();
    if positions.len() != function.blocks.len()
        || function.blocks.iter().any(|block| {
            super::cfg::successors(&block.terminator)
                .iter()
                .any(|id| !positions.contains_key(id))
        })
    {
        return result;
    }
    // Specialized clones may be analyzed before rejoining the module. Never
    // carry the module's cache audit across a newly introduced writer/exposure.
    if function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .any(|(_, instruction, _)| match instruction {
            Instruction::StoreGlobal { symbol, .. } => {
                cached_values.contains_key(symbol)
                    || symbol
                        .strip_suffix("$ok")
                        .is_some_and(|global| cached_values.contains_key(global))
            }
            Instruction::NativeCall { .. }
            | Instruction::CompositeNativeCall { .. }
            | Instruction::ArcStoreStrong { .. } => true,
            Instruction::CallRuntime { name, .. }
            | Instruction::CallStatic { symbol: name, .. } => matches!(
                name.as_str(),
                "dartforge_globals_area"
                    | "dartforge_globals_area_id"
                    | "dartforge_ffi_native_symbol"
            ),
            _ => false,
        })
    {
        return result;
    }
    let cfg = Cfg::new(function);
    // A value is consumed on a phi's incoming edge, not in the join block.
    // Reject malformed SSA rather than manufacturing an availability proof.
    let mut definitions: HashSet<_> = function.params.iter().map(|(value, _, _)| *value).collect();
    if definitions.len() != function.params.len() {
        return result;
    }
    for (block_index, block) in function.blocks.iter().enumerate() {
        for (value, instruction, _) in &block.instructions {
            if !definitions.insert(*value) {
                return result;
            }
            if let Instruction::Phi { incoming, .. } = instruction {
                let edges: HashSet<_> = incoming.iter().map(|(edge, _)| *edge).collect();
                if edges.len() != incoming.len()
                    || edges.len() != cfg.predecessors[block_index].len()
                    || cfg.predecessors[block_index]
                        .iter()
                        .any(|p| !edges.contains(&function.blocks[*p].id))
                {
                    return result;
                }
            }
        }
    }
    let mut available = HashMap::new();
    for (block_index, block) in function.blocks.iter().enumerate() {
        if !cfg.reachable(block_index) {
            continue;
        }
        for (index, (value, instruction, ty)) in block.instructions.iter().enumerate() {
            let Instruction::CallStatic {
                symbol,
                args,
                ret_ty: Type::Ref,
            } = instruction
            else {
                continue;
            };
            let Some(token) = getters.get(symbol) else {
                continue;
            };
            if !args.is_empty() || *ty != Type::Ref {
                continue;
            }
            let [
                (
                    pending,
                    Instruction::CallRuntime {
                        name,
                        args: pending_args,
                        ret_ty: Type::I8,
                    },
                    Type::I8,
                ),
                (
                    condition,
                    Instruction::ICmp(
                        ICmpOp::Ne,
                        Operand::Val(pending_value),
                        Operand::Constant(Constant::Int(0)),
                    ),
                    Type::I1,
                ),
            ] = &block.instructions[index + 1..]
            else {
                continue;
            };
            if name != "dartforge_exception_pending"
                || !pending_args.is_empty()
                || pending != pending_value
            {
                continue;
            }
            let Terminator::CondBranch {
                cond: Operand::Val(branch),
                then_block,
                else_block,
            } = &block.terminator
            else {
                continue;
            };
            if branch != condition || then_block == else_block {
                continue;
            }
            let success = positions[else_block];
            if cfg.predecessors[success].as_slice() != [block_index] {
                continue;
            }
            if uses_available(function, &cfg, &positions, *value, success, None) {
                result.insert(*value, token.clone());
                available.insert(*value, (success, None));
            }
        }
    }
    // The warm path must test the audited cache's own initialization flag.
    // An unguarded read can still be null and is never an enum constant fact.
    for (block_index, block) in function.blocks.iter().enumerate() {
        if !cfg.reachable(block_index) || cfg.predecessors[block_index].len() != 1 {
            continue;
        }
        let parent = cfg.predecessors[block_index][0];
        let guard = &function.blocks[parent];
        let [
            ..,
            (
                flag,
                Instruction::LoadGlobal {
                    symbol: flag_name,
                    ty: Type::I8,
                },
                Type::I8,
            ),
            (
                condition,
                Instruction::ICmp(
                    ICmpOp::Eq,
                    Operand::Val(tested),
                    Operand::Constant(Constant::Int(1)),
                ),
                Type::I1,
            ),
        ] = guard.instructions.as_slice()
        else {
            continue;
        };
        if flag != tested
            || !matches!(&guard.terminator, Terminator::CondBranch { cond: Operand::Val(branch), then_block, else_block }
                if branch == condition && *then_block == block.id && else_block != then_block)
        {
            continue;
        }
        for (index, (value, instruction, ty)) in block.instructions.iter().enumerate() {
            let Instruction::LoadGlobal {
                symbol,
                ty: Type::Ref,
            } = instruction
            else {
                continue;
            };
            if *ty != Type::Ref || flag_name.strip_suffix("$ok") != Some(symbol.as_str()) {
                continue;
            }
            let Some(token) = cached_values.get(symbol) else {
                continue;
            };
            if uses_available(function, &cfg, &positions, *value, block_index, Some(index)) {
                result.insert(*value, token.clone());
                available.insert(*value, (block_index, Some(index)));
            }
        }
    }
    // Only merge identical proven handles. No optimistic facts seed cycles;
    // the bound limits work while leaving unresolved phis conservatively unknown.
    for _ in 0..function.blocks.len().min(128) {
        let mut changed = false;
        for &block_index in &cfg.rpo {
            let block = &function.blocks[block_index];
            for (index, (value, instruction, ty)) in block.instructions.iter().enumerate() {
                if result.contains_key(value) {
                    continue;
                }
                let Instruction::Phi {
                    incoming,
                    ty: Type::Ref,
                } = instruction
                else {
                    continue;
                };
                if *ty != Type::Ref || incoming.is_empty() {
                    continue;
                }
                let mut token = None;
                let mut valid = true;
                for (edge, operand) in incoming {
                    let Operand::Val(source) = operand else {
                        valid = false;
                        break;
                    };
                    let Some(source_token) = result.get(source) else {
                        valid = false;
                        break;
                    };
                    let (origin, definition) = available[source];
                    if !available_on_edge(&cfg, origin, definition, positions[edge], block_index)
                        || token.as_ref().is_some_and(|prior| prior != source_token)
                    {
                        valid = false;
                        break;
                    }
                    token = Some(source_token.clone());
                }
                if valid
                    && uses_available(function, &cfg, &positions, *value, block_index, Some(index))
                    && let Some(token) = token
                {
                    result.insert(*value, token);
                    available.insert(*value, (block_index, Some(index)));
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    result
}

#[cfg(test)]
mod tests;

/// Check ordinary uses at their instruction and phi uses at the predecessor exit.
fn uses_available(
    function: &Function,
    cfg: &Cfg,
    positions: &HashMap<BlockId, usize>,
    value: ValueId,
    origin: usize,
    definition: Option<usize>,
) -> bool {
    let mut valid = true;
    for (block_index, block) in function.blocks.iter().enumerate() {
        for (index, (_, instruction, _)) in block.instructions.iter().enumerate() {
            if let Instruction::Phi { incoming, .. } = instruction {
                for (edge, operand) in incoming {
                    if *operand == Operand::Val(value)
                        && !available_on_edge(cfg, origin, definition, positions[edge], block_index)
                    {
                        valid = false;
                    }
                }
            } else {
                operands(instruction, &mut |operand| {
                    if *operand == Operand::Val(value)
                        && (!dominates(cfg, origin, block_index)
                            || (origin == block_index
                                && definition.is_some_and(|defined| index <= defined)))
                    {
                        valid = false;
                    }
                });
            }
        }
        terminator_operands(&block.terminator, &mut |operand| {
            if *operand == Operand::Val(value) && !dominates(cfg, origin, block_index) {
                valid = false;
            }
        });
    }
    valid
}

/// A pending check proves the success edge itself, including a phi directly at
/// its destination. Ordinary definitions must already dominate the predecessor.
fn available_on_edge(
    cfg: &Cfg,
    origin: usize,
    definition: Option<usize>,
    predecessor: usize,
    destination: usize,
) -> bool {
    dominates(cfg, origin, predecessor)
        || (definition.is_none()
            && origin == destination
            && cfg.predecessors[origin].as_slice() == [predecessor])
}
