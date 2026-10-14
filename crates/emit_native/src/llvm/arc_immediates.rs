//! Same-block physical null and tagged Smi facts for ARC call emission.

use crate::hir::*;
use dartforge_runtime::layout::smi;
use std::collections::{HashMap, HashSet};

/// Physical non-handles only; logical ARC ownership is deliberately untouched.
#[derive(Default)]
pub(super) struct ImmediateRefs {
    sites: HashMap<ValueId, (BlockId, usize)>,
}

impl ImmediateRefs {
    pub(super) fn new(function: &Function, canonical_owned_box: bool) -> Self {
        if !function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|(_, instruction, _)| {
                matches!(
                    instruction,
                    Instruction::ArcCopy {
                        value: Operand::Val(_)
                    } | Instruction::ArcDrop {
                        value: Operand::Val(_)
                    }
                )
            })
        {
            return Self::default();
        }
        let mut values = HashSet::new();
        let mut blocks = HashSet::new();
        for (value, _, _) in &function.params {
            if !values.insert(*value) {
                return Self::default();
            }
        }
        for block in &function.blocks {
            if !blocks.insert(block.id) {
                return Self::default();
            }
            for (value, _, _) in &block.instructions {
                if !values.insert(*value) {
                    return Self::default();
                }
            }
        }
        let mut proof = Self::default();
        for block in &function.blocks {
            let mut integers = HashMap::new();
            for (position, (value, instruction, ty)) in block.instructions.iter().enumerate() {
                let integer = |operand: &Operand| match operand {
                    Operand::Constant(Constant::Int(n)) => Some(*n),
                    Operand::Val(v) => integers.get(v).copied(),
                    _ => None,
                };
                let immediate = *ty == Type::Ref
                    && match instruction {
                        Instruction::Const(Constant::Null) => true,
                        Instruction::Box {
                            op,
                            from: Type::I64,
                        } => integer(op).is_some_and(|n| smi::of(n).is_some()),
                        Instruction::CallRuntime {
                            name,
                            args,
                            ret_ty: Type::Ref,
                        } if canonical_owned_box && name == "dartforge_arc_box_int_owned_v1" => {
                            match args.as_slice() {
                                [(op, Type::I64)] => {
                                    integer(op).is_some_and(|n| smi::of(n).is_some())
                                }
                                _ => false,
                            }
                        }
                        Instruction::ArcCopy { value } | Instruction::ArcMove { value } => {
                            proof.contains(value, block.id, position)
                        }
                        _ => false,
                    };
                if immediate {
                    proof.sites.insert(*value, (block.id, position));
                }
                // Only earlier exact i64 definitions can supply a boxed constant.
                if let Instruction::Const(Constant::Int(n)) = instruction
                    && *ty == Type::I64
                {
                    integers.insert(*value, *n);
                }
            }
        }
        proof
    }

    pub(super) fn contains(&self, operand: &Operand, block: BlockId, position: usize) -> bool {
        match operand {
            Operand::Constant(Constant::Null) => true,
            Operand::Val(value) => self
                .sites
                .get(value)
                .is_some_and(|&(origin, order)| origin == block && order < position),
            _ => false,
        }
    }
}
