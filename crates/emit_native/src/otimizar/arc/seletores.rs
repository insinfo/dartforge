//! Repasse uniforme pelo adaptador que exige destino registrado e readonly.

use super::*;

pub(super) fn conferir(m: &Module) -> Result<(), String> {
    for (indice, f) in m.functions.iter().enumerate() {
        let tipos: HashMap<_, _> = f
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
        let papeis = m.parametros_vetores_dart.get(&f.symbol);
        let papel = |o: &Operand| match o {
            Operand::Val(v) => papeis.and_then(|p| p.get(v)),
            _ => None,
        };
        for b in &f.blocks {
            for (v, i, t) in &b.instructions {
                let (recv, args, desc) = match i {
                    Instruction::CallSeletorRepasse {
                        recv, args, desc, ..
                    }
                    | Instruction::CallClosureRepasse {
                        closure: recv,
                        args,
                        desc,
                        ..
                    } => (recv, args, desc),
                    _ => continue,
                };
                let ref_valida = matches!(recv, Operand::Val(v) if tipos.get(v) == Some(&Type::Ref))
                    || matches!(recv, Operand::Constant(Constant::Null));
                if *t != Type::Ref
                    || !ref_valida
                    || papel(args) != Some(&VetorEntradaDart::Argumentos)
                    || papel(desc) != Some(&VetorEntradaDart::Descritor)
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
                if super::classificacao::conferir_pendencia(b, *v, then_block).is_none() {
                    // A forma bruta tem o mesmo sufixo; conferir uma cópia com a
                    // convenção nova permite validar antes de publicar a mudança.
                    let mut copia = b.clone();
                    for (_, i, _) in &mut copia.instructions {
                        if let Instruction::CallSeletorRepasse { arc_owned, .. }
                        | Instruction::CallClosureRepasse { arc_owned, .. } = i
                        {
                            *arc_owned = true;
                        }
                    }
                    if super::classificacao::conferir_pendencia(&copia, *v, then_block).is_none() {
                        return Err(format!(
                            "repasse ARC: {} v{} exige guarda pending",
                            f.symbol, v.0
                        ));
                    }
                }
                if m.tabelas
                    .get(indice)
                    .is_some_and(|t| t.invocacoes.contains_key(v))
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

pub(super) fn preparar(funcoes: &mut [Function]) {
    for f in funcoes {
        for (_, i, _) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
            if let Instruction::CallSeletorRepasse { arc_owned, .. }
            | Instruction::CallClosureRepasse { arc_owned, .. } = i
            {
                *arc_owned = true;
            }
        }
    }
}
