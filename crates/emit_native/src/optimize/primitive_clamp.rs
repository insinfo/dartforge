//! Guarded primitive clamp with existing Ref identity and selector checks intact.
use crate::hir::*;
use std::collections::{HashMap, HashSet};

const MAX_SITES: usize = 128;

fn dominates(cfg: &super::cfg::Cfg, definition: usize, mut use_block: usize) -> bool {
    if !cfg.reachable(use_block) {
        return false;
    }
    loop {
        if use_block == definition {
            return true;
        }
        let parent = cfg.idom[use_block];
        if parent == use_block || parent == usize::MAX {
            return false;
        }
        use_block = parent;
    }
}

struct Definitions<'a> {
    instructions: HashMap<ValueId, (&'a Instruction, Type)>,
    positions: HashMap<ValueId, (usize, usize)>,
    parameters: HashMap<ValueId, Type>,
    cfg: super::cfg::Cfg,
}
impl<'a> Definitions<'a> {
    fn new(function: &'a Function) -> Option<Self> {
        if function.blocks.is_empty() {
            return None;
        }
        let blocks: HashSet<_> = function.blocks.iter().map(|b| b.id).collect();
        if blocks.len() != function.blocks.len()
            || function.blocks.iter().any(|b| {
                super::cfg::successors(&b.terminator)
                    .iter()
                    .any(|id| !blocks.contains(id))
            })
        {
            return None;
        }
        let mut instructions = HashMap::new();
        let mut positions = HashMap::new();
        let mut parameters = HashMap::new();
        for (value, _, ty) in &function.params {
            if parameters.insert(*value, *ty).is_some() {
                return None;
            }
        }
        for (block, b) in function.blocks.iter().enumerate() {
            for (index, (value, instruction, ty)) in b.instructions.iter().enumerate() {
                if parameters.contains_key(value)
                    || instructions.insert(*value, (instruction, *ty)).is_some()
                {
                    return None;
                }
                positions.insert(*value, (block, index));
            }
        }
        Some(Self {
            instructions,
            positions,
            parameters,
            cfg: super::cfg::Cfg::new(function),
        })
    }
    fn available(&self, operand: &Operand, ty: Type, block: usize, index: usize) -> bool {
        match operand {
            Operand::Val(value) => {
                if let Some(actual) = self.parameters.get(value) {
                    return *actual == ty;
                }
                let Some((_, actual)) = self.instructions.get(value) else {
                    return false;
                };
                let Some(&(definition, position)) = self.positions.get(value) else {
                    return false;
                };
                *actual == ty
                    && if definition == block {
                        position < index
                    } else {
                        dominates(&self.cfg, definition, block)
                    }
            }
            Operand::Constant(Constant::Null) => ty == Type::Ref,
            Operand::Constant(Constant::Int(_)) => ty == Type::I64,
            _ => false,
        }
    }
    fn bound(&self, operand: &Operand, expected: i64, block: usize, index: usize) -> bool {
        if !self.available(operand, Type::Ref, block, index)
            || !literal_bound(operand, expected, &self.instructions)
        {
            return false;
        }
        let Operand::Val(value) = operand else {
            return false;
        };
        let (Instruction::Box { op, .. }, _) = self.instructions[value] else {
            return false;
        };
        let &(definition, position) = &self.positions[value];
        self.available(op, Type::I64, definition, position)
    }
}

/// A source premise is insufficient when foreign code can replace tables or
/// expose primitive storage. Dead FFI adapters are removed by the preceding
/// established recipe/reachability fixpoint, never exempted by their names.
fn closed(module: &Module) -> bool {
    if !module.primitive_clamp_certificate
        || !module.arc_fixed_ancestry
        || module.sdk_library
        || module.arc_memory
        || module.arc_prepared
        || module.arc_aot_ports.is_some()
        || module.table_exceptions
        || !module.ffi_callbacks.is_empty()
        || !module.ffi_trampolines.is_empty()
        || !crate::pruning::receiver_provenance::registry::closed(&[module])
    {
        return false;
    }
    for cid in [
        dartforge_runtime::layout::cid::SMI,
        dartforge_runtime::layout::cid::MINT,
        dartforge_runtime::layout::cid::DOUBLE,
    ] {
        if module.pruning_ambiguous_class_ids.contains(&(cid as u32))
            || module.classes.iter().any(|class| class.id == cid as u32)
        {
            // These are imported runtime classes; a local physical-ID definition
            // cannot inherit the selected source certificate.
            return false;
        }
    }
    let mut symbols = HashSet::new();
    for function in &module.functions {
        if !symbols.insert(&function.symbol)
            || matches!(
                function.symbol.as_str(),
                "dartforge_value_class"
                    | "df.unbox_int"
                    | "df.unbox_double"
                    | "dartforge_globals_area"
                    | "dartforge_globals_area_id"
            )
        {
            return false;
        }
        for (_, instruction, _) in function.blocks.iter().flat_map(|b| &b.instructions) {
            match instruction {
                Instruction::NativeCall { .. }
                | Instruction::CompositeNativeCall { .. }
                | Instruction::NativeWrite { .. }
                | Instruction::ArcStoreStrong { .. } => return false,
                Instruction::CallRuntime { name, .. }
                | Instruction::CallStatic { symbol: name, .. }
                    if name.starts_with("dartforge_ffi_")
                        || name.starts_with("dartforge_native_Ffi_")
                        || matches!(
                            name.as_str(),
                            "dartforge_globals_area" | "dartforge_globals_area_id"
                        ) =>
                {
                    return false;
                }
                _ => {}
            }
        }
    }
    true
}

/// Optimize only actual integer literal bounds, preserving their original boxes.
fn literal_bound(
    operand: &Operand,
    expected: i64,
    definitions: &HashMap<ValueId, (&Instruction, Type)>,
) -> bool {
    let Operand::Val(value) = operand else {
        return false;
    };
    let Some((
        Instruction::Box {
            op,
            from: Type::I64,
        },
        Type::Ref,
    )) = definitions.get(value)
    else {
        return false;
    };
    match op {
        Operand::Constant(Constant::Int(n)) => *n == expected,
        Operand::Val(value) => {
            matches!(definitions.get(value),Some((Instruction::Const(Constant::Int(n)),Type::I64)) if *n==expected)
        }
        _ => false,
    }
}

fn call_parts<'a>(
    instruction: &'a Instruction,
    targets: &HashSet<String>,
) -> Option<(&'a Operand, &'a Operand, &'a Operand)> {
    match instruction {
        Instruction::CallSelector {
            selector,
            recv,
            args,
            names,
            tuple_types,
        } if matches!(selector.as_str(), "c:clamp" | "tc:clamp")
            && names.is_empty()
            && *tuple_types == Operand::Constant(Constant::Int(0))
            && args.len() == 2 =>
        {
            Some((recv, &args[0], &args[1]))
        }
        Instruction::CallStatic {
            symbol,
            args,
            ret_ty: Type::Ref,
        } if targets.contains(symbol) && args.len() == 3 => Some((&args[0], &args[1], &args[2])),
        _ => None,
    }
}

pub(crate) fn rewrite(module: &mut Module) -> usize {
    if std::env::var("DARTFORGE_NO_PRIMITIVE_CLAMP").as_deref() == Ok("1") || !closed(module) {
        return 0;
    }
    let mut rewritten = 0;
    for function in &mut module.functions {
        if function.debug_info.is_some() || function.blocks.len() > 4096 {
            continue;
        }
        loop {
            if rewritten >= MAX_SITES {
                return rewritten;
            }
            let Some(definitions) = Definitions::new(function) else {
                break;
            };
            let site = function.blocks.iter().enumerate().find_map(|(block, b)| {
                b.instructions
                    .iter()
                    .enumerate()
                    .find_map(|(index, (_, instruction, ty))| {
                        let (receiver, lower, upper) =
                            call_parts(instruction, &module.primitive_clamp_targets)?;
                        (*ty == Type::Ref
                            && definitions.available(receiver, Type::Ref, block, index)
                            && definitions.bound(lower, 0, block, index)
                            && definitions.bound(upper, 1, block, index))
                        .then_some((block, index))
                    })
            });
            let Some((block, index)) = site else {
                break;
            };
            // A fallback retains the original selector instruction. Do not visit
            // generated fallback blocks again within this invocation.
            let Some(new_blocks) = split(function, block, index, &module.primitive_clamp_targets)
            else {
                break;
            };
            rewritten += 1;
            // Continue only through the original remainder and original blocks.
            // The fallback's argument copies prevent literal matching on a rerun.
            function.blocks.extend(new_blocks);
        }
    }
    rewritten
}

struct Builder {
    value: u32,
    block: u32,
}
impl Builder {
    fn emit(
        &mut self,
        instructions: &mut Vec<(ValueId, Instruction, Type)>,
        instruction: Instruction,
        ty: Type,
    ) -> Operand {
        let value = ValueId(self.value);
        self.value += 1;
        instructions.push((value, instruction, ty));
        Operand::Val(value)
    }
    fn block(&mut self) -> BlockId {
        let block = BlockId(self.block);
        self.block += 1;
        block
    }
}
fn int(n: i64) -> Operand {
    Operand::Constant(Constant::Int(n))
}

fn split(
    function: &mut Function,
    position: usize,
    index: usize,
    targets: &HashSet<String>,
) -> Option<Vec<BasicBlock>> {
    let max_value = function
        .params
        .iter()
        .map(|(v, _, _)| v.0)
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|b| b.instructions.iter().map(|(v, _, _)| v.0)),
        )
        .max()
        .unwrap_or(0);
    let max_block = function.blocks.iter().map(|b| b.id.0).max()?;
    // Bound all IDs before mutating the function.
    if max_value > u32::MAX - 64 || max_block > u32::MAX - 16 {
        return None;
    }
    let mut builder = Builder {
        value: max_value + 1,
        block: max_block + 1,
    };
    let (recv, lower, upper) =
        call_parts(&function.blocks[position].instructions[index].1, targets)?;
    let (receiver, lower, upper) = (recv.clone(), lower.clone(), upper.clone());
    let slow = builder.block();
    let numeric = builder.block();
    let integer = builder.block();
    let floating = builder.block();
    let integer_upper = builder.block();
    let float_upper = builder.block();
    let return_lower = builder.block();
    let return_upper = builder.block();
    let return_receiver = builder.block();
    let join = builder.block();
    let original = function.blocks[position].id;
    let tail = function.blocks[position].instructions.split_off(index);
    let (result, mut call, ty) = tail[0].clone();
    let terminator = std::mem::replace(
        &mut function.blocks[position].terminator,
        Terminator::Unreachable,
    );
    let instructions = &mut function.blocks[position].instructions;
    let cid = builder.emit(
        instructions,
        Instruction::CallRuntime {
            name: "dartforge_value_class".into(),
            args: vec![(receiver.clone(), Type::Ref)],
            ret_ty: Type::I64,
        },
        Type::I64,
    );
    let smi = builder.emit(
        instructions,
        Instruction::ICmp(
            ICmpOp::Eq,
            cid.clone(),
            int(i64::from(dartforge_runtime::layout::cid::SMI)),
        ),
        Type::I1,
    );
    let mint = builder.emit(
        instructions,
        Instruction::ICmp(
            ICmpOp::Eq,
            cid.clone(),
            int(i64::from(dartforge_runtime::layout::cid::MINT)),
        ),
        Type::I1,
    );
    let double = builder.emit(
        instructions,
        Instruction::ICmp(
            ICmpOp::Eq,
            cid,
            int(i64::from(dartforge_runtime::layout::cid::DOUBLE)),
        ),
        Type::I1,
    );
    let integer_condition = builder.emit(instructions, Instruction::Or(smi, mint), Type::I1);
    let primitive = builder.emit(
        instructions,
        Instruction::Or(integer_condition, double.clone()),
        Type::I1,
    );
    function.blocks[position].terminator = Terminator::CondBranch {
        cond: primitive,
        then_block: numeric,
        else_block: slow,
    };
    let numeric = BasicBlock {
        id: numeric,
        instructions: Vec::new(),
        terminator: Terminator::CondBranch {
            cond: double,
            then_block: floating,
            else_block: integer,
        },
    };
    let mut integer_instructions = Vec::new();
    let number = builder.emit(
        &mut integer_instructions,
        Instruction::Unbox {
            op: receiver.clone(),
            to: Type::I64,
        },
        Type::I64,
    );
    let below = builder.emit(
        &mut integer_instructions,
        Instruction::ICmp(ICmpOp::Slt, number.clone(), int(0)),
        Type::I1,
    );
    let integer = BasicBlock {
        id: integer,
        instructions: integer_instructions,
        terminator: Terminator::CondBranch {
            cond: below,
            then_block: return_lower,
            else_block: integer_upper,
        },
    };
    let mut integer_upper_instructions = Vec::new();
    let above = builder.emit(
        &mut integer_upper_instructions,
        Instruction::ICmp(ICmpOp::Sgt, number, int(1)),
        Type::I1,
    );
    let integer_upper = BasicBlock {
        id: integer_upper,
        instructions: integer_upper_instructions,
        terminator: Terminator::CondBranch {
            cond: above,
            then_block: return_upper,
            else_block: return_receiver,
        },
    };
    let mut float_instructions = Vec::new();
    let number = builder.emit(
        &mut float_instructions,
        Instruction::Unbox {
            op: receiver.clone(),
            to: Type::F64,
        },
        Type::F64,
    );
    let bits = builder.emit(
        &mut float_instructions,
        Instruction::Bitcast {
            op: number.clone(),
            to: Type::I64,
        },
        Type::I64,
    );
    let negative_zero = builder.emit(
        &mut float_instructions,
        Instruction::ICmp(ICmpOp::Eq, bits.clone(), int(i64::MIN)),
        Type::I1,
    );
    let below = builder.emit(
        &mut float_instructions,
        Instruction::FCmp(
            FCmpOp::Lt,
            number.clone(),
            Operand::Constant(Constant::Double(0.0)),
        ),
        Type::I1,
    );
    let below = builder.emit(
        &mut float_instructions,
        Instruction::Or(below, negative_zero),
        Type::I1,
    );
    let floating = BasicBlock {
        id: floating,
        instructions: float_instructions,
        terminator: Terminator::CondBranch {
            cond: below,
            then_block: return_lower,
            else_block: float_upper,
        },
    };
    let mut upper_instructions = Vec::new();
    let absolute_bits = builder.emit(
        &mut upper_instructions,
        Instruction::And(bits, int(i64::MAX)),
        Type::I64,
    );
    let nan = builder.emit(
        &mut upper_instructions,
        Instruction::ICmp(ICmpOp::Sgt, absolute_bits, int(0x7ff0000000000000)),
        Type::I1,
    );
    let above = builder.emit(
        &mut upper_instructions,
        Instruction::FCmp(FCmpOp::Gt, number, Operand::Constant(Constant::Double(1.0))),
        Type::I1,
    );
    let above = builder.emit(
        &mut upper_instructions,
        Instruction::Or(above, nan),
        Type::I1,
    );
    let float_upper = BasicBlock {
        id: float_upper,
        instructions: upper_instructions,
        terminator: Terminator::CondBranch {
            cond: above,
            then_block: return_upper,
            else_block: return_receiver,
        },
    };
    // Copies keep the original Ref operands and prevent this fallback from
    // being mistaken for another optimizable literal-bound call.
    let mut slow_instructions = Vec::new();
    let lower_copy = builder.emit(
        &mut slow_instructions,
        Instruction::Phi {
            incoming: vec![(original, lower.clone())],
            ty: Type::Ref,
        },
        Type::Ref,
    );
    let upper_copy = builder.emit(
        &mut slow_instructions,
        Instruction::Phi {
            incoming: vec![(original, upper.clone())],
            ty: Type::Ref,
        },
        Type::Ref,
    );
    if let Instruction::CallSelector { args, .. } = &mut call {
        *args = vec![lower_copy, upper_copy];
    } else if let Instruction::CallStatic { args, .. } = &mut call {
        *args = vec![receiver.clone(), lower_copy, upper_copy];
    }
    let fallback = builder.emit(&mut slow_instructions, call, ty);
    let slow = BasicBlock {
        id: slow,
        instructions: slow_instructions,
        terminator: Terminator::Branch(join),
    };
    let bound = |id| BasicBlock {
        id,
        instructions: Vec::new(),
        terminator: Terminator::Branch(join),
    };
    let mut remainder = vec![(
        result,
        Instruction::Phi {
            incoming: vec![
                (slow.id, fallback),
                (return_lower, lower),
                (return_upper, upper),
                (return_receiver, receiver),
            ],
            ty: Type::Ref,
        },
        Type::Ref,
    )];
    remainder.extend(tail.into_iter().skip(1));
    // Moving the original terminator changes the predecessor of each original
    // successor, including a loop header that happens to be the current block.
    for block in &mut function.blocks {
        for (_, instruction, _) in &mut block.instructions {
            if let Instruction::Phi { incoming, .. } = instruction {
                for (predecessor, _) in incoming {
                    if *predecessor == original {
                        *predecessor = join;
                    }
                }
            }
        }
    }
    for (_, instruction, _) in &mut remainder {
        if let Instruction::Phi { incoming, .. } = instruction {
            for (predecessor, _) in incoming {
                if *predecessor == original {
                    *predecessor = join;
                }
            }
        }
    }
    Some(vec![
        numeric,
        integer,
        floating,
        integer_upper,
        float_upper,
        bound(return_lower),
        bound(return_upper),
        bound(return_receiver),
        slow,
        BasicBlock {
            id: join,
            instructions: remainder,
            terminator,
        },
    ])
}

#[cfg(test)]
mod tests;
