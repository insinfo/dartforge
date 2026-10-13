//! Repasse uniforme pelo adaptador que exige destino registrado e readonly.

use super::*;

// O vetor de closure/seletor é construído pelo emissor a partir destas Ref;
// não aceita words implícitas nem uma tupla RTI sem produtor de proveniência.
// `getters` são os getters RTI comprovados do módulo (`rti_caches::getters_rti`).
// Sem módulo (`None`, produção de contratos de um corpo), a tupla vinda de
// `CallStatic` sem argumentos com retorno I64 é aceita só pela forma: o
// contrato é o mesmo, e a proveniência é exigida pelos gates com módulo
// (`check` no início da preparação e a certificação das portas AOT).
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
    // A avaliação auditada produz ID no universo RTI, que vive com a imagem
    // AOT; não é endereço do heap nem word Dart. Seus argumentos/receita ainda
    // passam pelos contratos runtime e pelos produtores dos caches/callees.
    // O getter de receita constante (`df.rti.*`) também produz ID RTI, mas
    // somente com o corpo e as escritas do cache comprovados, não pelo nome.
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
        return Err(format!("{origin} ARC em {}: argumentos Ref, descritor ou tupla RTI sem ABI conferida", f.symbol));
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
                        return Err(format!("closure ARC: {} v{} exige guarda pending", f.symbol, v.0));
                    };
                    if super::classification::check_pending(b, *v, then_block).is_none()
                        || m.tables.get(index).is_some_and(|t| t.invocations.contains_key(v)) {
                        return Err(format!("closure ARC: {} v{} exige pending, sem invoke LLVM", f.symbol, v.0));
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
                    || role(args) != Some(&DartEntryVector::Argumentos)
                    || role(desc) != Some(&DartEntryVector::Descriptor)
                {
                    return Err(format!(
                        "repasse ARC: {} v{} sem proveniência da ABI uniforme",
                        f.symbol, v.0
                    ));
                }
                let Terminator::CondBranch { then_block, .. } = b.terminator else {
                    return Err(format!(
                        "repasse ARC: {} v{} exige guarda pending",
                        f.symbol, v.0
                    ));
                };
                if super::classification::check_pending(b, *v, then_block).is_none() {
                    // A forma bruta tem o mesmo sufixo; conferir uma cópia com a
                    // convenção nova permite validar antes de publicar a mudança.
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
                            "repasse ARC: {} v{} exige guarda pending",
                            f.symbol, v.0
                        ));
                    }
                }
                if m.tables
                    .get(index)
                    .is_some_and(|t| t.invocations.contains_key(v))
                {
                    return Err(format!(
                        "repasse ARC: {} v{} deve conferir pending, sem invoke LLVM",
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
