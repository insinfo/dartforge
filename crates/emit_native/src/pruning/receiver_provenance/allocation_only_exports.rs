//! Allocation-only export gate; publication and imported consumption are separate.
//!
//! The producer must bind this borrowed closure to the exact selected SDK cache
//! key and emitted object hashes before serializing it. Consumers additionally
//! reconcile every symbol, ABI and table against the actual linked app/SDK.
use super::*;
mod rti;

pub(super) struct AllocationExport<'a> {
    pub(super) symbol: &'a str,
    pub(super) parameters: Vec<Type>,
    pub(super) classes: Classes,
    /// Actual immutable bodies; serializers must preserve their artifact binding.
    pub(super) closure: Vec<&'a Function>,
}

/// Shared immutable evidence for the recursive allocation-export audit.
struct AuditGraph<'a, 'g> {
    proof: &'g Provenance<'a>,
    bodies: &'g HashMap<&'a str, &'a Function>,
    duplicates: &'g HashSet<&'a str>,
    rti: &'g rti::Inventory,
}

pub(super) fn candidates<'a>(
    modules: &[&'a Module],
    proof: &Provenance<'a>,
) -> Vec<AllocationExport<'a>> {
    if modules.iter().any(|m| {
        !m.arc_fixed_ancestry || m.arc_memory || m.arc_prepared || m.arc_aot_ports.is_some()
    }) {
        return Vec::new();
    }
    let mut bodies = HashMap::new();
    let mut duplicates = HashSet::new();
    for f in modules.iter().flat_map(|m| &m.functions) {
        if bodies.insert(f.symbol.as_str(), f).is_some() {
            duplicates.insert(f.symbol.as_str());
        }
    }
    let rti = rti::Inventory::new(modules);
    let graph = AuditGraph { proof, bodies: &bodies, duplicates: &duplicates, rti: &rti };
    let mut exports = Vec::new();
    for (&symbol, &body) in &bodies {
        let Some(classes) = proof.returns.get(symbol) else {
            continue;
        };
        if body.return_ty != Type::Ref
            || classes.iter().all(|cid| *cid == 1)
            || proof.values_for(body).is_none()
        {
            continue;
        }
        let mut visiting = HashSet::new();
        let mut completed = HashSet::new();
        let mut closure = Vec::new();
        if audit(
            body,
            &graph,
            &mut visiting,
            &mut completed,
            &mut closure,
        ) {
            exports.push(AllocationExport {
                symbol,
                parameters: body.params.iter().map(|(_, _, ty)| *ty).collect(),
                classes: classes.clone(),
                closure,
            });
        }
    }
    exports.sort_by_key(|export| export.symbol);
    exports
}

fn audit<'a>(
    body: &'a Function,
    graph: &AuditGraph<'a, '_>,
    visiting: &mut HashSet<&'a str>,
    completed: &mut HashSet<&'a str>,
    closure: &mut Vec<&'a Function>,
) -> bool {
    let AuditGraph { proof, bodies, duplicates, rti } = *graph;
    let symbol = body.symbol.as_str();
    if duplicates.contains(symbol) || symbol.starts_with("dfc.") || proof.values_for(body).is_none()
    {
        return false;
    }
    if completed.contains(symbol) {
        return true;
    }
    if !visiting.insert(symbol) {
        return false;
    }
    let Ok(shape) = Shape::new(body) else {
        return false;
    };
    let rti_helper = rti.helper(symbol);
    // Audit all blocks, including dead ones: a new optimization pass must not
    // resurrect excluded cache/address/global behavior behind this certificate.
    for (_, instruction, _) in body.blocks.iter().flat_map(|block| &block.instructions) {
        match instruction {
            I::Const(
                Constant::Null | Constant::Int(_) | Constant::Bool(_) | Constant::Double(_),
            )
            | I::AllocObject { .. }
            | I::AllocList { .. }
            | I::Phi { .. }
            | I::ICmp(..)
            | I::LNot(_) => {}
            I::CallStatic {
                symbol,
                args,
                ret_ty,
            } => {
                let Some(&callee) = bodies.get(symbol.as_str()) else {
                    return false;
                };
                if callee.return_ty != *ret_ty
                    || callee.params.len() != args.len()
                    || !callee
                        .params
                        .iter()
                        .zip(args)
                        .all(|((_, _, ty), arg)| shape.operand_type(arg) == Some(*ty))
                    || !audit(
                        callee, graph, visiting, completed, closure,
                    )
                {
                    return false;
                }
            }
            // This exact runtime ABI is already the allocation premise of the
            // body analysis. A user definition of the reserved symbol invalidates it.
            I::CallRuntime {
                name,
                args,
                ret_ty: Type::Ref,
            } if name == "dartforge_object_new" && !bodies.contains_key(name.as_str()) => {
                if !matches!(args.as_slice(), [
                    (Operand::Constant(Constant::Int(cid)), Type::I64),
                    (Operand::Constant(Constant::Int(count)), Type::I64),
                ] if (0..=i64::from(i32::MAX)).contains(cid) && usize::try_from(*count).is_ok())
                {
                    return false;
                }
            }
            I::Const(Constant::String(_) | Constant::StringWtf8(_)) if rti_helper => {}
            instruction
                if rti.scalar_access(instruction, &shape)
                    || rti.runtime_effect(instruction, &shape, rti_helper) => {}
            _ => return false,
        }
    }
    // No function pointers or symbol/address operands may hide in Phi/allocations.
    let mut forbidden_operand = false;
    for block in &body.blocks {
        for (_, instruction, _) in &block.instructions {
            operands(instruction, &mut |operand| {
                forbidden_operand |= matches!(operand, Operand::Constant(Constant::Function(_)));
            });
        }
        terminator_operands(&block.terminator, &mut |operand| {
            forbidden_operand |= matches!(operand, Operand::Constant(Constant::Function(_)));
        });
    }
    if !rti_helper {
        for block in &body.blocks {
            for (_, instruction, _) in &block.instructions {
                operands(instruction, &mut |operand| {
                    forbidden_operand |= matches!(
                        operand,
                        Operand::Constant(Constant::String(_) | Constant::StringWtf8(_))
                    );
                });
            }
            terminator_operands(&block.terminator, &mut |operand| {
                forbidden_operand |= matches!(
                    operand,
                    Operand::Constant(Constant::String(_) | Constant::StringWtf8(_))
                );
            });
        }
    }
    if forbidden_operand {
        return false;
    }
    visiting.remove(symbol);
    completed.insert(symbol);
    closure.push(body);
    true
}

#[cfg(test)]
mod tests;
