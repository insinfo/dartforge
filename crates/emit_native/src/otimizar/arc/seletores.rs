//! Repasse uniforme pelo adaptador que exige destino registrado e readonly.

use super::*;

// O vetor de closure/seletor é construído pelo emissor a partir destas Ref;
// não aceita words implícitas nem uma tupla RTI sem produtor de proveniência.
// `getters` são os getters RTI comprovados do módulo (`caches_rti::getters_rti`).
// Sem módulo (`None`, produção de contratos de um corpo), a tupla vinda de
// `CallStatic` sem argumentos com retorno I64 é aceita só pela forma: o
// contrato é o mesmo, e a proveniência é exigida pelos gates com módulo
// (`conferir` no início da preparação e a certificação das portas AOT).
pub(super) fn conferir_vetor_local(
    f: &Function,
    inst: &Instruction,
    ty: Type,
    getters: Option<&HashSet<String>>,
) -> Result<bool, String> {
    let (closure, args, nomes, tupla_tipos, ret_ty, origem) = match inst {
        Instruction::CallClosure { closure, args, nomes, tupla_tipos, ret_ty } =>
            (closure, args, nomes, tupla_tipos, *ret_ty, "closure"),
        Instruction::CallSeletor { recv, args, nomes, tupla_tipos, .. } =>
            (recv, args, nomes, tupla_tipos, ty, "seletor"),
        _ => return Ok(false),
    };
    let tipos: HashMap<_, _> = f.params.iter().map(|(v, _, t)| (*v, *t))
        .chain(f.blocks.iter().flat_map(|b| &b.instructions).map(|(v, _, t)| (*v, *t))).collect();
    let referencia = |op: &Operand| matches!(op, Operand::Val(v) if tipos.get(v) == Some(&Type::Ref))
        || matches!(op, Operand::Constant(Constant::Null));
    // A avaliação auditada produz ID no universo RTI, que vive com a imagem
    // AOT; não é endereço do heap nem word Dart. Seus argumentos/receita ainda
    // passam pelos contratos runtime e pelos produtores dos caches/callees.
    // O getter de receita constante (`df.rti.*`) também produz ID RTI, mas
    // somente com o corpo e as escritas do cache comprovados, não pelo nome.
    let tupla_nativa = *tupla_tipos == Operand::Constant(Constant::Int(0))
        || matches!(tupla_tipos, Operand::Val(id) if f.blocks.iter().flat_map(|b| &b.instructions).any(|(v, i, t)|
            v == id && *t == Type::I64 && match i {
                Instruction::CallRuntime { name, ret_ty: Type::I64, .. } => name == "dartforge_rti_avaliar",
                Instruction::CallStatic { symbol, args, ret_ty: Type::I64 } =>
                    args.is_empty() && getters.is_none_or(|g| g.contains(symbol)),
                _ => false,
            }));
    if ty != Type::Ref || ret_ty != Type::Ref || !referencia(closure)
        || !args.iter().all(referencia) || nomes.len() > args.len()
        || nomes.windows(2).any(|par| par[0] >= par[1])
        || !tupla_nativa {
        return Err(format!("{origem} ARC em {}: argumentos Ref, descritor ou tupla RTI sem ABI conferida", f.symbol));
    }
    Ok(true)
}

pub(super) fn conferir(m: &Module, getters: &HashSet<String>) -> Result<(), String> {
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
                if conferir_vetor_local(f, i, *t, Some(getters))? {
                    let Terminator::CondBranch { then_block, .. } = b.terminator else {
                        return Err(format!("closure ARC: {} v{} exige guarda pending", f.symbol, v.0));
                    };
                    if super::classificacao::conferir_pendencia(b, *v, then_block).is_none()
                        || m.tabelas.get(indice).is_some_and(|t| t.invocacoes.contains_key(v)) {
                        return Err(format!("closure ARC: {} v{} exige pending, sem invoke LLVM", f.symbol, v.0));
                    }
                    continue;
                }
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
