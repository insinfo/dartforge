//! Solução local monotônica de aliases normais por SCC de chamadas (§28.3).
//! Compõe preservação/publicação de corpos positivos por chamadas diretas.
//! Demais efeitos permanecem opacos; não certifica políticas ARC.

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
    // Declaração é premissa de ABI, independente do orçamento de precisão.
    for f in &corpos {
        conferir_globais(m, f)?;
    }
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
    let mut contexto_opaco = vec![false; corpos.len()];
    let mut resumos: Vec<_> = corpos
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
                || crate::llvm::LlvmEmitter::pode_exigir_contexto_por_raizes(f)
                || tabela.is_some_and(|t| {
                    !t.invocacoes.is_empty()
                        || !t.pousos.is_empty()
                        || !t.saidas.is_empty()
                        || t.cleanup_estrangeiro.is_some()
                })
            {
                contexto_opaco[v] = true;
                resumo.invalidar_cobertura();
            }
            debug_assert_eq!(resumo.retorno(), &retornos[v]);
            Ok(resumo)
        })
        .collect::<Result<_, String>>()?;
    // O estado inicial é desconhecido nas chamadas. Só promover quando
    // todas as dependências já têm cobertura positiva; nunca supor uma
    // SCC recursiva pura apenas porque nenhum store foi encontrado nela.
    let mut fila: VecDeque<_> = (0..corpos.len()).collect();
    let mut enfileirados: HashSet<_> = (0..corpos.len()).collect();
    while let Some(v) = fila.pop_front() {
        enfileirados.remove(&v);
        if contexto_opaco[v]
            || (resumos[v].campos() == super::resumos::CamposHeapArc::Preservados
                && resumos[v].publicacoes() == super::resumos::PublicacoesHeapArc::SomenteResultado)
        {
            continue;
        }
        if super::resumos::cobertura_com_chamadas(corpos[v], |s| {
            ids.get(s).map(|&w| (corpos[w], &resumos[w]))
        }) {
            resumos[v].marcar_cobertura();
            for &w in &dependentes[v] {
                if enfileirados.insert(w) {
                    fila.push_back(w);
                }
            }
        }
    }
    // Inclusões de stores têm ponto fixo próprio, sem promover a cobertura
    // de campos/publicações nem ignorar prólogos/saídas opacos. Recursão
    // propaga uniões; desconhecimento e orçamento excedido são absorventes.
    let mut escritas = vec![super::escritas::InclusoesEscritas::vazio(); corpos.len()];
    let mut globais = vec![super::escritas::InclusoesPublicacoes::vazio(); corpos.len()];
    let mut caminhos: Vec<_> = corpos
        .iter()
        .map(|f| {
            if f.return_ty == Type::Ref {
                super::escritas::RetornoCaminhos::vazio()
            } else {
                super::escritas::RetornoCaminhos::desconhecido()
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
                let (resultado, novo, publicacoes) =
                    super::escritas::extrair_com_efeitos(corpos[v], limite, true, |s| {
                        ids.get(s).map(|&w| {
                            (
                                corpos[w],
                                caminhos[w].clone(),
                                &escritas[w],
                                globais[w].clone(),
                            )
                        })
                    })?;
                let mudou_escrita = escritas[v].unir(&novo, limite);
                let mudou_global = globais[v].unir(&publicacoes, limite);
                if caminhos[v].unir(&resultado, limite) || mudou_escrita || mudou_global {
                    for &w in &dependentes[v] {
                        if membros.contains(&w) && enfileirados.insert(w) {
                            fila.push_back(w);
                        }
                    }
                }
            }
            let sem_evidencia: Vec<_> = componente
                .iter()
                .copied()
                .filter(|&v| caminhos[v] == super::escritas::RetornoCaminhos::vazio())
                .collect();
            if sem_evidencia.is_empty() {
                break;
            }
            for v in sem_evidencia {
                caminhos[v] = super::escritas::RetornoCaminhos::desconhecido();
                for &w in &dependentes[v] {
                    if membros.contains(&w) && enfileirados.insert(w) {
                        fila.push_back(w);
                    }
                }
            }
        }
    }
    for (((resumo, inclusoes), retorno), publicacoes) in
        resumos.iter_mut().zip(escritas).zip(caminhos).zip(globais)
    {
        resumo.inclusoes_escritas = inclusoes;
        resumo.retorno_caminhos = retorno;
        resumo.inclusoes_publicacoes = publicacoes;
    }
    Ok(corpos
        .into_iter()
        .zip(resumos)
        .map(|(f, resumo)| (f.symbol.clone(), (f, resumo)))
        .collect())
}

pub(super) fn conferir_globais(m: &Module, f: &Function) -> Result<(), String> {
    for (_, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
        let simbolo = match i {
            Instruction::StoreGlobal {
                simbolo,
                ty: Type::Ref,
                ..
            }
            | Instruction::ArcStoreStrong {
                slot: SlotForte::Global { simbolo },
                ..
            }
            | Instruction::ArcLoadStrong {
                slot: SlotForte::Global { simbolo },
            } => Some(simbolo),
            _ => None,
        };
        if let Some(simbolo) = simbolo {
            let declaracoes: Vec<_> = m.globais.iter().filter(|(_, _, s)| s == simbolo).collect();
            if declaracoes.len() != 1 || declaracoes[0].1 != Type::Ref {
                return Err(format!(
                    "publicacoes ARC: global @{simbolo} exige declaração Ref única"
                ));
            }
        }
    }
    Ok(())
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

    fn escalar(nome: &str, alvo: Option<&str>) -> Function {
        Function {
            symbol: nome.into(),
            name: nome.into(),
            depuracao: None,
            params: vec![(ValueId(0), "x".into(), Type::I64)],
            return_ty: Type::I64,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: alvo
                    .into_iter()
                    .map(|symbol| {
                        (
                            ValueId(1),
                            Instruction::CallStatic {
                                symbol: symbol.into(),
                                args: vec![Operand::Val(ValueId(0))],
                                ret_ty: Type::I64,
                            },
                            Type::I64,
                        )
                    })
                    .collect(),
                terminator: Terminator::Return(Some(Operand::Val(ValueId(if alvo.is_some() {
                    1
                } else {
                    0
                })))),
            }],
        }
    }

    #[test]
    fn cobertura_composta_cadeia_profunda_e_ordem_dos_corpos() {
        let mut m = Module::new();
        for v in 0..1000 {
            let alvo = (v < 999).then(|| format!("f{}", v + 1));
            m.functions.push(escalar(&format!("f{v}"), alvo.as_deref()));
        }
        for _ in 0..2 {
            let r = resolver(&m, &HashSet::from(["f0"]), 8).unwrap();
            assert_eq!(r.len(), 1000);
            for (_, resumo) in r.values() {
                assert_eq!(
                    resumo.campos(),
                    super::super::resumos::CamposHeapArc::Preservados
                );
                assert_eq!(
                    resumo.publicacoes(),
                    super::super::resumos::PublicacoesHeapArc::SomenteResultado
                );
            }
            m.functions.reverse();
        }
    }

    #[test]
    fn cobertura_composta_recusa_dependencia_opaca_recursao_e_abi() {
        use super::super::resumos::*;
        let base = vec![
            escalar("f", Some("g")),
            escalar("g", Some("h")),
            escalar("h", None),
        ];
        for caso in 0..4 {
            let mut m = Module::new();
            m.functions = base.clone();
            match caso {
                0 => m.functions[2] = escalar("h", Some("externo")),
                1 => m.functions[2] = escalar("h", Some("g")),
                2 => {
                    m.functions[2].blocks[0].terminator =
                        Terminator::Throw(Operand::Constant(Constant::Null))
                }
                3 => {
                    if let Instruction::CallStatic { args, .. } =
                        &mut m.functions[1].blocks[0].instructions[0].1
                    {
                        args[0] = Operand::Constant(Constant::Null);
                    }
                }
                _ => unreachable!(),
            }
            let r = resolver(&m, &HashSet::from(["f"]), 8).unwrap();
            assert_eq!(
                r["f"].1.campos(),
                CamposHeapArc::Desconhecidos,
                "caso {caso}"
            );
            assert_eq!(
                r["f"].1.publicacoes(),
                PublicacoesHeapArc::Desconhecidas,
                "caso {caso}"
            );
        }
    }

    #[test]
    fn cobertura_composta_nao_reaprova_contexto_ou_tabela_ausente() {
        use super::super::resumos::*;
        let mut m = Module::new();
        m.functions = vec![escalar("f", Some("g")), escalar("g", None)];
        m.excecoes_por_tabelas = true;
        for tabelas in [
            vec![],
            vec![
                TabelasDaFuncao::default(),
                TabelasDaFuncao {
                    confere_pilha: true,
                    ..Default::default()
                },
            ],
        ] {
            m.tabelas = tabelas;
            let r = resolver(&m, &HashSet::from(["f"]), 8).unwrap();
            assert_eq!(r["f"].1.campos(), CamposHeapArc::Desconhecidos);
            assert_eq!(r["g"].1.publicacoes(), PublicacoesHeapArc::Desconhecidas);
        }
        m.tabelas = vec![TabelasDaFuncao::default(); 2];
        let r = resolver(&m, &HashSet::from(["f"]), 8).unwrap();
        assert_eq!(r["f"].1.campos(), CamposHeapArc::Preservados);
        // Alterar o próprio prólogo impede a propagação mesmo com g coberto.
        m.tabelas[0].confere_pilha = true;
        let r = resolver(&m, &HashSet::from(["f"]), 8).unwrap();
        assert_eq!(r["f"].1.campos(), CamposHeapArc::Desconhecidos);
        assert_eq!(r["g"].1.campos(), CamposHeapArc::Preservados);
    }

    #[test]
    fn cobertura_composta_consumida_no_modulo_invalida_dependencia_transitiva() {
        use super::super::{escape::*, modulo::analisar_no_modulo};
        let mut m = Module::new();
        m.functions = vec![
            escalar("f", Some("g")),
            escalar("g", Some("h")),
            escalar("h", None),
        ];
        assert!(analisar_no_modulo(&m, "f", 8).unwrap().is_none());
        m.memoria_arc = true;
        let a = analisar_no_modulo(&m, "f", 8).unwrap().unwrap();
        assert!(a.desconhecidos().is_empty());
        let e = calcular_no_modulo(&m, "f", &a).unwrap();
        assert!(
            !e.publicacoes
                .keys()
                .any(|c| matches!(c, CausaEscape::OperacaoOpaca(_)))
        );
        // Efeito implícito de materialização em h, apesar do retorno escalar:
        // invalida a solução antiga de f e torna a nova chamada opaca.
        m.functions[2].blocks[0].instructions.push((
            ValueId(2),
            Instruction::Const(Constant::String("novo efeito".into())),
            Type::Ref,
        ));
        assert!(calcular_no_modulo(&m, "f", &a).is_err());
        let nova = analisar_no_modulo(&m, "f", 8).unwrap().unwrap();
        assert_eq!(nova.desconhecidos(), &[ValueId(1)]);
        assert!(
            calcular_no_modulo(&m, "f", &nova)
                .unwrap()
                .publicacoes
                .contains_key(&CausaEscape::OperacaoOpaca(1))
        );
    }

    #[test]
    fn cadeia_com_calculo_preserva_campos_e_recusa_conversao_ou_efeito_falivel() {
        use super::super::{escape::*, modulo::analisar_no_modulo, resumos::*};
        let mut m = Module::new();
        m.memoria_arc = true;
        m.layouts_campos_arc.insert(1, vec![Type::Ref]);
        m.layouts_campos_arc.insert(2, vec![]);
        m.functions.push(Function {
            symbol: "caller".into(),
            name: "caller".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(0),
                        Instruction::AllocObject {
                            class_id: 2,
                            fields: vec![],
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(1),
                        Instruction::AllocObject {
                            class_id: 1,
                            fields: vec![Operand::Val(ValueId(0))],
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(2),
                        Instruction::CallStatic {
                            symbol: "f0".into(),
                            args: vec![Operand::Constant(Constant::Int(4))],
                            ret_ty: Type::I64,
                        },
                        Type::I64,
                    ),
                    (
                        ValueId(3),
                        Instruction::GetField {
                            object: Operand::Val(ValueId(1)),
                            index: 0,
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
            }],
        });
        super::super::origens::registrar(&mut m, 0, true);
        for v in 0..64 {
            let alvo = (v < 63).then(|| format!("f{}", v + 1));
            m.functions.push(escalar(&format!("f{v}"), alvo.as_deref()));
        }
        m.functions[64].blocks[0].instructions.push((
            ValueId(1),
            Instruction::Add(
                Operand::Val(ValueId(0)),
                Operand::Constant(Constant::Int(1)),
            ),
            Type::I64,
        ));
        m.functions[64].blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
        for _ in 0..2 {
            let r = resolver(&m, &HashSet::from(["f0"]), 8).unwrap();
            for (_, resumo) in r.values() {
                assert_eq!(resumo.campos(), CamposHeapArc::Preservados);
                assert_eq!(resumo.publicacoes(), PublicacoesHeapArc::SomenteResultado);
                assert_eq!(resumo.retorno(), &RetornoHeapArc::Desconhecido);
            }
            let a = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
            assert_eq!(a.valor(ValueId(0)), a.valor(ValueId(3)));
            assert_eq!(a.valor(ValueId(3)).unwrap().nos().unwrap().len(), 1);
            assert!(a.desconhecidos().is_empty());
            // Resultado I64 não recebe prova de representação trivial/frescor.
            assert!(a.valor(ValueId(2)).unwrap().nos().is_none());
            assert!(
                !calcular_no_modulo(&m, "caller", &a)
                    .unwrap()
                    .publicacoes
                    .contains_key(&CausaEscape::OperacaoOpaca(2))
            );
            m.functions.reverse();
            calcular_no_modulo(&m, "caller", &a).unwrap();
        }
        let anterior = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
        for i in [
            Instruction::SDiv(
                Operand::Val(ValueId(0)),
                Operand::Constant(Constant::Int(0)),
            ),
            Instruction::SRem(
                Operand::Val(ValueId(0)),
                Operand::Constant(Constant::Int(0)),
            ),
        ] {
            m.functions[64].blocks[0].instructions[0].1 = i;
            assert!(calcular_no_modulo(&m, "caller", &anterior).is_err());
            let a = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
            assert!(a.valor(ValueId(3)).unwrap().nos().is_none());
            assert_eq!(a.desconhecidos(), &[ValueId(2)]);
            assert!(
                calcular_no_modulo(&m, "caller", &a)
                    .unwrap()
                    .publicacoes
                    .contains_key(&CausaEscape::OperacaoOpaca(2))
            );
        }
        m.functions[64].blocks[0].instructions[0].1 = Instruction::Add(
            Operand::Val(ValueId(0)),
            Operand::Constant(Constant::Int(1)),
        );
        for arg in [
            Constant::Double(4.0),
            Constant::Null,
            Constant::String("4".into()),
        ] {
            if let Instruction::CallStatic { args, .. } =
                &mut m.functions[0].blocks[0].instructions[2].1
            {
                args[0] = Operand::Constant(arg);
            }
            let a = analisar_no_modulo(&m, "caller", 8).unwrap().unwrap();
            assert!(a.valor(ValueId(3)).unwrap().nos().is_none());
            assert_eq!(a.desconhecidos(), &[ValueId(2)]);
        }
        m.memoria_arc = false;
        assert!(analisar_no_modulo(&m, "caller", 8).unwrap().is_none());
    }

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
