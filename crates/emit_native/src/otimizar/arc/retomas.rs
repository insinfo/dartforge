//! Validação do par nativo que os cleanups ARC devolvem ao unwind Itanium.
use super::*;

/// Cleanup que preserva o par nativo: drops e fechamento auditado sem coleta.
pub(crate) fn instrucao_de_retoma(i: &Instruction) -> bool {
    match i {
        Instruction::ArcDrop { .. } => true,
        Instruction::CallRuntime { name, args, ret_ty } => {
            name == "dartforge_arc_quadro_fechar_v1"
                && *ret_ty == Type::Void
                && matches!(args.as_slice(), [(Operand::Val(_), Type::I64)])
        }
        _ => false,
    }
}

// O emissor e os tokens compartilham o contrato antes de referenciar o pouso.
pub(crate) fn conferir(f: &Function, t: &TabelasDaFuncao) -> Result<(), String> {
    if !t
        .saidas
        .values()
        .any(|modo| *modo == SaidaPorExcecao::Retoma)
    {
        return Ok(());
    }
    let mut retomas = t.clone();
    retomas
        .saidas
        .retain(|_, modo| *modo == SaidaPorExcecao::Retoma);
    if retomas.saidas.is_empty() {
        return Ok(());
    }
    tokens::conferir_saidas_excepcionais(f, &retomas, true)?;
    super::quadros::verificar(f)?;
    let mut invocacoes = HashMap::<BlockId, usize>::new();
    for id in t.invocacoes.values() {
        *invocacoes.entry(*id).or_default() += 1;
    }
    let mut chegadas = HashMap::<BlockId, usize>::new();
    for b in &f.blocks {
        for id in super::super::cfg::sucessores(&b.terminator) {
            if !retomas.saidas.contains_key(&id) {
                continue;
            }
            let coerente = matches!(&b.terminator,
                Terminator::CondBranch { cond: Operand::Constant(Constant::Bool(false)), then_block, else_block }
                if *then_block == id && *else_block != id)
                && b.instructions.last().is_some_and(|(v, i, _)| {
                    t.invocacoes.get(v) == Some(&id) && matches!(i, Instruction::CallStatic { .. })
                });
            if !coerente {
                return Err(format!("Retoma b{} tem entrada sem invoke", id.0));
            }
            *chegadas.entry(id).or_default() += 1;
        }
    }
    let mut ids: Vec<_> = retomas.saidas.keys().copied().collect();
    ids.sort_by_key(|id| id.0);
    for id in ids {
        if chegadas.get(&id) != Some(&1) || invocacoes.get(&id) != Some(&1) {
            return Err(format!("Retoma b{} exige exatamente um invoke", id.0));
        }
    }
    Ok(())
}
