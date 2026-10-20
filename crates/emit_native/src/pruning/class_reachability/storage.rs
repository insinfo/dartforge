//! Bounded storage witnesses for the experimental closed-class proof.
//!
//! A field declaration is not evidence about the bits stored in an object.
//! Track local allocation identities and every write, so scalar words cannot
//! become managed references through an erased field ABI. Unknown receivers,
//! pointer-valued heap reads and list storage need separate proofs.

use super::*;

const MANAGED: u8 = 1;
const SCALAR: u8 = 2;
const MAX_SLOTS: usize = 8192;

pub(super) struct Proof {
    aliases: HashMap<ValueId, ValueId>,
    slots: HashMap<ValueId, Vec<u8>>,
}

/// Actual emitter representations, not just annotations consumed by Shape.
fn representations(function: &Function) -> Option<HashMap<ValueId, Type>> {
    let mut types: HashMap<_, _> = function
        .params
        .iter()
        .map(|(id, _, ty)| (*id, *ty))
        .collect();
    for (id, instruction, annotated) in function.blocks.iter().flat_map(|b| &b.instructions) {
        let actual = crate::llvm::LlvmEmitter::result_type(instruction, *annotated);
        if (*annotated == Type::Ref) != (actual == Type::Ref)
            || (*annotated == Type::Ptr) != (actual == Type::Ptr)
            || types.insert(*id, actual).is_some()
        {
            return None;
        }
    }
    Some(types)
}

fn kind(operand: &Operand, types: &HashMap<ValueId, Type>) -> Option<u8> {
    let ty = match operand {
        Operand::Val(id) => *types.get(id)?,
        Operand::Constant(Constant::Null | Constant::String(_) | Constant::StringWtf8(_)) => {
            Type::Ref
        }
        Operand::Constant(Constant::Int(_) | Constant::Function(_)) => Type::I64,
        Operand::Constant(Constant::Bool(_)) => Type::I1,
        Operand::Constant(Constant::Double(_)) => Type::F64,
    };
    match ty {
        Type::Ref => Some(MANAGED),
        Type::I1 | Type::I8 | Type::I64 | Type::F64 | Type::Ptr => Some(SCALAR),
        _ => None,
    }
}

/// Runtime spelling, erased argument representations and declared return type.
type RuntimeCall<'a> = (&'a str, &'a [(Operand, Type)], Type);

fn runtime(instruction: &Instruction) -> Option<RuntimeCall<'_>> {
    match instruction {
        Instruction::CallRuntime { name, args, ret_ty }
        | Instruction::DeclaredFieldCall {
            name, args, ret_ty, ..
        } => Some((name, args, *ret_ty)),
        _ => None,
    }
}

impl Proof {
    pub(super) fn new(function: &Function) -> Option<Self> {
        let types = representations(function)?;
        let instructions: Vec<_> = function
            .blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .collect();
        let constants: HashMap<_, _> = instructions
            .iter()
            .filter_map(|(id, instruction, _)| {
                if let Instruction::Const(Constant::Int(n)) = instruction {
                    Some((*id, *n))
                } else {
                    None
                }
            })
            .collect();
        let integer = |operand: &Operand| match operand {
            Operand::Constant(Constant::Int(n)) => Some(*n),
            Operand::Val(id) => constants.get(id).copied(),
            _ => None,
        };
        let mut proof = Self {
            aliases: HashMap::new(),
            slots: HashMap::new(),
        };
        let mut total = 0usize;
        for (id, instruction, _) in &instructions {
            let slots = match instruction {
                Instruction::AllocObject { fields, .. } => {
                    if fields.len() > MAX_SLOTS {
                        return None;
                    }
                    fields
                        .iter()
                        .map(|field| kind(field, &types))
                        .collect::<Option<Vec<_>>>()?
                }
                Instruction::CallRuntime {
                    name,
                    args,
                    ret_ty: Type::Ref,
                } if name == "dartforge_object_new" => {
                    let [
                        (_, Type::I64),
                        (Operand::Constant(Constant::Int(count)), Type::I64),
                    ] = args.as_slice()
                    else {
                        return None;
                    };
                    let count = usize::try_from(*count).ok()?;
                    if count > MAX_SLOTS {
                        return None;
                    }
                    // Zeroed uninitialized slots can be null or scalar zero.
                    vec![0; count]
                }
                _ => continue,
            };
            total = total.checked_add(slots.len())?;
            if total > MAX_SLOTS {
                return None;
            }
            proof.slots.insert(*id, slots);
            proof.aliases.insert(*id, *id);
        }
        // Preserve identity only; different allocations or unknown incoming
        // values cannot provide a common layout witness. Self-loop Phis are OK.
        loop {
            let mut changed = false;
            for (id, instruction, ty) in &instructions {
                if *ty != Type::Ref || proof.aliases.contains_key(id) {
                    continue;
                }
                let alias = match instruction {
                    Instruction::CheckNotNull(Operand::Val(source)) => {
                        proof.aliases.get(source).copied()
                    }
                    Instruction::Phi { incoming, .. } => {
                        let sources: Option<Vec<_>> = incoming
                            .iter()
                            .filter(|(_, op)| *op != Operand::Val(*id))
                            .map(|(_, op)| proof.site(op))
                            .collect();
                        sources.and_then(|sources| {
                            sources
                                .first()
                                .copied()
                                .filter(|first| sources.iter().all(|source| source == first))
                        })
                    }
                    _ => None,
                };
                if let Some(alias) = alias {
                    proof.aliases.insert(*id, alias);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        // Accumulate all writes before admitting any read, including writes
        // after a read or in another branch. No type-changing store is hidden.
        for (_, instruction, _) in instructions {
            let store = match instruction {
                Instruction::SetField {
                    object,
                    index,
                    value,
                } => Some((object, *index, kind(value, &types)?)),
                Instruction::CellSet { cell, value } => Some((cell, 0, kind(value, &types)?)),
                _ => match runtime(instruction) {
                    Some(("dartforge_object_set", args, Type::Void)) => {
                        let [
                            (object, Type::Ref),
                            (index, Type::I64),
                            (value, _),
                            (flag, Type::I8),
                        ] = args
                        else {
                            return None;
                        };
                        let index = usize::try_from(integer(index)?).ok()?;
                        let actual = kind(value, &types)?;
                        if integer(flag)? != i64::from(actual == MANAGED) {
                            return None;
                        }
                        Some((object, index, actual))
                    }
                    _ => None,
                },
            };
            if let Some((object, index, value_kind)) = store {
                let site = proof.site(object)?;
                *proof.slots.get_mut(&site)?.get_mut(index)? |= value_kind;
            }
        }
        Some(proof)
    }

    fn site(&self, operand: &Operand) -> Option<ValueId> {
        match operand {
            Operand::Val(id) => self.aliases.get(id).copied(),
            _ => None,
        }
    }

    /// Heap loads may preserve a Ref only from slots that never hold scalars.
    pub(super) fn read(&self, object: &Operand, index: usize, result: Type) -> bool {
        let Some(slot) = self
            .site(object)
            .and_then(|site| self.slots.get(&site))
            .and_then(|slots| slots.get(index))
        else {
            return false;
        };
        match result {
            Type::Ref => slot & SCALAR == 0,
            Type::I1 | Type::I8 | Type::I64 | Type::F64 => true,
            _ => false,
        }
    }

    pub(super) fn write(&self, object: &Operand, index: usize, result: Type) -> bool {
        result == Type::Void
            && self
                .site(object)
                .and_then(|site| self.slots.get(&site))
                .is_some_and(|slots| index < slots.len())
    }
}

/// Runtime field indices and tags are known literals or local integer constants.
pub(super) fn index(function: &Function, operand: &Operand) -> Option<usize> {
    let number = match operand {
        Operand::Constant(Constant::Int(number)) => *number,
        Operand::Val(id) => function
            .blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .find_map(|(value, instruction, _)| match instruction {
                Instruction::Const(Constant::Int(n)) if value == id => Some(*n),
                _ => None,
            })?,
        _ => return None,
    };
    usize::try_from(number).ok()
}
