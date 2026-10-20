//! Typed CFG/SSA validation for the instructions admitted by the NoEscape subset.
//! This does not validate heap layout, stack initialization, or runtime effects.

use crate::hir::{
    BlockId, Constant, Function, ICmpOp, Instruction as I, Module, Operand, Terminator, Type,
    ValueId,
};
use std::collections::{HashMap, HashSet};

/// Version of the admitted typing and CFG/SSA validation rules.
pub const VALIDATOR_VERSION: &str = "noescape-typed-ssa-v1";

/// Malformed IR differs from a well-formed operation outside this subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    Malformed,
    Unsupported,
}
/// A structural/type failure with its exact HIR location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationFailure {
    pub kind: FailureKind,
    pub block: Option<BlockId>,
    pub value: Option<ValueId>,
    pub reason: String,
}
/// Counts from a completed subset validation. Not a layout/effect certificate.
#[derive(Clone, Debug)]
pub struct ValidatedSubset {
    pub validator_version: &'static str,
    pub blocks: usize,
    pub definitions: usize,
}
#[derive(Clone, Copy)]
struct Definition {
    ty: Type,
    block: Option<BlockId>,
    position: usize,
}
struct Validator<'a> {
    module: &'a Module,
    function: &'a Function,
    definitions: HashMap<ValueId, Definition>,
    predecessors: HashMap<BlockId, Vec<BlockId>>,
    dominators: HashMap<BlockId, HashSet<BlockId>>,
}

/// Validates CFG edges, SSA availability/dominance, Phi edges, and exact types.
///
/// Only this closed primitive/reference-identity/static-call subset is supported.
/// Unreachable blocks and unmodeled operations fail closed, distinctly from
/// malformed SSA. Phi predecessors include edge multiplicity, as LLVM requires.
/// This does not establish numeric heap layout, ABI provenance, or effects.
///
/// ```
/// use dartforge_emit_native::{contracts::ssa::validate, hir::{Function, Module, Type}};
/// let function = Function { symbol: "empty".into(), name: "empty".into(),
///     params: vec![], return_ty: Type::Void, blocks: vec![], debug_info: None };
/// assert!(validate(&Module::default(), &function).is_err());
/// ```
///
/// # Errors
/// Returns a located `Malformed` failure for invalid CFG/SSA/types and an
/// `Unsupported` failure for forms whose typing/dominance policy is unavailable.
pub fn validate(
    module: &Module,
    function: &Function,
) -> Result<ValidatedSubset, ValidationFailure> {
    let mut validator = Validator {
        module,
        function,
        definitions: HashMap::new(),
        predecessors: HashMap::new(),
        dominators: HashMap::new(),
    };
    validator.cfg()?;
    validator.check()?;
    Ok(ValidatedSubset {
        validator_version: VALIDATOR_VERSION,
        blocks: function.blocks.len(),
        definitions: validator.definitions.len(),
    })
}
fn failure(
    kind: FailureKind,
    block: Option<BlockId>,
    value: Option<ValueId>,
    reason: impl Into<String>,
) -> ValidationFailure {
    ValidationFailure {
        kind,
        block,
        value,
        reason: reason.into(),
    }
}
fn malformed(
    block: Option<BlockId>,
    value: Option<ValueId>,
    reason: impl Into<String>,
) -> ValidationFailure {
    failure(FailureKind::Malformed, block, value, reason)
}
impl Validator<'_> {
    fn cfg(&mut self) -> Result<(), ValidationFailure> {
        for (id, _, ty) in &self.function.params {
            if *ty == Type::Void
                || self
                    .definitions
                    .insert(
                        *id,
                        Definition {
                            ty: *ty,
                            block: None,
                            position: 0,
                        },
                    )
                    .is_some()
            {
                return Err(malformed(
                    None,
                    Some(*id),
                    "invalid or duplicate parameter definition",
                ));
            }
        }
        let Some(entry) = self.function.blocks.first() else {
            return Err(malformed(None, None, "function has no entry block"));
        };
        for block in &self.function.blocks {
            if self.predecessors.insert(block.id, Vec::new()).is_some() {
                return Err(malformed(Some(block.id), None, "duplicate block identity"));
            }
            for (position, (id, _, ty)) in block.instructions.iter().enumerate() {
                if self
                    .definitions
                    .insert(
                        *id,
                        Definition {
                            ty: *ty,
                            block: Some(block.id),
                            position,
                        },
                    )
                    .is_some()
                {
                    return Err(malformed(
                        Some(block.id),
                        Some(*id),
                        "duplicate SSA definition",
                    ));
                }
            }
        }
        let blocks: HashMap<_, _> = self.function.blocks.iter().map(|b| (b.id, b)).collect();
        for block in &self.function.blocks {
            if let Terminator::Switch { cases, .. } = &block.terminator {
                let mut keys = HashSet::new();
                if cases.iter().any(|(key, _)| !keys.insert(*key)) {
                    return Err(malformed(Some(block.id), None, "duplicate switch case"));
                }
            }
            for target in successors(&block.terminator) {
                let Some(predecessors) = self.predecessors.get_mut(&target) else {
                    return Err(malformed(
                        Some(block.id),
                        None,
                        "branch target is unavailable",
                    ));
                };
                predecessors.push(block.id);
            }
        }
        if !self.predecessors[&entry.id].is_empty() {
            return Err(malformed(
                Some(entry.id),
                None,
                "entry block cannot have predecessors",
            ));
        }
        let mut reachable = HashSet::new();
        let mut pending = vec![entry.id];
        while let Some(id) = pending.pop() {
            if reachable.insert(id) {
                pending.extend(successors(&blocks[&id].terminator));
            }
        }
        if reachable.len() != blocks.len() {
            return Err(failure(
                FailureKind::Unsupported,
                self.function
                    .blocks
                    .iter()
                    .find(|b| !reachable.contains(&b.id))
                    .map(|b| b.id),
                None,
                "unreachable-component dominance policy is not implemented",
            ));
        }
        for block in &self.function.blocks {
            self.dominators.insert(
                block.id,
                if block.id == entry.id {
                    HashSet::from([entry.id])
                } else {
                    reachable.clone()
                },
            );
        }
        loop {
            let mut changed = false;
            for block in self.function.blocks.iter().filter(|b| b.id != entry.id) {
                let predecessors = &self.predecessors[&block.id];
                let mut intersection = self.dominators[&predecessors[0]].clone();
                for predecessor in &predecessors[1..] {
                    intersection.retain(|id| self.dominators[predecessor].contains(id));
                }
                intersection.insert(block.id);
                if intersection != self.dominators[&block.id] {
                    self.dominators.insert(block.id, intersection);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        Ok(())
    }

    fn operand_type(
        &self,
        operand: &Operand,
        block: BlockId,
        value: Option<ValueId>,
    ) -> Result<Type, ValidationFailure> {
        match operand {
            Operand::Val(id) => self
                .definitions
                .get(id)
                .map(|d| d.ty)
                .filter(|ty| *ty != Type::Void)
                .ok_or_else(|| {
                    malformed(
                        Some(block),
                        value,
                        format!("undefined or void SSA operand {}", id.0),
                    )
                }),
            Operand::Constant(constant) => Ok(constant_type(constant)),
        }
    }
    fn use_at(
        &self,
        operand: &Operand,
        block: BlockId,
        position: usize,
        value: Option<ValueId>,
    ) -> Result<(), ValidationFailure> {
        self.operand_type(operand, block, value)?;
        if let Operand::Val(id) = operand {
            let definition = self.definitions[id];
            if let Some(origin) = definition.block {
                if origin == block {
                    if definition.position >= position {
                        return Err(malformed(
                            Some(block),
                            value,
                            "SSA use occurs before its definition",
                        ));
                    }
                } else if !self.dominators[&block].contains(&origin) {
                    return Err(malformed(
                        Some(block),
                        value,
                        "SSA definition does not dominate its use",
                    ));
                }
            }
        }
        Ok(())
    }
    fn expect(
        &self,
        operand: &Operand,
        ty: Type,
        block: BlockId,
        value: Option<ValueId>,
    ) -> Result<(), ValidationFailure> {
        if self.operand_type(operand, block, value)? != ty {
            return Err(malformed(
                Some(block),
                value,
                format!("operand must have type {ty:?}"),
            ));
        }
        Ok(())
    }
    fn check(&self) -> Result<(), ValidationFailure> {
        for block in &self.function.blocks {
            let mut ordinary_seen = false;
            for (position, (id, instruction, ty)) in block.instructions.iter().enumerate() {
                if let I::Phi {
                    incoming,
                    ty: declared,
                } = instruction
                {
                    if ordinary_seen || incoming.is_empty() || *declared != *ty || *ty == Type::Void
                    {
                        return Err(malformed(
                            Some(block.id),
                            Some(*id),
                            "Phi must be a nonempty typed block prefix",
                        ));
                    }
                    let mut expected = self.predecessors[&block.id].clone();
                    expected.sort_by_key(|b| b.0);
                    let mut actual: Vec<_> = incoming.iter().map(|(b, _)| *b).collect();
                    actual.sort_by_key(|b| b.0);
                    if expected != actual {
                        return Err(malformed(
                            Some(block.id),
                            Some(*id),
                            "Phi incoming labels must equal predecessor edges including multiplicity",
                        ));
                    }
                    let mut repeated = HashMap::new();
                    for (predecessor, operand) in incoming {
                        self.expect(operand, *ty, block.id, Some(*id))?;
                        let end = self
                            .function
                            .blocks
                            .iter()
                            .find(|b| b.id == *predecessor)
                            .unwrap()
                            .instructions
                            .len();
                        self.use_at(operand, *predecessor, end, Some(*id))?;
                        if let Some(previous) = repeated.insert(*predecessor, operand)
                            && !exact_operand(previous, operand)
                        {
                            return Err(malformed(
                                Some(block.id),
                                Some(*id),
                                "duplicate Phi predecessor edges need the same operand",
                            ));
                        }
                    }
                } else {
                    ordinary_seen = true;
                    let operands = instruction_operands(instruction).ok_or_else(|| {
                        failure(
                            FailureKind::Unsupported,
                            Some(block.id),
                            Some(*id),
                            "instruction is outside the typed SSA validation subset",
                        )
                    })?;
                    for operand in operands {
                        self.use_at(operand, block.id, position, Some(*id))?;
                    }
                    self.instruction(instruction, *ty, block.id, *id)?;
                }
            }
            let end = block.instructions.len();
            match &block.terminator {
                Terminator::Return(Some(value)) => {
                    self.use_at(value, block.id, end, None)?;
                    self.expect(value, self.function.return_ty, block.id, None)?;
                }
                Terminator::Return(None) if self.function.return_ty == Type::Void => {}
                Terminator::Return(None) => {
                    return Err(malformed(
                        Some(block.id),
                        None,
                        "missing non-void return value",
                    ));
                }
                Terminator::CondBranch { cond, .. } => {
                    self.use_at(cond, block.id, end, None)?;
                    self.expect(cond, Type::I1, block.id, None)?;
                }
                Terminator::Switch { val, .. } => {
                    self.use_at(val, block.id, end, None)?;
                    self.expect(val, Type::I64, block.id, None)?;
                }
                Terminator::Throw(value) => {
                    self.use_at(value, block.id, end, None)?;
                    self.expect(value, Type::Ref, block.id, None)?;
                }
                Terminator::Branch(_) | Terminator::Unreachable => {}
            }
        }
        Ok(())
    }
    fn instruction(
        &self,
        instruction: &I,
        result: Type,
        block: BlockId,
        value: ValueId,
    ) -> Result<(), ValidationFailure> {
        let error = || {
            malformed(
                Some(block),
                Some(value),
                "instruction operand/result types are incompatible",
            )
        };
        let unary = |operand: &Operand, input, output| -> Result<(), ValidationFailure> {
            self.expect(operand, input, block, Some(value))?;
            if result != output {
                return Err(error());
            }
            Ok(())
        };
        let binary = |a: &Operand, b: &Operand, input, output| -> Result<(), ValidationFailure> {
            self.expect(a, input, block, Some(value))?;
            self.expect(b, input, block, Some(value))?;
            if result != output {
                return Err(error());
            }
            Ok(())
        };
        match instruction {
            I::Const(constant) => {
                if result != constant_type(constant)
                    && !(matches!(constant, Constant::Int(_))
                        && matches!(result, Type::I8 | Type::I1))
                {
                    return Err(error());
                }
            }
            I::Add(a, b)
            | I::Sub(a, b)
            | I::Mul(a, b)
            | I::SDiv(a, b)
            | I::SRem(a, b)
            | I::Shl(a, b)
            | I::AShr(a, b)
            | I::LShr(a, b)
            | I::And(a, b)
            | I::Or(a, b)
            | I::Xor(a, b) => binary(a, b, Type::I64, Type::I64)?,
            I::Neg(a) | I::Not(a) => unary(a, Type::I64, Type::I64)?,
            I::FAdd(a, b) | I::FSub(a, b) | I::FMul(a, b) | I::FDiv(a, b) => {
                binary(a, b, Type::F64, Type::F64)?
            }
            I::FNeg(a) => unary(a, Type::F64, Type::F64)?,
            I::LNot(a) => unary(a, Type::I1, Type::I1)?,
            I::ICmp(op, a, b) => {
                let input = self.operand_type(a, block, Some(value))?;
                if !matches!(input, Type::I64 | Type::I8 | Type::I1)
                    && !(input == Type::Ref && matches!(op, ICmpOp::Eq | ICmpOp::Ne))
                {
                    return Err(error());
                }
                binary(a, b, input, Type::I1)?;
            }
            I::FCmp(_, a, b) => binary(a, b, Type::F64, Type::I1)?,
            I::IntToDouble(a) => unary(a, Type::I64, Type::F64)?,
            I::DoubleToInt(a) => unary(a, Type::F64, Type::I64)?,
            I::ZExt { op, from, to } | I::Trunc { op, from, to } => {
                let Some(from_width) = integer_width(*from) else {
                    return Err(error());
                };
                let Some(to_width) = integer_width(*to) else {
                    return Err(error());
                };
                if matches!(instruction, I::ZExt { .. }) && from_width >= to_width
                    || matches!(instruction, I::Trunc { .. }) && from_width <= to_width
                {
                    return Err(error());
                }
                unary(op, *from, *to)?;
            }
            I::Bitcast { op, to } => {
                let from = self.operand_type(op, block, Some(value))?;
                if bit_width(from).is_none() || bit_width(from) != bit_width(*to) || result != *to {
                    return Err(error());
                }
            }
            I::CallStatic {
                symbol,
                args,
                ret_ty,
            } => {
                let mut matching = self.module.functions.iter().filter(|f| f.symbol == *symbol);
                let Some(callee) = matching.next() else {
                    return Err(failure(
                        FailureKind::Unsupported,
                        Some(block),
                        Some(value),
                        "static callee signature is unavailable",
                    ));
                };
                if matching.next().is_some() {
                    return Err(error());
                }
                if args.len() != callee.params.len()
                    || *ret_ty != callee.return_ty
                    || result != *ret_ty
                {
                    return Err(error());
                }
                for (argument, parameter) in args.iter().zip(&callee.params) {
                    self.expect(argument, parameter.2, block, Some(value))?;
                }
            }
            _ => {
                return Err(failure(
                    FailureKind::Unsupported,
                    Some(block),
                    Some(value),
                    "instruction typing is unavailable",
                ));
            }
        }
        Ok(())
    }
}
fn constant_type(value: &Constant) -> Type {
    match value {
        Constant::Int(_) => Type::I64,
        Constant::Double(_) => Type::F64,
        Constant::Bool(_) => Type::I1,
        _ => Type::Ref,
    }
}
fn integer_width(ty: Type) -> Option<u32> {
    match ty {
        Type::I1 => Some(1),
        Type::I8 => Some(8),
        Type::I64 => Some(64),
        _ => None,
    }
}
fn bit_width(ty: Type) -> Option<u32> {
    integer_width(ty).or(match ty {
        Type::F64 => Some(64),
        Type::V4F32 | Type::V4I32 | Type::V2F64 => Some(128),
        _ => None,
    })
}
fn successors(exit: &Terminator) -> Vec<BlockId> {
    match exit {
        Terminator::Branch(b) => vec![*b],
        Terminator::CondBranch {
            then_block,
            else_block,
            ..
        } => vec![*then_block, *else_block],
        Terminator::Switch { default, cases, .. } => std::iter::once(*default)
            .chain(cases.iter().map(|(_, b)| *b))
            .collect(),
        _ => vec![],
    }
}
fn exact_operand(a: &Operand, b: &Operand) -> bool {
    match (a, b) {
        (Operand::Constant(Constant::Double(a)), Operand::Constant(Constant::Double(b))) => {
            a.to_bits() == b.to_bits()
        }
        _ => a == b,
    }
}
fn instruction_operands(instruction: &I) -> Option<Vec<&Operand>> {
    Some(match instruction {
        I::Const(_) => vec![],
        I::Add(a, b)
        | I::Sub(a, b)
        | I::Mul(a, b)
        | I::SDiv(a, b)
        | I::SRem(a, b)
        | I::Shl(a, b)
        | I::AShr(a, b)
        | I::LShr(a, b)
        | I::And(a, b)
        | I::Or(a, b)
        | I::Xor(a, b)
        | I::FAdd(a, b)
        | I::FSub(a, b)
        | I::FMul(a, b)
        | I::FDiv(a, b)
        | I::ICmp(_, a, b)
        | I::FCmp(_, a, b) => vec![a, b],
        I::Neg(a)
        | I::Not(a)
        | I::FNeg(a)
        | I::LNot(a)
        | I::IntToDouble(a)
        | I::DoubleToInt(a)
        | I::ZExt { op: a, .. }
        | I::Trunc { op: a, .. }
        | I::Bitcast { op: a, .. } => vec![a],
        I::CallStatic { args, .. } => args.iter().collect(),
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
