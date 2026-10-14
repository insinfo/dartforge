//! Positive branch facts from canonical subtype queries and one-bit conjunctions.

use super::*;

pub(super) fn positive(
    operand: &Operand,
    consumer: (usize, usize),
    definitions: &Definitions,
    instructions: &HashMap<ValueId, &Instruction>,
    idom: &[usize],
    subtypes: &HashMap<ValueId, (ValueId, u32)>,
) -> Vec<(ValueId, u32)> {
    Walker {
        definitions,
        instructions,
        idom,
        subtypes,
        remaining: 128,
    }
    .boolean(operand, consumer)
    .unwrap_or_default()
}

struct Walker<'a> {
    definitions: &'a Definitions,
    instructions: &'a HashMap<ValueId, &'a Instruction>,
    idom: &'a [usize],
    subtypes: &'a HashMap<ValueId, (ValueId, u32)>,
    remaining: usize,
}

impl Walker<'_> {
    fn ty(&self, operand: &Operand) -> Option<Type> {
        match operand {
            Operand::Val(v) => self.definitions.get(v).map(|(ty, _)| *ty),
            Operand::Constant(Constant::Bool(_)) => Some(Type::I1),
            Operand::Constant(Constant::Int(_)) => Some(Type::I64),
            _ => None,
        }
    }

    fn location(&mut self, v: ValueId, consumer: (usize, usize)) -> Option<(usize, usize)> {
        self.remaining = self.remaining.checked_sub(1)?;
        if !available(self.definitions, self.idom, v, consumer) {
            return None;
        }
        let instruction = self.instructions.get(&v)?;
        let ty = self.definitions[&v].0;
        if crate::llvm::LlvmEmitter::result_type(instruction, ty) != ty {
            return None;
        }
        self.definitions.get(&v)?.1
    }

    fn zero(&self, operand: &Operand, ty: Type) -> bool {
        matches!(
            (operand, ty),
            (Operand::Constant(Constant::Int(0)), Type::I8 | Type::I64)
                | (Operand::Constant(Constant::Bool(false)), Type::I1)
        )
    }

    fn conjunction(
        &mut self,
        a: &Operand,
        b: &Operand,
        location: (usize, usize),
    ) -> Option<Vec<(ValueId, u32)>> {
        if self.ty(a) != Some(Type::I1) || self.ty(b) != Some(Type::I1) {
            return None;
        }
        let mut predicates = self.boolean(a, location)?;
        predicates.extend(self.boolean(b, location)?);
        Some(predicates)
    }

    fn boolean(
        &mut self,
        operand: &Operand,
        consumer: (usize, usize),
    ) -> Option<Vec<(ValueId, u32)>> {
        if self.ty(operand) != Some(Type::I1) {
            return None;
        }
        let Operand::Val(v) = operand else {
            return Some(vec![]);
        };
        if !available(self.definitions, self.idom, *v, consumer) {
            return None;
        }
        if self.definitions[v].1.is_none() {
            return Some(vec![]);
        }
        let location = self.location(*v, consumer)?;
        let instruction = *self.instructions.get(v)?;
        let mut valid = true;
        crate::optimize::operands::operands(instruction, &mut |operand| {
            if let Operand::Val(v) = operand {
                valid &= available(self.definitions, self.idom, *v, location);
            }
        });
        if !valid {
            return None;
        }
        match instruction {
            Instruction::And(a, b) => self.conjunction(a, b, location),
            Instruction::ICmp(ICmpOp::Ne, a, b) => {
                if self.ty(a).is_some_and(|ty| self.zero(b, ty)) {
                    self.nonzero(a, location)
                } else if self.ty(b).is_some_and(|ty| self.zero(a, ty)) {
                    self.nonzero(b, location)
                } else {
                    Some(vec![])
                }
            }
            instruction => {
                if matches!(instruction, Instruction::CallRuntime { ret_ty, .. } | Instruction::CallStatic { ret_ty, .. } if *ret_ty != Type::I1)
                    || matches!(instruction, Instruction::Const(constant) if !matches!(constant, Constant::Bool(_)))
                {
                    return None;
                }
                let mut valid = true;
                crate::optimize::operands::operands(instruction, &mut |operand| {
                    if let Operand::Val(v) = operand {
                        valid &= available(self.definitions, self.idom, *v, location);
                    }
                });
                valid.then_some(vec![])
            }
        }
    }

    fn nonzero(
        &mut self,
        operand: &Operand,
        consumer: (usize, usize),
    ) -> Option<Vec<(ValueId, u32)>> {
        if self.ty(operand) == Some(Type::I1) {
            return self.boolean(operand, consumer);
        }
        let Operand::Val(v) = operand else {
            return Some(vec![]);
        };
        let location = self.location(*v, consumer)?;
        match self.ty(operand) {
            Some(Type::I8) => Some(self.subtypes.get(v).copied().into_iter().collect()),
            // LLVM zero-extends the I1 inputs before an I64 And. Its nonzero
            // result requires both inputs to be true; arbitrary integers do not.
            Some(Type::I64) => match self.instructions.get(v)? {
                Instruction::And(a, b) => self.conjunction(a, b, location),
                _ => Some(vec![]),
            },
            _ => None,
        }
    }
}
