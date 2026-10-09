//! Disponibilidade linear de tokens no CFG preparado, com Phi por aresta.
//! O inventário e os efeitos são contratos semânticos fornecidos pelo produtor.
//! Esta análise não certifica externs, slots, regiões ou invalidação de borrows.

use super::super::{
    cfg::Cfg,
    operandos::{operandos, operandos_do_terminador},
};
use super::{OrigemOwner, Ownership};
use crate::hir::*;
use std::collections::{HashMap, HashSet, VecDeque};

/// Consumo explícito de argumentos de uma instrução ordinária.
///
/// As listas preservam multiplicidade: consumir o mesmo token duas vezes
/// exige duas cópias distintas. Não há contrato implícito para chamada ausente.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::EfeitoTokens;
/// let efeito = EfeitoTokens::default();
/// assert!(!efeito.pode_falhar && efeito.erro.is_empty());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EfeitoTokens {
    /// Consumo tanto no sucesso quanto na falha, antes do resultado.
    pub sempre: Vec<ValueId>,
    /// Consumo apenas no sucesso.
    pub sucesso: Vec<ValueId>,
    /// Consumo apenas na falha de invoke ou conferência de pendência runtime.
    pub erro: Vec<ValueId>,
    /// Exige saída excepcional em invoke ou no mapa de pendências do plano.
    pub pode_falhar: bool,
}

/// Contrato de transferência do resultado ao chamador.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::RetornoTokens;
/// assert_ne!(RetornoTokens::Owned, RetornoTokens::Borrowed);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RetornoTokens {
    /// Resultado sem token gerenciado (inclusive funções void).
    #[default]
    Trivial,
    /// Transfere um token owned; null não exige contagem física.
    Owned,
    /// Empréstimo sustentado pelo chamador, nunca por owner local.
    Borrowed,
}

/// Contratos na mesma versão do CFG e do inventário de ownership.
///
/// Fornecer uma entrada por instrução ordinária, também sem consumo; as
/// operações ARC e Phi têm regras próprias e não aceitam sobrescrita.
/// Um plano ausente não significa que argumentos sejam emprestados.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::PlanoTokens;
/// assert!(PlanoTokens::default().instrucoes.is_empty());
/// assert!(PlanoTokens::default().pendencias.is_empty());
/// ```
#[derive(Debug, Clone, Default)]
pub struct PlanoTokens {
    /// Efeitos exatos antes do resultado e nas saídas de invoke/pendência.
    pub instrucoes: HashMap<ValueId, EfeitoTokens>,
    /// Convenção do retorno, não deduzida da largura da representação.
    pub retorno: RetornoTokens,
    /// Chamada runtime pending e bloco de erro após conferência explícita.
    /// Não são pousos LLVM. Exige sufixo exception_pending/ICmp Ne zero e
    /// CondBranch para erro/sucesso, sem outra operação entre chamada e teste.
    pub pendencias: HashMap<ValueId, BlockId>,
}

fn propria(inst: &Instruction) -> bool {
    matches!(
        inst,
        Instruction::ArcCopy { .. }
            | Instruction::ArcMove { .. }
            | Instruction::ArcDrop { .. }
            | Instruction::ArcLoadStrong { .. }
            | Instruction::ArcStoreStrong { .. }
            | Instruction::Phi { .. }
    )
}

fn conferir(
    v: ValueId,
    classes: &HashMap<ValueId, Ownership>,
    ativos: &HashSet<ValueId>,
) -> Result<(), String> {
    let mut atual = v;
    loop {
        match classes[&atual] {
            Ownership::Trivial => return Ok(()),
            Ownership::Owned if ativos.contains(&atual) => return Ok(()),
            Ownership::Owned => {
                return Err(format!(
                    "v{} depende do token indisponível v{}",
                    v.0, atual.0
                ));
            }
            Ownership::Borrowed {
                owner: OrigemOwner::Chamador,
                ..
            } => return Ok(()),
            Ownership::Borrowed {
                owner: OrigemOwner::Valor(owner),
                ..
            } => atual = owner,
        }
    }
}

fn usar(
    op: &Operand,
    classes: &HashMap<ValueId, Ownership>,
    ativos: &HashSet<ValueId>,
) -> Result<(), String> {
    if let Operand::Val(v) = op {
        conferir(*v, classes, ativos)?;
    }
    Ok(())
}

fn consumir(
    v: ValueId,
    classes: &HashMap<ValueId, Ownership>,
    ativos: &mut HashSet<ValueId>,
) -> Result<(), String> {
    if classes[&v] != Ownership::Owned {
        return Err(format!("consumo de v{} exige Owned", v.0));
    }
    if !ativos.remove(&v) {
        return Err(format!("token v{} já consumido ou indisponível", v.0));
    }
    Ok(())
}

fn consumir_operando(
    op: &Operand,
    classes: &HashMap<ValueId, Ownership>,
    ativos: &mut HashSet<ValueId>,
) -> Result<(), String> {
    match op {
        Operand::Val(v) => consumir(*v, classes, ativos),
        Operand::Constant(Constant::Null) => Ok(()),
        _ => Err("consumo exige token SSA ou null".into()),
    }
}

fn aplicar(
    ids: &[ValueId],
    classes: &HashMap<ValueId, Ownership>,
    ativos: &mut HashSet<ValueId>,
) -> Result<(), String> {
    for &v in ids {
        consumir(v, classes, ativos)?;
    }
    Ok(())
}

fn produzir(
    v: ValueId,
    classes: &HashMap<ValueId, Ownership>,
    ativos: &mut HashSet<ValueId>,
) -> Result<(), String> {
    if classes[&v] == Ownership::Owned && !ativos.insert(v) {
        return Err(format!(
            "nova produção de v{} com token anterior ainda disponível",
            v.0
        ));
    }
    conferir(v, classes, ativos)
}

fn do_chamador(mut v: ValueId, classes: &HashMap<ValueId, Ownership>) -> bool {
    loop {
        match classes[&v] {
            Ownership::Borrowed {
                owner: OrigemOwner::Chamador,
                ..
            } => return true,
            Ownership::Borrowed {
                owner: OrigemOwner::Valor(owner),
                ..
            } => v = owner,
            _ => return false,
        }
    }
}

/// O inventário já excluiu ciclos; aliases emprestados conservam a raiz.
fn raiz_owner(mut v: ValueId, classes: &HashMap<ValueId, Ownership>) -> Option<OrigemOwner> {
    loop {
        match classes[&v] {
            Ownership::Trivial => return None,
            Ownership::Owned => return Some(OrigemOwner::Valor(v)),
            Ownership::Borrowed {
                owner: OrigemOwner::Chamador,
                ..
            } => return Some(OrigemOwner::Chamador),
            Ownership::Borrowed {
                owner: OrigemOwner::Valor(proximo),
                ..
            } => v = proximo,
        }
    }
}

/// Limites léxicos herdados também precisam sobreviver à criação de um alias.
fn escopos_owner(mut v: ValueId, classes: &HashMap<ValueId, Ownership>) -> HashSet<u32> {
    let mut escopos = HashSet::new();
    while let Ownership::Borrowed { owner, escopo } = classes[&v] {
        if escopo != 0 {
            escopos.insert(escopo);
        }
        match owner {
            OrigemOwner::Chamador => break,
            OrigemOwner::Valor(proximo) => v = proximo,
        }
    }
    escopos
}

fn caminho(f: &Function, pais: &[Option<usize>], mut b: usize) -> Vec<u32> {
    let mut r = vec![f.blocks[b].id.0];
    while let Some(pai) = pais[b] {
        b = pai;
        r.push(f.blocks[b].id.0);
    }
    r.reverse();
    r
}

fn erro_fluxo(f: &Function, pais: &[Option<usize>], b: usize, mensagem: String) -> String {
    format!(
        "tokens em {} b{}: {mensagem}; caminho {:?}",
        f.symbol,
        f.blocks[b].id.0,
        caminho(f, pais, b)
    )
}

/// Confere disponibilidade, transferência e consumo único no CFG preparado.
///
/// Requer CFG/SSA e representações válidos, inventário semântico completo e
/// contratos previamente certificados pelo produtor. Invoke só produz token
/// no sucesso. Phi owned consome entradas simultaneamente na aresta, inclusive
/// trocas em backedges; null produz uma obrigação lógica sem RC físico.
/// Junções exigem inventários iguais, sem união que esconda consumo condicional.
/// Saídas devem consumir todos os tokens locais, transferindo o retorno owned.
/// Borrowed só pode voltar quando sua cadeia termina no chamador.
///
/// Não insere operações, certifica os contratos, verifica escopos léxicos,
/// invalidação de slots/borrows, regiões ou limpeza de estados suspensos.
/// Essas obrigações continuam necessárias para aceitar o pipeline ARC inteiro.
///
/// # Erros
/// Retorna símbolo, ponto e caminho para uso/consumo indisponível, fuga de token,
/// junção incompatível, retorno inválido ou metadado ausente/obsoleto. Também
/// propaga diagnósticos de cobertura, dependências e forma excepcional.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "f".into(), name: "f".into(), depuracao: None,
///     params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Void,
///     blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
///         (ValueId(1), Instruction::ArcCopy { value: Operand::Val(ValueId(0)) }, Type::Ref),
///         (ValueId(2), Instruction::ArcDrop { value: Operand::Val(ValueId(1)) }, Type::Void)],
///         terminator: Terminator::Return(None) }] };
/// let classes = HashMap::from([(ValueId(0), Ownership::Borrowed {
///     owner: OrigemOwner::Chamador, escopo: 0 }), (ValueId(1), Ownership::Owned),
///     (ValueId(2), Ownership::Trivial)]);
/// verificar_tokens(&f, &classes, &TabelasDaFuncao::default(), &PlanoTokens::default())?;
/// # Ok::<(), String>(())
/// ```
pub fn verificar_tokens(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    tabelas: &TabelasDaFuncao,
    plano: &PlanoTokens,
) -> Result<(), String> {
    analisar_tokens(f, classes, tabelas, plano, false).map(|_| ())
}

/// Confere o mesmo fluxo, admitindo somente tokens restantes em Return.
/// Não relaxa consumo duplicado, disponibilidade, convenção ou junções.
pub(super) fn saidas_para_cleanup(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    tabelas: &TabelasDaFuncao,
    plano: &PlanoTokens,
) -> Result<HashMap<BlockId, Vec<ValueId>>, String> {
    analisar_tokens(f, classes, tabelas, plano, true)
}

pub(super) fn conferir_saidas_excepcionais(
    f: &Function,
    tabelas: &TabelasDaFuncao,
    exigir_canonico: bool,
) -> Result<(), String> {
    let mut saidas: Vec<_> = tabelas.saidas.iter().collect();
    saidas.sort_by_key(|(b, _)| b.0);
    for (id, modo) in saidas {
        let b = f
            .blocks
            .iter()
            .find(|b| b.id == *id)
            .ok_or_else(|| format!("saída excepcional obsoleta b{}", id.0))?;
        let Terminator::Return(op) = &b.terminator else {
            return Err(format!("saída excepcional b{} não é Return", id.0));
        };
        if *modo == SaidaPorExcecao::Guarda {
            return Err(format!(
                "saída Guarda b{} exige CFG explícito antes do ARC",
                id.0
            ));
        }
        let esperado = if f.return_ty == Type::Void {
            None
        } else {
            Some(super::super::operandos::constante_padrao(f.return_ty))
        };
        if exigir_canonico && *op != esperado {
            return Err(format!(
                "saída Lanca b{} exige retorno canônico sem transferência",
                id.0
            ));
        }
    }
    Ok(())
}

// Só normaliza corpos temporários: o chamador publica após verificar tudo.
pub(super) fn normalizar_saidas_lanca(
    f: &mut Function,
    tabelas: &TabelasDaFuncao,
) -> Result<(), String> {
    super::ssa::verificar(f)?;
    conferir_saidas_excepcionais(f, tabelas, false)?;
    for b in &mut f.blocks {
        if tabelas.saidas.get(&b.id) == Some(&SaidaPorExcecao::Lanca) {
            b.terminator = Terminator::Return(if f.return_ty == Type::Void {
                None
            } else {
                Some(super::super::operandos::constante_padrao(f.return_ty))
            });
        }
    }
    Ok(())
}

fn analisar_tokens(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    tabelas: &TabelasDaFuncao,
    plano: &PlanoTokens,
    permitir_cleanup: bool,
) -> Result<HashMap<BlockId, Vec<ValueId>>, String> {
    conferir_saidas_excepcionais(f, tabelas, true)?;
    super::classificacao::vivacidade_com_saidas(f, classes, tabelas, &plano.pendencias)?;
    let erro_meta = |m: String| format!("tokens em {}: {m}", f.symbol);
    let mut esperados = HashSet::new();
    for b in &f.blocks {
        for (v, inst, _) in &b.instructions {
            if propria(inst) {
                if let Instruction::ArcLoadStrong {
                    slot:
                        SlotForte::Quadro {
                            quadro: Operand::Val(q),
                            ..
                        },
                }
                | Instruction::ArcStoreStrong {
                    slot:
                        SlotForte::Quadro {
                            quadro: Operand::Val(q),
                            ..
                        },
                    ..
                } = inst
                {
                    if classes[q] != Ownership::Trivial {
                        return Err(erro_meta(format!(
                            "v{}: ID de quadro deve ser Trivial",
                            v.0
                        )));
                    }
                }
                let classe = if matches!(
                    inst,
                    Instruction::ArcDrop { .. } | Instruction::ArcStoreStrong { .. }
                ) {
                    Some(Ownership::Trivial)
                } else if matches!(
                    inst,
                    Instruction::ArcCopy { .. }
                        | Instruction::ArcMove { .. }
                        | Instruction::ArcLoadStrong { .. }
                ) {
                    Some(Ownership::Owned)
                } else {
                    None
                };
                if classe.is_some_and(|c| classes[v] != c) || tabelas.invocacoes.contains_key(v) {
                    return Err(erro_meta(format!(
                        "v{}: contrato incompatível com operação ARC/Phi",
                        v.0
                    )));
                }
                continue;
            }
            esperados.insert(*v);
            let e = plano
                .instrucoes
                .get(v)
                .ok_or_else(|| erro_meta(format!("v{} sem contrato de consumo", v.0)))?;
            if let Instruction::CallRuntime { name, .. } = inst {
                if dartforge_runtime::ownership::contrato(name).is_ok() {
                    let c = super::contrato_chamada_runtime(inst).map_err(erro_meta)?;
                    if classes[v] != c.resultado
                        || e.sempre != c.efeito.sempre
                        || e.sucesso != c.efeito.sucesso
                        || e.erro != c.efeito.erro
                        || e.pode_falhar != c.efeito.pode_falhar
                    {
                        return Err(erro_meta(format!(
                            "v{}: plano incompatível com ownership.tsv para {name}",
                            v.0
                        )));
                    }
                }
            }
            if e.pode_falhar
                != (tabelas.invocacoes.contains_key(v) || plano.pendencias.contains_key(v))
                || (!e.pode_falhar && !e.erro.is_empty())
            {
                return Err(erro_meta(format!(
                    "v{}: contrato não corresponde às saídas excepcionais",
                    v.0
                )));
            }
            let mut args = HashSet::new();
            operandos(inst, &mut |o| {
                if let Operand::Val(v) = o {
                    args.insert(*v);
                }
            });
            for &c in e.sempre.iter().chain(&e.sucesso).chain(&e.erro) {
                if !args.contains(&c) {
                    return Err(erro_meta(format!(
                        "v{}: consumo de v{} não é argumento",
                        v.0, c.0
                    )));
                }
            }
        }
    }
    let mut extras: Vec<_> = plano
        .instrucoes
        .keys()
        .filter(|v| !esperados.contains(v))
        .copied()
        .collect();
    extras.sort_by_key(|v| v.0);
    if let Some(v) = extras.first() {
        return Err(erro_meta(format!(
            "v{}: contrato obsoleto ou sobrescreve regra ARC/Phi",
            v.0
        )));
    }
    if f.blocks.is_empty() {
        return Err(erro_meta("função sem entrada".into()));
    }
    let cfg = Cfg::novo(f);
    let mut saidas = HashMap::new();
    let mut entradas: Vec<Option<HashSet<ValueId>>> = vec![None; f.blocks.len()];
    entradas[0] = Some(
        f.params
            .iter()
            .filter(|(v, _, _)| classes[v] == Ownership::Owned)
            .map(|(v, _, _)| *v)
            .collect(),
    );
    let mut pais = vec![None; f.blocks.len()];
    let mut fila = VecDeque::from([0]);
    while let Some(i) = fila.pop_front() {
        let b = &f.blocks[i];
        let mut ativos = entradas[i].clone().unwrap();
        let mut invoke = None;
        for (v, inst, _) in &b.instructions {
            let resultado = (|| -> Result<(), String> {
                if matches!(inst, Instruction::Phi { .. }) {
                    return conferir(*v, classes, &ativos);
                }
                let mut falha = None;
                operandos(inst, &mut |o| {
                    if falha.is_none() {
                        falha = usar(o, classes, &ativos).err();
                    }
                });
                if let Some(m) = falha {
                    return Err(m);
                }
                match inst {
                    Instruction::ArcCopy { .. } | Instruction::ArcLoadStrong { .. } => {
                        produzir(*v, classes, &mut ativos)
                    }
                    Instruction::ArcStoreStrong { value, modo, .. } => {
                        if *modo == ModoStoreForte::Move {
                            consumir_operando(value, classes, &mut ativos)
                        } else {
                            Ok(())
                        }
                    }
                    Instruction::ArcMove { value } => {
                        consumir_operando(value, classes, &mut ativos)?;
                        produzir(*v, classes, &mut ativos)
                    }
                    Instruction::ArcDrop { value } => {
                        consumir_operando(value, classes, &mut ativos)
                    }
                    _ => {
                        let e = &plano.instrucoes[v];
                        aplicar(&e.sempre, classes, &mut ativos)?;
                        if e.pode_falhar {
                            invoke = Some(*v);
                            Ok(())
                        } else {
                            aplicar(&e.sucesso, classes, &mut ativos)?;
                            produzir(*v, classes, &mut ativos)
                        }
                    }
                }
            })();
            resultado.map_err(|m| erro_fluxo(f, &pais, i, format!("v{}: {m}", v.0)))?;
        }
        let mut falha = None;
        operandos_do_terminador(&b.terminator, &mut |o| {
            if falha.is_none() {
                falha = usar(o, classes, &ativos).err();
            }
        });
        if let Some(m) = falha {
            return Err(erro_fluxo(f, &pais, i, m));
        }
        if let Terminator::Return(valor) = &b.terminator
            && tabelas.saidas.get(&b.id) != Some(&SaidaPorExcecao::Lanca)
        {
            match (plano.retorno, valor) {
                (RetornoTokens::Owned, Some(v)) => consumir_operando(v, classes, &mut ativos)
                    .map_err(|m| erro_fluxo(f, &pais, i, m))?,
                (RetornoTokens::Borrowed, Some(Operand::Val(v))) if do_chamador(*v, classes) => {}
                (RetornoTokens::Borrowed, Some(Operand::Constant(Constant::Null))) => {}
                (RetornoTokens::Trivial, None) => {}
                (RetornoTokens::Trivial, Some(Operand::Val(v)))
                    if classes[v] == Ownership::Trivial => {}
                (RetornoTokens::Trivial, Some(Operand::Constant(c)))
                    if !matches!(c, Constant::String(_) | Constant::StringWtf8(_)) => {}
                _ => {
                    return Err(erro_fluxo(
                        f,
                        &pais,
                        i,
                        "retorno não satisfaz a convenção de tokens".into(),
                    ));
                }
            }
        }
        if cfg.sucessores[i].is_empty() && !ativos.is_empty() {
            if permitir_cleanup && matches!(b.terminator, Terminator::Return(_)) {
                let mut restantes: Vec<_> = ativos.iter().copied().collect();
                restantes.sort_unstable_by_key(|v| v.0);
                saidas.insert(b.id, restantes);
                continue;
            }
            let mut ids: Vec<_> = ativos.iter().map(|v| v.0).collect();
            ids.sort_unstable();
            return Err(erro_fluxo(
                f,
                &pais,
                i,
                format!("tokens não consumidos na saída {ids:?}"),
            ));
        }
        for &s in &cfg.sucessores[i] {
            let destino = &f.blocks[s];
            let proxima = (|| -> Result<HashSet<ValueId>, String> {
                let mut r = ativos.clone();
                if let Some(v) = invoke {
                    let e = &plano.instrucoes[&v];
                    if tabelas
                        .invocacoes
                        .get(&v)
                        .or_else(|| plano.pendencias.get(&v))
                        == Some(&destino.id)
                    {
                        aplicar(&e.erro, classes, &mut r)?;
                    } else {
                        aplicar(&e.sucesso, classes, &mut r)?;
                        produzir(v, classes, &mut r)?;
                    }
                }
                let mut consumidos = Vec::new();
                let mut produzidos = Vec::new();
                for (v, inst, _) in &destino.instructions {
                    if let Instruction::Phi { incoming, .. } = inst {
                        for (de, op) in incoming {
                            if *de != b.id {
                                continue;
                            }
                            usar(op, classes, &r)?;
                            if matches!(classes[v], Ownership::Borrowed { .. }) {
                                let compativel = match op {
                                    Operand::Constant(Constant::Null) => true,
                                    Operand::Val(entrada) => {
                                        classes[entrada] == Ownership::Trivial
                                            || (raiz_owner(*entrada, classes)
                                                == raiz_owner(*v, classes)
                                                && escopos_owner(*entrada, classes)
                                                    .is_subset(&escopos_owner(*v, classes)))
                                    }
                                    _ => false,
                                };
                                if !compativel {
                                    return Err(format!(
                                        "Phi borrowed v{}: owner ou escopo incompatível na entrada",
                                        v.0
                                    ));
                                }
                            }
                            if classes[v] == Ownership::Owned {
                                match op {
                                    Operand::Val(entrada)
                                        if classes[entrada] == Ownership::Owned =>
                                    {
                                        consumidos.push(*entrada)
                                    }
                                    Operand::Constant(Constant::Null) => {}
                                    _ => {
                                        return Err(format!(
                                            "Phi v{} exige entrada owned ou null",
                                            v.0
                                        ));
                                    }
                                }
                                produzidos.push(*v);
                            }
                        }
                    }
                }
                aplicar(&consumidos, classes, &mut r)?;
                for v in produzidos {
                    produzir(v, classes, &mut r)?;
                }
                Ok(r)
            })()
            .map_err(|m| {
                erro_fluxo(
                    f,
                    &pais,
                    i,
                    format!("aresta b{} -> b{}: {m}", b.id.0, destino.id.0),
                )
            })?;
            match &entradas[s] {
                Some(anterior) if *anterior != proxima => {
                    return Err(erro_fluxo(
                        f,
                        &pais,
                        i,
                        format!(
                            "junção b{} com tokens diferentes; caminho anterior {:?}",
                            destino.id.0,
                            caminho(f, &pais, s)
                        ),
                    ));
                }
                Some(_) => {}
                None => {
                    entradas[s] = Some(proxima);
                    pais[s] = Some(i);
                    fila.push_back(s);
                }
            }
        }
    }
    Ok(saidas)
}

#[cfg(test)]
mod testes;
