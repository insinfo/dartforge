//! Scalar replacement of objects that do not escape.
//!
//! An object created in the function (`dartforge_object_new`) whose only use is
//! reading and writing its own fields by constant index
//! (`dartforge_object_get`/`dartforge_object_set`, `GetField`/`SetField`)
//! need not exist: nobody observes its identity (not `==`, not
//! `is`, not the collector), and each field becomes a local — which `mem2reg`
//! then promotes to an SSA value. Any use beyond those (argument,
//! return, `phi`, a write into another object, a type, RTI) counts as an escape, and
//! the object stays as it is. After inlining the constructor and the
//! methods, the temporaries of the `Point(a, b).x` kind vanish.

use super::operands::*;
use crate::hir::*;
use std::collections::HashMap;

/// An access to field `idx_value` of a candidate object.
enum Access {
    /// Read: typed logical representation or legacy bits in `I64`.
    Le(Type),
    /// Write: the value and whether it is a reference.
    Writes(Type, bool),
}

fn const_index(o: &Operand) -> Option<usize> {
    match o {
        Operand::Constant(Constant::Int(i)) if *i >= 0 => Some(*i as usize),
        _ => None,
    }
}

fn flag(o: &Operand) -> Option<bool> {
    match o {
        Operand::Constant(Constant::Int(n)) => Some(*n != 0),
        Operand::Constant(Constant::Bool(b)) => Some(*b),
        _ => None,
    }
}

/// Replaces the objects of `func` that do not escape. Returns whether anything changed.
pub fn replace_objects(func: &mut Function) -> bool {
    let types = function_types(func);
    let mut candidates: HashMap<ValueId, Vec<(usize, Access)>> = HashMap::new();
    for b in &func.blocks {
        for (v, inst, _) in &b.instructions {
            // Only program objects (cid ≥ 128): runtime classes have a
            // layout and identity known to the runtime (§2.4).
            if let Instruction::CallRuntime { name, args, .. } | Instruction::DeclaredFieldCall { name, args, .. } = inst
                && name == "dartforge_object_new"
                && matches!(args.first(), Some((Operand::Constant(Constant::Int(c)), _))
                    if *c >= dartforge_runtime::layout::FIRST_FREE_CID)
            {
                candidates.insert(*v, Vec::new());
            }
        }
    }
    if candidates.is_empty() {
        return false;
    }
    let mut escaped: std::collections::HashSet<ValueId> = std::collections::HashSet::new();
    for b in &func.blocks {
        for (_, inst, ty) in &b.instructions {
            // The recognized access: the object and the field, not in another position.
            let recognized = match inst {
                Instruction::CallRuntime { name, args, .. } | Instruction::DeclaredFieldCall { name, args, .. } if name == "dartforge_object_get" && args.len() == 2 => {
                    match (&args[0].0, const_index(&args[1].0)) {
                        (Operand::Val(o), Some(i)) if candidates.contains_key(o) => Some((*o, i, Access::Le(*ty), None)),
                        _ => None,
                    }
                }
                Instruction::CallRuntime { name, args, .. } | Instruction::DeclaredFieldCall { name, args, .. } if name == "dartforge_object_set" && args.len() == 4 => {
                    match (&args[0].0, const_index(&args[1].0), flag(&args[3].0)) {
                        (Operand::Val(o), Some(i), Some(r)) if candidates.contains_key(o) => {
                            let t = operand_type(&args[2].0, &types).unwrap_or(Type::Void);
                            Some((*o, i, Access::Writes(t, r), Some(&args[2].0)))
                        }
                        _ => None,
                    }
                }
                Instruction::GetField { object: Operand::Val(o), index } if candidates.contains_key(o) => {
                    Some((*o, *index, Access::Le(*ty), None))
                }
                Instruction::SetField { object: Operand::Val(o), index, value } if candidates.contains_key(o) => {
                    let t = operand_type(value, &types).unwrap_or(Type::Void);
                    Some((*o, *index, Access::Writes(t, t == Type::Ref), Some(value)))
                }
                _ => None,
            };
            match recognized {
                Some((o, i, access, val_ref)) => {
                    // The object itself written into a field (its own or another's).
                    if let Some(Operand::Val(x)) = val_ref
                        && candidates.contains_key(x)
                    {
                        escaped.insert(*x);
                    }
                    candidates.get_mut(&o).expect("candidato").push((i, access));
                }
                None => operands(inst, &mut |op| {
                    if let Operand::Val(x) = op
                        && candidates.contains_key(x)
                        && !matches!(inst, Instruction::CallRuntime { name, .. } | Instruction::DeclaredFieldCall { name, .. } if name == "dartforge_object_new")
                    {
                        escaped.insert(*x);
                    }
                }),
            }
        }
        terminator_operands(&b.terminator, &mut |op| {
            if let Operand::Val(x) = op
                && candidates.contains_key(x)
            {
                escaped.insert(*x);
            }
        });
    }

    // All accesses must agree on the representation of the field;
    // typed reads cannot load a legacy write in bits.
    let mut locals: HashMap<(ValueId, usize), Type> = HashMap::new();
    let mut order: Vec<ValueId> = candidates.keys().copied().filter(|v| !escaped.contains(v)).collect();
    order.sort_by_key(|v| v.0);
    let mut accepted = Vec::new();
    'objetos: for v in order {
        let mut fields: HashMap<usize, Type> = HashMap::new();
        for (i, a) in &candidates[&v] {
            let t = match a {
                Access::Le(t) => *t,
                Access::Writes(t, true) if *t == Type::Ref => Type::Ref,
                Access::Writes(t, false) if matches!(t, Type::I64 | Type::F64 | Type::I1 | Type::I8) => *t,
                _ => continue 'objetos,
            };
            if !matches!(t, Type::Ref | Type::I64 | Type::F64 | Type::I1 | Type::I8) || *fields.entry(*i).or_insert(t) != t {
                continue 'objetos;
            }
        }
        for (i, t) in fields {
            locals.insert((v, i), t);
        }
        accepted.push(v);
    }
    if accepted.is_empty() {
        return false;
    }

    // One local per field, at the entry.
    let (mut next, _) = largest_ids(func);
    let mut keys: Vec<(ValueId, usize)> = locals.keys().copied().collect();
    keys.sort_by_key(|(v, i)| (v.0, *i));
    let mut addresses: HashMap<(ValueId, usize), ValueId> = HashMap::new();
    let mut allocas = Vec::new();
    let mut initializations: HashMap<ValueId, Vec<(ValueId, Instruction, Type)>> = HashMap::new();
    for k in keys {
        next += 1;
        addresses.insert(k, ValueId(next));
        allocas.push((ValueId(next), Instruction::Alloca(locals[&k]), Type::Ptr));
        if matches!(locals[&k], Type::F64 | Type::I1 | Type::I8) {
            // Reinitializes at the allocation point, not at the entry: an allocation
            // inside a loop starts zeroed in each iteration.
            let val = match locals[&k] {
                Type::F64 => Operand::Constant(Constant::Double(0.0)),
                Type::I1 => Operand::Constant(Constant::Bool(false)),
                _ => Operand::Constant(Constant::Int(0)),
            };
            next += 1;
            initializations.entry(k.0).or_default().push((ValueId(next), Instruction::Store {
                ptr: Operand::Val(addresses[&k]), val,
            }, Type::Void));
        }
    }
    let accepted: std::collections::HashSet<ValueId> = accepted.into_iter().collect();
    for b in &mut func.blocks {
        b.instructions = std::mem::take(&mut b.instructions).into_iter().flat_map(|inst| {
            if accepted.contains(&inst.0) {
                initializations.remove(&inst.0).unwrap_or_default()
            } else { vec![inst] }
        }).collect();
        for (_, inst, ty) in &mut b.instructions {
            let new = match inst {
                Instruction::CallRuntime { name, args, .. } | Instruction::DeclaredFieldCall { name, args, .. } if name == "dartforge_object_get" && args.len() == 2 => {
                    match (&args[0].0, const_index(&args[1].0)) {
                        (Operand::Val(o), Some(i)) if accepted.contains(o) => {
                            Some(Instruction::Load { ptr: Operand::Val(addresses[&(*o, i)]), ty: *ty })
                        }
                        _ => None,
                    }
                }
                Instruction::CallRuntime { name, args, .. } | Instruction::DeclaredFieldCall { name, args, .. } if name == "dartforge_object_set" && args.len() == 4 => {
                    match (&args[0].0, const_index(&args[1].0)) {
                        (Operand::Val(o), Some(i)) if accepted.contains(o) => {
                            Some(Instruction::Store { ptr: Operand::Val(addresses[&(*o, i)]), val: args[2].0.clone() })
                        }
                        _ => None,
                    }
                }
                Instruction::GetField { object: Operand::Val(o), index } if accepted.contains(o) => {
                    Some(Instruction::Load { ptr: Operand::Val(addresses[&(*o, *index)]), ty: *ty })
                }
                Instruction::SetField { object: Operand::Val(o), index, value } if accepted.contains(o) => {
                    Some(Instruction::Store { ptr: Operand::Val(addresses[&(*o, *index)]), val: value.clone() })
                }
                _ => None,
            };
            if let Some(n) = new {
                if matches!(n, Instruction::Store { .. }) {
                    *ty = Type::Void;
                }
                *inst = n;
            }
        }
    }
    func.blocks[0].instructions.splice(0..0, allocas);
    true
}
