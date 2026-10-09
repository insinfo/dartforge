//! Conferência dos limites de borrows no CFG preparado, incluindo arestas.
//! O lowering deverá fornecer o plano; não inferimos escopos por vivacidade.

use super::super::{
    cfg::Cfg,
    operandos::{operandos, operandos_do_terminador},
};
use super::{OrigemOwner, Ownership, vivacidade_classificada};
use crate::hir::*;
use std::collections::{HashMap, VecDeque};

/// Abertura ou fechamento léxico; zero designa a invocação inteira.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::AlteracaoEscopo;
/// let abrir = AlteracaoEscopo::Abrir(1);
/// assert_ne!(abrir, AlteracaoEscopo::Fechar(1));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlteracaoEscopo {
    /// Entra num escopo que ainda não está ativo.
    Abrir(u32),
    /// Sai exatamente do escopo mais interno.
    Fechar(u32),
}

/// Limites explícitos na mesma HIR que o inventário de ownership.
///
/// Alterações ocorrem antes da instrução, antes do terminador e na aresta,
/// nessa ordem. Arestas permitem representar cleanup sem atribuí-lo a todos
/// os sucessores. O escopo zero está sempre ativo e não pode ser fechado.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::PlanoEscopos;
/// let plano = PlanoEscopos::default();
/// assert!(plano.arestas.is_empty());
/// ```
#[derive(Debug, Clone, Default)]
pub struct PlanoEscopos {
    /// Alterações antes de uma definição identificada pelo ValueId.
    pub antes: HashMap<ValueId, Vec<AlteracaoEscopo>>,
    /// Alterações antes do terminador do bloco.
    pub saidas: HashMap<BlockId, Vec<AlteracaoEscopo>>,
    /// Alterações depois do terminador e antes dos Phi do sucessor.
    pub arestas: HashMap<(BlockId, BlockId), Vec<AlteracaoEscopo>>,
}

fn alterar(pilha: &mut Vec<u32>, eventos: &[AlteracaoEscopo]) -> Result<(), String> {
    for evento in eventos {
        match *evento {
            AlteracaoEscopo::Abrir(e) if !pilha.contains(&e) => pilha.push(e),
            AlteracaoEscopo::Fechar(e) if e != 0 && pilha.last() == Some(&e) => {
                pilha.pop();
            }
            _ => return Err(format!("limite inválido {evento:?}; ativos {pilha:?}")),
        }
    }
    Ok(())
}

fn conferir(
    mut v: ValueId,
    classes: &HashMap<ValueId, Ownership>,
    pilha: &[u32],
) -> Result<(), String> {
    // O inventário já foi conferido: a cadeia tem origem e não tem ciclos.
    while let Some(Ownership::Borrowed { escopo, owner }) = classes.get(&v) {
        if !pilha.contains(escopo) {
            return Err(format!(
                "v{} emprestado de {owner:?} fora do escopo {escopo}; ativos {pilha:?}",
                v.0
            ));
        }
        match owner {
            OrigemOwner::Valor(proximo) => v = *proximo,
            OrigemOwner::Chamador => break,
        }
    }
    Ok(())
}

/// Reconstrói só no erro o caminho da árvore de descoberta, sem guardar
/// uma cópia de todos os prefixos em cada bloco.
fn caminho(f: &Function, pais: &[Option<usize>], mut b: usize) -> Vec<u32> {
    let mut caminho = vec![f.blocks[b].id.0];
    while let Some(p) = pais[b] {
        b = p;
        caminho.push(f.blocks[b].id.0);
    }
    caminho.reverse();
    caminho
}

fn erro_no_fluxo(
    f: &Function,
    pais: &[Option<usize>],
    b: usize,
    aresta: Option<usize>,
    ponto: String,
    motivo: String,
) -> String {
    let mut caminho = caminho(f, pais, b);
    if let Some(s) = aresta {
        caminho.push(f.blocks[s].id.0);
    }
    format!(
        "ARC003 em {}: {ponto}: {motivo}; caminho {caminho:?}",
        f.symbol
    )
}

/// Confere limites e usos de borrows com pilhas iguais nas junções.
///
/// Exige CFG/SSA válidos e plano explícito da mesma função. Escopos zero dos
/// parâmetros duram a invocação. Phi lê a entrada depois dos eventos da sua
/// aresta. Cada uso confere também a cadeia de sustentação transitiva.
/// Esta análise não prova consumo de owners, contrato do tipo,
/// transferência no retorno nem disponibilidade excepcional de invoke.
///
/// # Erros
/// Retorna ARC003 com função e ponto se houver limite obsoleto, fechamento
/// fora de ordem, pilhas diferentes na junção, escopo aberto na saída ou
/// definição/uso borrowed fora de seu escopo. Ignora fluxo inalcançável,
/// mas rejeita chaves do plano ausentes da HIR.
/// Erros de fluxo incluem um caminho desde a entrada; junções incluem
/// os dois caminhos que apresentam pilhas distintas.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::*};
/// use std::collections::HashMap;
/// let f = Function { symbol: "f".into(), name: "f".into(), params: vec![],
///     return_ty: Type::Void, depuracao: None, blocks: vec![BasicBlock {
///     id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
/// verificar_escopos(&f, &HashMap::new(), &PlanoEscopos::default())?;
/// # Ok::<(), String>(())
/// ```
pub fn verificar_escopos(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    plano: &PlanoEscopos,
) -> Result<(), String> {
    vivacidade_classificada(f, classes)?;
    let cfg = Cfg::novo(f);
    let pos: HashMap<_, _> = f
        .blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id, i))
        .collect();
    let defs: HashMap<_, _> = f
        .blocks
        .iter()
        .flat_map(|b| b.instructions.iter().map(|(v, _, _)| (*v, b.id)))
        .collect();
    let erro = |ponto: String, motivo: String| format!("ARC003 em {}: {ponto}: {motivo}", f.symbol);
    let mut ids: Vec<_> = plano.antes.keys().copied().collect();
    ids.sort_by_key(|v| v.0);
    for id in ids {
        if !defs.contains_key(&id) {
            return Err(erro(format!("v{}", id.0), "limite sem instrução".into()));
        }
    }
    let mut ids: Vec<_> = plano.saidas.keys().copied().collect();
    ids.sort_by_key(|b| b.0);
    for id in ids {
        if !pos.contains_key(&id) {
            return Err(erro(format!("b{}", id.0), "limite sem bloco".into()));
        }
    }
    let mut edges: Vec<_> = plano.arestas.keys().copied().collect();
    edges.sort_by_key(|(a, b)| (a.0, b.0));
    for (de, para) in edges {
        if !pos
            .get(&de)
            .zip(pos.get(&para))
            .is_some_and(|(&a, &b)| cfg.sucessores[a].contains(&b))
        {
            return Err(erro(
                format!("b{} -> b{}", de.0, para.0),
                "limite sem aresta".into(),
            ));
        }
    }
    for (v, _, _) in &f.params {
        conferir(*v, classes, &[0]).map_err(|m| erro("parâmetro".into(), m))?;
    }
    if f.blocks.is_empty() {
        return Ok(());
    }
    let mut entradas: Vec<Option<Vec<u32>>> = vec![None; f.blocks.len()];
    let mut pais = vec![None; f.blocks.len()];
    entradas[0] = Some(vec![0]);
    let mut fila = VecDeque::from([0]);
    while let Some(i) = fila.pop_front() {
        let b = &f.blocks[i];
        let mut pilha = entradas[i].clone().unwrap();
        let ponto = |v: ValueId| format!("b{} v{}", b.id.0, v.0);
        for (v, inst, _) in &b.instructions {
            alterar(&mut pilha, plano.antes.get(v).map_or(&[], Vec::as_slice))
                .map_err(|m| erro_no_fluxo(f, &pais, i, None, ponto(*v), m))?;
            conferir(*v, classes, &pilha)
                .map_err(|m| erro_no_fluxo(f, &pais, i, None, ponto(*v), m))?;
            if !matches!(inst, Instruction::Phi { .. }) {
                let mut invalido = None;
                operandos(inst, &mut |o| {
                    if invalido.is_none()
                        && let Operand::Val(v) = o
                    {
                        invalido = conferir(*v, classes, &pilha).err();
                    }
                });
                if let Some(m) = invalido {
                    return Err(erro_no_fluxo(f, &pais, i, None, ponto(*v), m));
                }
            }
        }
        alterar(
            &mut pilha,
            plano.saidas.get(&b.id).map_or(&[], Vec::as_slice),
        )
        .map_err(|m| erro_no_fluxo(f, &pais, i, None, format!("b{} saída", b.id.0), m))?;
        let mut invalido = None;
        operandos_do_terminador(&b.terminator, &mut |o| {
            if invalido.is_none()
                && let Operand::Val(v) = o
            {
                invalido = conferir(*v, classes, &pilha).err();
            }
        });
        if let Some(m) = invalido {
            return Err(erro_no_fluxo(
                f,
                &pais,
                i,
                None,
                format!("b{} terminador", b.id.0),
                m,
            ));
        }
        if cfg.sucessores[i].is_empty() && pilha != [0] {
            return Err(erro_no_fluxo(
                f,
                &pais,
                i,
                None,
                format!("b{} saída", b.id.0),
                format!("escopos não fechados {pilha:?}"),
            ));
        }
        for &s in &cfg.sucessores[i] {
            let destino = &f.blocks[s];
            let ponto = format!("b{} -> b{}", b.id.0, destino.id.0);
            let mut proxima = pilha.clone();
            alterar(
                &mut proxima,
                plano
                    .arestas
                    .get(&(b.id, destino.id))
                    .map_or(&[], Vec::as_slice),
            )
            .map_err(|m| erro_no_fluxo(f, &pais, i, Some(s), ponto.clone(), m))?;
            for (_, inst, _) in &destino.instructions {
                if let Instruction::Phi { incoming, .. } = inst {
                    for (de, o) in incoming {
                        if *de == b.id
                            && let Operand::Val(v) = o
                        {
                            conferir(*v, classes, &proxima).map_err(|m| {
                                erro_no_fluxo(f, &pais, i, Some(s), ponto.clone(), m)
                            })?;
                        }
                    }
                }
            }
            match &entradas[s] {
                Some(anterior) if *anterior != proxima => {
                    return Err(erro_no_fluxo(
                        f,
                        &pais,
                        i,
                        Some(s),
                        ponto,
                        format!(
                            "junção com escopos diferentes {anterior:?} e {proxima:?}; caminho anterior {:?}",
                            caminho(f, &pais, s)
                        ),
                    ));
                }
                Some(_) => {}
                None => {
                    pais[s] = Some(i);
                    entradas[s] = Some(proxima);
                    fila.push_back(s);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::super::OrigemOwner;
    use super::*;

    fn exemplo() -> (Function, HashMap<ValueId, Ownership>, PlanoEscopos) {
        let f = Function {
            symbol: "escopos".into(),
            name: "escopos".into(),
            depuracao: None,
            params: vec![(ValueId(0), "dono".into(), Type::Ref)],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(1),
                        Instruction::Bitcast {
                            op: Operand::Val(ValueId(0)),
                            to: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(2),
                        Instruction::Bitcast {
                            op: Operand::Val(ValueId(1)),
                            to: Type::Ref,
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(None),
            }],
        };
        let classes = HashMap::from([
            (
                ValueId(0),
                Ownership::Borrowed {
                    owner: OrigemOwner::Chamador,
                    escopo: 0,
                },
            ),
            (
                ValueId(1),
                Ownership::Borrowed {
                    owner: OrigemOwner::Valor(ValueId(0)),
                    escopo: 1,
                },
            ),
            (ValueId(2), Ownership::Owned),
        ]);
        let plano = PlanoEscopos {
            antes: HashMap::from([(ValueId(1), vec![AlteracaoEscopo::Abrir(1)])]),
            saidas: HashMap::from([(BlockId(0), vec![AlteracaoEscopo::Fechar(1)])]),
            ..Default::default()
        };
        (f, classes, plano)
    }

    #[test]
    fn borrow_nao_pode_ser_definido_ou_usado_fora_do_limite() {
        let (f, classes, mut plano) = exemplo();
        verificar_escopos(&f, &classes, &plano).unwrap();
        plano.saidas.clear();
        plano
            .antes
            .insert(ValueId(2), vec![AlteracaoEscopo::Fechar(1)]);
        assert!(
            verificar_escopos(&f, &classes, &plano)
                .unwrap_err()
                .contains("v1 emprestado")
        );
        plano.antes.clear();
        assert!(
            verificar_escopos(&f, &classes, &plano)
                .unwrap_err()
                .contains("b0 v1")
        );
    }

    #[test]
    fn borrow_transitivo_nao_estende_o_limite_de_seu_owner() {
        let (mut f, mut classes, plano) = exemplo();
        f.return_ty = Type::Ref;
        classes.insert(
            ValueId(2),
            Ownership::Borrowed {
                owner: OrigemOwner::Valor(ValueId(1)),
                escopo: 0,
            },
        );
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(2))));
        let erro = verificar_escopos(&f, &classes, &plano).unwrap_err();
        assert!(
            erro.contains("b0 terminador")
                && erro.contains("v1 emprestado")
                && erro.contains("caminho [0]"),
            "{erro}"
        );
    }

    #[test]
    fn cleanup_de_aresta_precisa_concordar_na_juncao_e_no_backedge() {
        let (mut f, classes, mut plano) = exemplo();
        plano.saidas.clear();
        f.blocks[0].terminator = Terminator::CondBranch {
            cond: Operand::Constant(Constant::Bool(false)),
            then_block: BlockId(1),
            else_block: BlockId(2),
        };
        for i in 1..=3 {
            f.blocks.push(BasicBlock {
                id: BlockId(i),
                instructions: vec![],
                terminator: if i == 3 {
                    Terminator::Return(None)
                } else {
                    Terminator::Branch(BlockId(3))
                },
            });
        }
        plano
            .arestas
            .insert((BlockId(0), BlockId(1)), vec![AlteracaoEscopo::Fechar(1)]);
        plano
            .arestas
            .insert((BlockId(0), BlockId(2)), vec![AlteracaoEscopo::Fechar(1)]);
        verificar_escopos(&f, &classes, &plano).unwrap();
        plano.arestas.remove(&(BlockId(0), BlockId(2)));
        let erro = verificar_escopos(&f, &classes, &plano).unwrap_err();
        assert!(
            erro.contains("junção")
                && erro.contains("caminho anterior [0, 1, 3]")
                && erro.contains("caminho [0, 2, 3]"),
            "{erro}"
        );
        // A volta precisa fechar a ativação anterior antes de abrir a próxima.
        f.blocks.truncate(1);
        f.blocks[0].terminator = Terminator::Branch(BlockId(0));
        plano.arestas.clear();
        assert!(
            verificar_escopos(&f, &classes, &plano)
                .unwrap_err()
                .contains("junção")
        );
        plano
            .arestas
            .insert((BlockId(0), BlockId(0)), vec![AlteracaoEscopo::Fechar(1)]);
        verificar_escopos(&f, &classes, &plano).unwrap();
    }

    #[test]
    fn phi_usa_borrow_apos_cleanup_da_aresta_e_plano_obsoleto_falha() {
        let (mut f, mut classes, mut plano) = exemplo();
        plano.saidas.clear();
        f.blocks[0].terminator = Terminator::Branch(BlockId(1));
        f.blocks.push(BasicBlock {
            id: BlockId(1),
            instructions: vec![(
                ValueId(3),
                Instruction::Phi {
                    ty: Type::Ref,
                    incoming: vec![(BlockId(0), Operand::Val(ValueId(1)))],
                },
                Type::Ref,
            )],
            terminator: Terminator::Return(None),
        });
        classes.insert(ValueId(3), Ownership::Owned);
        plano
            .saidas
            .insert(BlockId(1), vec![AlteracaoEscopo::Fechar(1)]);
        verificar_escopos(&f, &classes, &plano).unwrap();
        plano
            .arestas
            .insert((BlockId(0), BlockId(1)), vec![AlteracaoEscopo::Fechar(1)]);
        assert!(
            verificar_escopos(&f, &classes, &plano)
                .unwrap_err()
                .contains("b0 -> b1: v1")
        );
        plano.antes.insert(ValueId(99), vec![]);
        assert!(
            verificar_escopos(&f, &classes, &plano)
                .unwrap_err()
                .contains("limite sem instrução")
        );
    }
}
