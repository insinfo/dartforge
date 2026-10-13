//! Uniform forwarding through the adapter that requires a registered, readonly destination.

use super::*;

// The closure/selector vector is built by the emitter from these Refs;
// it accepts neither implicit words nor an RTI tuple without a provenance producer.
// `getters` are the proven RTI getters of the module (`rti_caches::getters_rti`).
// Without a module (`None`, contract production for one body), the tuple coming from
// an argument-less `CallStatic` returning I64 is accepted by shape alone: the
// contract is the same, and the provenance is required by the gates with a module
// (`check` at the start of the preparation and the certification of the AOT ports).
pub(super) fn check_local_vector(
    f: &Function,
    inst: &Instruction,
    ty: Type,
    getters: Option<&HashSet<String>>,
) -> Result<bool, String> {
    let (closure, args, names, tuple_types, ret_ty, origin) = match inst {
        Instruction::CallClosure { closure, args, names, tuple_types, ret_ty } =>
            (closure, args, names, tuple_types, *ret_ty, "closure"),
        Instruction::CallSelector { recv, args, names, tuple_types, .. } =>
            (recv, args, names, tuple_types, ty, "seletor"),
        _ => return Ok(false),
    };
    let types: HashMap<_, _> = f.params.iter().map(|(v, _, t)| (*v, *t))
        .chain(f.blocks.iter().flat_map(|b| &b.instructions).map(|(v, _, t)| (*v, *t))).collect();
    let reference = |op: &Operand| matches!(op, Operand::Val(v) if types.get(v) == Some(&Type::Ref))
        || matches!(op, Operand::Constant(Constant::Null));
    // The audited evaluation produces an ID in the RTI universe, which lives with
    // the AOT image; it is neither a heap address nor a Dart word. Its arguments/recipe
    // still go through the runtime contracts and the producers of the caches/callees.
    // The constant recipe getter (`df.rti.*`) also produces an RTI ID, but
    // only with its body and the cache writes proven, not by its name.
    let native_tuple = *tuple_types == Operand::Constant(Constant::Int(0))
        || matches!(tuple_types, Operand::Val(id) if f.blocks.iter().flat_map(|b| &b.instructions).any(|(v, i, t)|
            v == id && *t == Type::I64 && match i {
                Instruction::CallRuntime { name, ret_ty: Type::I64, .. } => name == "dartforge_rti_avaliar",
                Instruction::CallStatic { symbol, args, ret_ty: Type::I64 } =>
                    args.is_empty() && getters.is_none_or(|g| g.contains(symbol)),
                _ => false,
            }));
    if ty != Type::Ref || ret_ty != Type::Ref || !reference(closure)
        || !args.iter().all(reference) || names.len() > args.len()
        || names.windows(2).any(|par| par[0] >= par[1])
        || !native_tuple {
        return Err(format!("ARC {origin} in {}: Ref arguments, descriptor or RTI tuple without a checked ABI", f.symbol));
    }
    Ok(true)
}

pub(super) fn check(m: &Module, getters: &HashSet<String>) -> Result<(), String> {
    for (index, f) in m.functions.iter().enumerate() {
        let types: HashMap<_, _> = f
            .params
            .iter()
            .map(|(v, _, t)| (*v, *t))
            .chain(
                f.blocks
                    .iter()
                    .flat_map(|b| &b.instructions)
                    .map(|(v, _, t)| (*v, *t)),
            )
            .collect();
        let roles = m.dart_vector_parameters.get(&f.symbol);
        let role = |o: &Operand| match o {
            Operand::Val(v) => roles.and_then(|p| p.get(v)),
            _ => None,
        };
        for b in &f.blocks {
            for (v, i, t) in &b.instructions {
                if check_local_vector(f, i, *t, Some(getters))? {
                    let Terminator::CondBranch { then_block, .. } = b.terminator else {
                        return Err(format!("ARC closure: {} v{} requires a pending guard", f.symbol, v.0));
                    };
                    if super::classification::check_pending(b, *v, then_block).is_none()
                        || m.tables.get(index).is_some_and(|t| t.invocations.contains_key(v)) {
                        return Err(format!("ARC closure: {} v{} requires pending, without an LLVM invoke", f.symbol, v.0));
                    }
                    continue;
                }
                let (recv, args, desc) = match i {
                    Instruction::CallSelectorForward {
                        recv, args, desc, ..
                    }
                    | Instruction::CallClosureForward {
                        closure: recv,
                        args,
                        desc,
                        ..
                    } => (recv, args, desc),
                    _ => continue,
                };
                let valid_ref = matches!(recv, Operand::Val(v) if types.get(v) == Some(&Type::Ref))
                    || matches!(recv, Operand::Constant(Constant::Null));
                if *t != Type::Ref
                    || !valid_ref
                    || role(args) != Some(&DartEntryVector::Arguments)
                    || role(desc) != Some(&DartEntryVector::Descriptor)
                {
                    return Err(format!(
                        "ARC forwarding: {} v{} without provenance from the uniform ABI",
                        f.symbol, v.0
                    ));
                }
                let Terminator::CondBranch { then_block, .. } = b.terminator else {
                    return Err(format!(
                        "ARC forwarding: {} v{} requires a pending guard",
                        f.symbol, v.0
                    ));
                };
                if super::classification::check_pending(b, *v, then_block).is_none() {
                    // The raw shape has the same suffix; checking a copy with the
                    // new convention allows validating before publishing the change.
                    let mut copy_inst = b.clone();
                    for (_, i, _) in &mut copy_inst.instructions {
                        if let Instruction::CallSelectorForward { arc_owned, .. }
                        | Instruction::CallClosureForward { arc_owned, .. } = i
                        {
                            *arc_owned = true;
                        }
                    }
                    if super::classification::check_pending(&copy_inst, *v, then_block).is_none() {
                        return Err(format!(
                            "ARC forwarding: {} v{} requires a pending guard",
                            f.symbol, v.0
                        ));
                    }
                }
                if m.tables
                    .get(index)
                    .is_some_and(|t| t.invocations.contains_key(v))
                {
                    return Err(format!(
                        "ARC forwarding: {} v{} must check pending, without an LLVM invoke",
                        f.symbol, v.0
                    ));
                }
            }
        }
    }
    Ok(())
}

pub(super) fn prepare(functions: &mut [Function]) {
    for f in functions {
        for (_, i, _) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
            if let Instruction::CallSelectorForward { arc_owned, .. }
            | Instruction::CallClosureForward { arc_owned, .. } = i
            {
                *arc_owned = true;
            }
        }
    }
}
