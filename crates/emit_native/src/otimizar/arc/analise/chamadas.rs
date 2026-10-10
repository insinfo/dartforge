//! Solução local monotônica de aliases normais por SCC de chamadas (§28.3).
//! Efeitos de heap/escape permanecem opacos; não certifica políticas ARC.

use super::resumos::{ResumoHeapArc, RetornoHeapArc, extrair_com_chamadas};
use crate::hir::*;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

/// Resume corpos alcançáveis pelas chamadas diretas das raízes fornecidas.
/// Alvos externos não são inventados. Bottom é interno: ciclos sem evidência
/// de retorno normal são elevados a topo antes de disponibilizar a SCC.
pub(super) fn resolver<'a>(
    m: &'a Module,
    raizes: &HashSet<&str>,
    limite: usize,
) -> Result<HashMap<String, (&'a Function, ResumoHeapArc)>, String> {
    let mut indice = HashMap::new();
    let mut posicoes = HashMap::new();
    for (posicao, f) in m.functions.iter().enumerate() {
        posicoes.insert(f.symbol.as_str(), posicao);
        if indice.insert(f.symbol.as_str(), f).is_some() {
            return Err(format!("resumo: corpo duplicado {}", f.symbol));
        }
    }
    let mut vistos = HashSet::new();
    let mut fila: VecDeque<_> = raizes.iter().copied().collect();
    let mut corpos = Vec::new();
    while let Some(s) = fila.pop_front() {
        if !vistos.insert(s) {
            continue;
        }
        let Some(&f) = indice.get(s) else { continue };
        corpos.push(f);
        fila.extend(alvos(f));
    }
    // Ordem nominal torna solução e diagnósticos independentes do HashSet.
    corpos.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    let ids: HashMap<_, _> = corpos
        .iter()
        .enumerate()
        .map(|(i, f)| (f.symbol.as_str(), i))
        .collect();
    let arestas: Vec<Vec<usize>> = corpos
        .iter()
        .map(|f| {
            let mut destinos: Vec<_> = alvos(f).filter_map(|s| ids.get(s).copied()).collect();
            destinos.sort_unstable();
            destinos.dedup();
            destinos
        })
        .collect();
    let mut dependentes = vec![Vec::new(); corpos.len()];
    for (v, destinos) in arestas.iter().enumerate() {
        for &w in destinos {
            dependentes[w].push(v);
        }
    }
    let vazio = || RetornoHeapArc::Aliases {
        parametros: BTreeSet::new(),
        nulo: false,
    };
    let mut retornos: Vec<_> = corpos
        .iter()
        .map(|f| {
            if f.return_ty == Type::Ref {
                vazio()
            } else {
                RetornoHeapArc::Desconhecido
            }
        })
        .collect();
    for componente in crate::otimizar::scc::componentes(&arestas) {
        let membros: HashSet<_> = componente.iter().copied().collect();
        let mut fila: VecDeque<_> = componente.iter().copied().collect();
        let mut enfileirados = membros.clone();
        loop {
            while let Some(v) = fila.pop_front() {
                enfileirados.remove(&v);
                let novo = extrair_com_chamadas(corpos[v], limite, true, |s| {
                    ids.get(s).map(|&w| (corpos[w], &retornos[w]))
                })?;
                if unir(&mut retornos[v], novo.retorno(), limite) {
                    for &w in &dependentes[v] {
                        if membros.contains(&w) && enfileirados.insert(w) {
                            fila.push_back(w);
                        }
                    }
                }
            }
            // Não expor bottom como ausência provada de aliases/null.
            let sem_evidencia: Vec<_> = componente
                .iter()
                .copied()
                .filter(|&v| retornos[v] == vazio())
                .collect();
            if sem_evidencia.is_empty() {
                break;
            }
            for v in sem_evidencia {
                retornos[v] = RetornoHeapArc::Desconhecido;
                for &w in &dependentes[v] {
                    if membros.contains(&w) && enfileirados.insert(w) {
                        fila.push_back(w);
                    }
                }
            }
        }
    }
    // Reextração com dependências estabilizadas constrói o resumo guardado
    // pela assinatura do próprio corpo, sem publicar aproximações da worklist.
    corpos
        .iter()
        .enumerate()
        .map(|(v, &f)| {
            let mut resumo = extrair_com_chamadas(f, limite, false, |s| {
                ids.get(s).map(|&w| (corpos[w], &retornos[w]))
            })?;
            let indice = posicoes[f.symbol.as_str()];
            let tabela = if m.excecoes_por_tabelas {
                m.tabelas.get(indice)
            } else {
                None
            };
            if (m.excecoes_por_tabelas && tabela.is_none())
                || crate::llvm::LlvmEmitter::exige_contexto_explicito(f, tabela)
            {
                resumo.invalidar_cobertura();
            }
            debug_assert_eq!(resumo.retorno(), &retornos[v]);
            Ok((f.symbol.clone(), (f, resumo)))
        })
        .collect()
}

fn alvos(f: &Function) -> impl Iterator<Item = &str> {
    f.blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter_map(|(_, i, _)| {
            if let Instruction::CallStatic { symbol, .. } = i {
                Some(symbol.as_str())
            } else {
                None
            }
        })
}

fn unir(destino: &mut RetornoHeapArc, origem: &RetornoHeapArc, limite: usize) -> bool {
    if *destino == RetornoHeapArc::Desconhecido {
        return false;
    }
    let anterior = destino.clone();
    match origem {
        RetornoHeapArc::Desconhecido => *destino = RetornoHeapArc::Desconhecido,
        RetornoHeapArc::Aliases {
            parametros: ps,
            nulo: ns,
        } => {
            if let RetornoHeapArc::Aliases { parametros, nulo } = destino {
                parametros.extend(ps);
                *nulo |= ns;
                if parametros.len() > limite {
                    *destino = RetornoHeapArc::Desconhecido;
                }
            }
        }
    }
    *destino != anterior
}

#[cfg(test)]
mod testes {
    use super::*;

    fn corpo(nome: &str, base: Option<Operand>, alvos: &[String]) -> Function {
        let mut opcoes: Vec<_> = base.into_iter().map(|o| (vec![], o)).collect();
        for (k, s) in alvos.iter().enumerate() {
            let v = ValueId(3 + k as u32);
            opcoes.push((
                vec![(
                    v,
                    Instruction::CallStatic {
                        symbol: s.clone(),
                        args: vec![
                            Operand::Val(ValueId(0)),
                            Operand::Val(ValueId(1)),
                            Operand::Val(ValueId(2)),
                        ],
                        ret_ty: Type::Ref,
                    },
                    Type::Ref,
                )],
                Operand::Val(v),
            ));
        }
        let mut blocks = Vec::new();
        let n = opcoes.len();
        for (k, (instructions, retorno)) in opcoes.into_iter().enumerate() {
            let id = BlockId(2 * k as u32);
            let ultimo = k + 1 == n;
            if !ultimo {
                blocks.push(BasicBlock {
                    id,
                    instructions: vec![],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(2)),
                        then_block: BlockId(id.0 + 1),
                        else_block: BlockId(id.0 + 2),
                    },
                });
            }
            blocks.push(BasicBlock {
                id: if ultimo { id } else { BlockId(id.0 + 1) },
                instructions,
                terminator: Terminator::Return(Some(retorno)),
            });
        }
        if blocks.is_empty() {
            blocks.push(BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(0)),
            });
        }
        Function {
            symbol: nome.into(),
            name: nome.into(),
            depuracao: None,
            params: vec![
                (ValueId(0), "x".into(), Type::Ref),
                (ValueId(1), "y".into(), Type::Ref),
                (ValueId(2), "cond".into(), Type::I1),
            ],
            return_ty: Type::Ref,
            blocks,
        }
    }

    #[test]
    fn scc_confere_aliases_com_alcance_independente_nos_512_grafos() {
        for mascara in 0..512 {
            let mut m = Module::new();
            let mut alcance = [[false; 3]; 3];
            for (v, linha) in alcance.iter_mut().enumerate() {
                linha[v] = true;
                let alvos: Vec<_> = (0..3)
                    .filter(|&w| mascara & (1 << (3 * v + w)) != 0)
                    .map(|w| {
                        linha[w] = true;
                        format!("f{w}")
                    })
                    .collect();
                m.functions.push(corpo(
                    &format!("f{v}"),
                    Some(Operand::Val(ValueId((v % 2) as u32))),
                    &alvos,
                ));
            }
            // Floyd-Warshall sobre o grafo original, sem usar a SCC/worklist.
            for k in 0..3 {
                for v in 0..3 {
                    for w in 0..3 {
                        alcance[v][w] |= alcance[v][k] && alcance[k][w];
                    }
                }
            }
            for limite in 0..=2 {
                let r = resolver(&m, &HashSet::from(["f0", "f1", "f2"]), limite).unwrap();
                for (v, linha) in alcance.iter().enumerate() {
                    let parametros: BTreeSet<_> =
                        (0..3).filter(|&w| linha[w]).map(|w| w % 2).collect();
                    let esperado = if parametros.len() > limite {
                        RetornoHeapArc::Desconhecido
                    } else {
                        RetornoHeapArc::Aliases {
                            parametros,
                            nulo: false,
                        }
                    };
                    assert_eq!(
                        r[&format!("f{v}")].1.retorno(),
                        &esperado,
                        "grafo {mascara}, limite {limite}, nó {v}"
                    );
                }
            }
        }
    }

    #[test]
    fn argumentos_permutados_na_recursao_unem_aliases_reais() {
        let mut m = Module::new();
        m.functions.push(corpo("f", None, &["g".into()]));
        let mut g = corpo("g", Some(Operand::Val(ValueId(0))), &["f".into()]);
        let chamada = g
            .blocks
            .last_mut()
            .unwrap()
            .instructions
            .last_mut()
            .unwrap();
        if let Instruction::CallStatic { args, .. } = &mut chamada.1 {
            args.swap(0, 1);
        }
        m.functions.push(g);
        let r = resolver(&m, &HashSet::from(["f"]), 2).unwrap();
        assert!(r.values().all(|(_, r)| r.retorno()
            == &RetornoHeapArc::Aliases {
                parametros: BTreeSet::from([0, 1]),
                nulo: false,
            }));
        let r = resolver(&m, &HashSet::from(["f"]), 1).unwrap();
        assert!(
            r.values()
                .all(|(_, r)| r.retorno() == &RetornoHeapArc::Desconhecido)
        );
    }

    #[test]
    fn recursao_null_sem_base_e_alvo_externo_nao_fabricam_fatos_vazios() {
        let mut m = Module::new();
        m.functions.push(corpo("f", None, &["g".into()]));
        m.functions.push(corpo(
            "g",
            Some(Operand::Constant(Constant::Null)),
            &["f".into()],
        ));
        let r = resolver(&m, &HashSet::from(["f"]), 8).unwrap();
        for (_, resumo) in r.values() {
            assert_eq!(
                resumo.retorno(),
                &RetornoHeapArc::Aliases {
                    parametros: BTreeSet::new(),
                    nulo: true
                }
            );
        }
        m.functions[1] = corpo("g", None, &["f".into()]);
        let r = resolver(&m, &HashSet::from(["f"]), 8).unwrap();
        assert!(
            r.values()
                .all(|(_, r)| r.retorno() == &RetornoHeapArc::Desconhecido)
        );
        m.functions[1] = corpo("g", Some(Operand::Val(ValueId(0))), &["externo".into()]);
        let r = resolver(&m, &HashSet::from(["f"]), 8).unwrap();
        assert!(
            r.values()
                .all(|(_, r)| r.retorno() == &RetornoHeapArc::Desconhecido)
        );
        assert!(!r.contains_key("externo"));
    }

    #[test]
    fn cadeia_profunda_independe_da_ordem_dos_corpos_sem_recursao_rust() {
        let mut m = Module::new();
        for v in 0..2000 {
            m.functions.push(if v == 1999 {
                corpo(&format!("f{v}"), Some(Operand::Val(ValueId(1))), &[])
            } else {
                corpo(&format!("f{v}"), None, &[format!("f{}", v + 1)])
            });
        }
        for _ in 0..2 {
            let r = resolver(&m, &HashSet::from(["f0"]), 8).unwrap();
            assert_eq!(r.len(), 2000);
            assert!(r.values().all(|(_, r)| r.retorno()
                == &RetornoHeapArc::Aliases {
                    parametros: BTreeSet::from([1]),
                    nulo: false
                }));
            m.functions.reverse();
        }
    }
}
