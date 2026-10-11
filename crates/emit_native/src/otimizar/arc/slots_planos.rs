//! Papéis nativos dos quadros compartilhados, derivados de um grupo verificado.
//! O contrato sela a assinatura e os repasses; mutações Ref exigem revalidar CFG.

use super::*;
use std::sync::Arc;

#[derive(Debug)]
struct Grupo {
    originais: Vec<Function>,
    funcoes: Vec<Function>,
    emprestados: HashMap<String, HashMap<ValueId, ValueId>>,
    locais: HashMap<String, (ValueId, usize)>,
    simbolos: HashMap<String, String>,
    identidade: String,
}

/// Contrato dos papéis nativos de uma família fechada de capturas Ref.
///
/// Criado somente após conferir origens, todos os chamadores e vida explícita
/// dos quadros. Não é um owner Ref nem certificado de unwind. Campos privados
/// impedem fabricar empréstimos a partir de parâmetros i64 arbitrários.
/// A assinatura e os repasses precisam permanecer iguais; demais alterações
/// ainda passam pela conferência atual de tipos, escape, CFG e cleanup.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::PlanoTokens;
/// assert!(PlanoTokens::default().quadros_compartilhados.is_none());
/// ```
#[derive(Debug, Clone)]
pub struct ContratoQuadrosCompartilhadosCfg {
    grupo: Arc<Grupo>,
    simbolo: String,
    assinatura: Vec<(ValueId, Type)>,
    retorno: Type,
    repasses: Vec<(ValueId, String, Vec<Operand>)>,
}

impl PartialEq for ContratoQuadrosCompartilhadosCfg {
    fn eq(&self, outro: &Self) -> bool {
        self.simbolo == outro.simbolo
            && self.assinatura == outro.assinatura
            && self.retorno == outro.retorno
            && self.repasses == outro.repasses
            && (Arc::ptr_eq(&self.grupo, &outro.grupo)
                || (self.grupo.identidade == outro.grupo.identidade
                    && self.grupo.emprestados == outro.grupo.emprestados
                    && self.grupo.locais == outro.grupo.locais
                    && self.grupo.simbolos == outro.grupo.simbolos))
    }
}
impl Eq for ContratoQuadrosCompartilhadosCfg {}

fn repasses(f: &Function, grupo: &Grupo) -> Vec<(ValueId, String, Vec<Operand>)> {
    let mut encontrados: Vec<_> = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter_map(|(v, i, _)| {
            let Instruction::CallStatic { symbol, args, .. } = i else {
                return None;
            };
            let pares = grupo.emprestados.get(symbol)?;
            let callee = grupo.funcoes.iter().find(|f| f.symbol == *symbol)?;
            let mut posicoes: Vec<_> = callee
                .params
                .iter()
                .enumerate()
                .filter_map(|(p, (q, _, _))| {
                    pares.get(q).map(|i| {
                        (
                            p,
                            callee.params.iter().position(|(v, _, _)| v == i).unwrap(),
                        )
                    })
                })
                .collect();
            posicoes.sort_unstable();
            let valores = posicoes
                .into_iter()
                .flat_map(|(q, i)| [args.get(q).cloned(), args.get(i).cloned()])
                .collect::<Option<Vec<_>>>()?;
            Some((*v, symbol.clone(), valores))
        })
        .collect();
    encontrados.sort_by_key(|(v, _, _)| v.0);
    encontrados
}

impl ContratoQuadrosCompartilhadosCfg {
    pub(super) fn conferir(&self, f: &Function) -> Result<(), String> {
        if f.symbol != self.simbolo
            || f.return_ty != self.retorno
            || f.params
                .iter()
                .map(|(v, _, t)| (*v, *t))
                .collect::<Vec<_>>()
                != self.assinatura
            || repasses(f, &self.grupo) != self.repasses
        {
            return Err(format!(
                "quadros compartilhados: assinatura/repasses obsoletos de {}",
                f.symbol
            ));
        }
        let mut m = Module::new();
        m.memoria_arc = true;
        m.functions = self.grupo.originais.clone();
        let mut r = RascunhoSlotsRefArc {
            funcoes: self.grupo.funcoes.clone(),
            emprestados: self.grupo.emprestados.clone(),
            locais: self.grupo.locais.clone(),
            simbolos: self.grupo.simbolos.clone(),
        };
        let atual = r
            .funcoes
            .iter_mut()
            .find(|c| c.symbol == f.symbol)
            .ok_or("quadros compartilhados: corpo fora da família")?;
        *atual = f.clone();
        verificar_vidas_slots_ref_cfg(&m, &r)
    }

    pub(super) fn pares(&self) -> &HashMap<ValueId, ValueId> {
        static VAZIO: std::sync::LazyLock<HashMap<ValueId, ValueId>> =
            std::sync::LazyLock::new(HashMap::new);
        self.grupo.emprestados.get(&self.simbolo).unwrap_or(&VAZIO)
    }

    pub(super) fn alvos(&self) -> HashMap<String, Vec<(usize, usize)>> {
        self.grupo
            .emprestados
            .iter()
            .map(|(s, ps)| {
                let f = self.grupo.funcoes.iter().find(|f| f.symbol == *s).unwrap();
                let posicoes: HashMap<_, _> = f
                    .params
                    .iter()
                    .enumerate()
                    .map(|(p, (v, _, _))| (*v, p))
                    .collect();
                let mut ordem: Vec<_> =
                    ps.iter().map(|(q, i)| (posicoes[q], posicoes[i])).collect();
                ordem.sort_unstable();
                (s.clone(), ordem)
            })
            .collect()
    }
}

/// Produz atomicamente papéis nativos e contratos de quadros nos planos.
///
/// Exige os corpos intermediários e seus planos por símbolo novo. Classifica
/// apenas quadro/índice importados como Trivial, sem registrá-los como escalares
/// Dart. Os quadros locais continuam sendo produzidos pela ABI auditada.
/// O contrato permite revalidar o CFG atual após inserir operações Ref;
/// não permite mudar assinatura, repasses ou publicar um endereço da família.
/// Não publica corpos, tabelas do módulo ou certificado de unwind estrangeiro.
///
/// # Erros
/// Falha do grupo, plano ausente/obsoleto ou classe conflitante. Nenhuma
/// classificação ou contrato é publicado em erro.
///
/// ```
/// use dartforge_emit_native::{hir::Module, otimizar::arc::*};
/// use std::collections::HashMap;
/// let mut m = Module::new(); m.memoria_arc = true;
/// let r = baixar_slots_ref_compartilhados(&m)?;
/// assert_eq!(produzir_planos_slots_ref_cfg(&m, &r, &mut HashMap::new())?, 0);
/// # Ok::<(), String>(())
/// ```
pub fn produzir_planos_slots_ref_cfg(
    m: &Module,
    r: &RascunhoSlotsRefArc,
    planos: &mut HashMap<String, PlanoFuncaoDart>,
) -> Result<usize, String> {
    verificar_vidas_slots_ref_cfg(m, r)?;
    let nomes: HashSet<_> = r.funcoes.iter().map(|f| f.symbol.as_str()).collect();
    if planos.len() != nomes.len() || planos.keys().any(|s| !nomes.contains(s.as_str())) {
        return Err("quadros compartilhados: inventário de planos incompatível".into());
    }
    if r.emprestados.is_empty() {
        return Ok(0);
    }
    let membros: HashSet<_> = r
        .emprestados
        .keys()
        .chain(r.locais.keys())
        .cloned()
        .collect();
    let originais: Vec<_> = m
        .functions
        .iter()
        .filter(|f| membros.contains(r.simbolos.get(&f.symbol).unwrap_or(&f.symbol)))
        .cloned()
        .collect();
    let funcoes: Vec<_> = r
        .funcoes
        .iter()
        .filter(|f| membros.contains(&f.symbol))
        .cloned()
        .collect();
    let identidade = format!("{originais:?}");
    let grupo = Arc::new(Grupo {
        originais,
        funcoes,
        emprestados: r.emprestados.clone(),
        locais: r.locais.clone(),
        simbolos: r.simbolos.clone(),
        identidade,
    });
    let mut novos = planos.clone();
    let mut total = 0;
    for f in &grupo.funcoes {
        let contrato = ContratoQuadrosCompartilhadosCfg {
            grupo: grupo.clone(),
            simbolo: f.symbol.clone(),
            assinatura: f.params.iter().map(|(v, _, t)| (*v, *t)).collect(),
            retorno: f.return_ty,
            repasses: repasses(f, &grupo),
        };
        contrato.conferir(f)?;
        let p = novos
            .get_mut(&f.symbol)
            .ok_or("quadros compartilhados: plano ausente")?;
        for v in contrato.pares().iter().flat_map(|(q, i)| [q, i]) {
            if p.classes.get(v).is_some_and(|c| *c != Ownership::Trivial) {
                return Err(format!(
                    "quadros compartilhados: classe conflitante em {} v{}",
                    f.symbol, v.0
                ));
            }
            if p.classes.insert(*v, Ownership::Trivial).is_none() {
                total += 1;
            }
        }
        p.tabelas.quadros_compartilhados = Some(contrato.clone());
        p.tokens.quadros_compartilhados = Some(contrato);
    }
    *planos = novos;
    Ok(total)
}

#[cfg(test)]
mod testes {
    use super::*;
    fn preparar() -> (
        Module,
        RascunhoSlotsRefArc,
        HashMap<String, PlanoFuncaoDart>,
    ) {
        let m = super::super::slots_vidas::testes::origem();
        let r = baixar_slots_ref_compartilhados(&m).unwrap();
        let p = r
            .funcoes
            .iter()
            .map(|f| (f.symbol.clone(), PlanoFuncaoDart::default()))
            .collect();
        (m, r, p)
    }

    #[test]
    fn papeis_nativos_sao_produzidos_sem_fatos_de_escalares_dart() {
        let (m, r, mut ps) = preparar();
        assert_eq!(produzir_planos_slots_ref_cfg(&m, &r, &mut ps).unwrap(), 8);
        let anteriores = ps.clone();
        assert_eq!(produzir_planos_slots_ref_cfg(&m, &r, &mut ps).unwrap(), 0);
        assert_eq!(ps, anteriores);
        for f in &r.funcoes {
            let p = &ps[&f.symbol];
            super::super::quadros::verificar_com_plano(f, &p.tokens).unwrap();
            assert!(p.tokens.quadros_compartilhados.is_some());
            assert_eq!(
                p.tokens.quadros_compartilhados,
                p.tabelas.quadros_compartilhados
            );
        }
        assert!(m.parametros_escalares_dart.is_empty());
    }

    #[test]
    fn conflitos_e_fatos_obsoletos_nao_publicam_parte_do_grupo() {
        for caso in 0..3 {
            let (m, mut r, mut ps) = preparar();
            let s = r.simbolos["a"].clone();
            match caso {
                0 => {
                    ps.get_mut(&s)
                        .unwrap()
                        .classes
                        .insert(ValueId(0), Ownership::Owned);
                }
                1 => {
                    ps.remove(&s);
                }
                _ => {
                    r.locais.get_mut("raiz").unwrap().1 = 1;
                }
            }
            let antes = ps.clone();
            assert!(produzir_planos_slots_ref_cfg(&m, &r, &mut ps).is_err());
            assert_eq!(ps, antes, "caso {caso}");
        }
    }

    #[test]
    fn contrato_admite_operacoes_ref_mas_nao_muda_ports_nativas() {
        let (m, r, mut ps) = preparar();
        produzir_planos_slots_ref_cfg(&m, &r, &mut ps).unwrap();
        let mut a = r.funcoes[1].clone();
        let c = ps[&a.symbol]
            .tokens
            .quadros_compartilhados
            .as_ref()
            .unwrap();
        a.blocks[0].instructions.insert(
            1,
            (
                ValueId(100),
                Instruction::ArcCopy {
                    value: Operand::Val(ValueId(2)),
                },
                Type::Ref,
            ),
        );
        a.blocks[0].instructions.push((
            ValueId(101),
            Instruction::ArcDrop {
                value: Operand::Val(ValueId(100)),
            },
            Type::Void,
        ));
        c.conferir(&a).unwrap();
        let Instruction::CallStatic { args, .. } = &mut a.blocks[0]
            .instructions
            .iter_mut()
            .find(|(_, i, _)| matches!(i, Instruction::CallStatic { .. }))
            .unwrap()
            .1
        else {
            unreachable!()
        };
        args[1] = args[3].clone();
        assert!(c.conferir(&a).unwrap_err().contains("obsoletos"));
    }

    #[test]
    fn producao_de_tokens_confere_quadros_emprestados_e_recusa_marcadores_divergentes() {
        let (m, r, mut ps) = preparar();
        produzir_planos_slots_ref_cfg(&m, &r, &mut ps).unwrap();
        let f = &r.funcoes[1];
        let p = ps.get_mut(&f.symbol).unwrap();
        // A instrução ordinária CallStatic precisa do produtor de chamadas
        // do conjunto. Aqui conferimos a folha que só lê/copía o slot.
        let mut folha = f.clone();
        folha.blocks[0]
            .instructions
            .retain(|(_, i, _)| !matches!(i, Instruction::CallStatic { .. }));
        assert!(
            produzir_e_verificar_tokens_dart(
                &folha,
                &mut p.classes,
                &mut p.tokens,
                &p.tabelas,
                &p.escopos
            )
            .is_err()
        );
        assert!(
            verificar_tokens(f, &p.classes, &TabelasDaFuncao::default(), &p.tokens)
                .unwrap_err()
                .contains("divergem")
        );
    }

    #[test]
    fn folha_produz_tokens_ref_com_pares_nativos_certificados() {
        let mut m = super::super::slots_vidas::testes::origem();
        m.functions.pop();
        m.functions[1].blocks[0]
            .instructions
            .retain(|(_, i, _)| !matches!(i, Instruction::CallStatic { .. }));
        let r = baixar_slots_ref_compartilhados(&m).unwrap();
        let mut ps = r
            .funcoes
            .iter()
            .map(|f| (f.symbol.clone(), PlanoFuncaoDart::default()))
            .collect();
        produzir_planos_slots_ref_cfg(&m, &r, &mut ps).unwrap();
        let mut folha = r.funcoes[1].clone();
        folha.blocks[0].instructions.push((
            ValueId(100),
            Instruction::ArcDrop {
                value: Operand::Val(ValueId(2)),
            },
            Type::Void,
        ));
        let p = ps.get_mut(&folha.symbol).unwrap();
        produzir_e_verificar_tokens_dart(
            &folha,
            &mut p.classes,
            &mut p.tokens,
            &p.tabelas,
            &p.escopos,
        )
        .unwrap();
        assert_eq!(p.classes[&ValueId(2)], Ownership::Owned);
        assert!(p.tabelas.cleanup_estrangeiro.is_none());
    }

    #[test]
    fn conjunto_prepara_corpos_com_slots_fortes_sem_reabrir_assinaturas() {
        let mut m = super::super::slots_vidas::testes::origem();
        m.functions.pop();
        m.functions[1].blocks[0]
            .instructions
            .retain(|(_, i, _)| !matches!(i, Instruction::CallStatic { .. }));
        let r = baixar_slots_ref_compartilhados(&m).unwrap();
        let mut ps = r
            .funcoes
            .iter()
            .map(|f| (f.symbol.clone(), PlanoFuncaoDart::default()))
            .collect();
        produzir_planos_slots_ref_cfg(&m, &r, &mut ps).unwrap();
        let mut fs = r.funcoes.clone();
        preparar_arc_funcoes_dart(&mut fs, &mut ps).unwrap();
        for f in &fs {
            let p = &ps[&f.symbol];
            verificar_contrato_funcao_dart(f, &p.classes, &p.tokens, &p.tabelas, &p.escopos)
                .unwrap();
        }
    }

    #[test]
    fn retoma_e_cleanup_preservam_quadro_do_chamador() {
        for retoma in [false, true] {
            let (m, r, mut ps) = preparar();
            produzir_planos_slots_ref_cfg(&m, &r, &mut ps).unwrap();
            let mut a = r.funcoes[1].clone();
            a.blocks[0].terminator = Terminator::CondBranch {
                cond: Operand::Constant(Constant::Bool(false)),
                then_block: BlockId(1),
                else_block: BlockId(2),
            };
            for id in [1, 2] {
                a.blocks.push(BasicBlock {
                    id: BlockId(id),
                    instructions: vec![(
                        ValueId(100 + id),
                        Instruction::ArcDrop {
                            value: Operand::Val(ValueId(2)),
                        },
                        Type::Void,
                    )],
                    terminator: Terminator::Return(None),
                });
            }
            let p = ps.get_mut(&a.symbol).unwrap();
            p.classes.insert(ValueId(4), Ownership::Trivial);
            p.tokens.instrucoes.insert(
                ValueId(4),
                EfeitoTokens {
                    pode_falhar: true,
                    ..Default::default()
                },
            );
            p.tabelas.invocacoes.insert(ValueId(4), BlockId(1));
            p.tabelas.pousos.insert(BlockId(1));
            if retoma {
                p.tabelas.saidas.insert(BlockId(1), SaidaPorExcecao::Retoma);
            }
            produzir_e_verificar_tokens_dart(
                &a,
                &mut p.classes,
                &mut p.tokens,
                &p.tabelas,
                &p.escopos,
            )
            .unwrap();
            super::super::conferir_retomas(&a, &p.tabelas).unwrap();
            let quadros = super::super::quadros::na_entrada_dos_pousos(&a, &p.tabelas).unwrap();
            assert!(
                quadros[&BlockId(1)].is_empty(),
                "empréstimo virou owner de cleanup"
            );
            p.owners_no_pouso =
                tokens_na_entrada_dos_pousos(&a, &p.classes, &p.tabelas, &p.tokens).unwrap();
            let certificado = CleanupEstrangeiro::novo(&a, p).unwrap();
            if retoma {
                assert!(certificado.is_none());
            } else {
                let c = certificado.unwrap();
                c.conferir(&a, &p.tabelas).unwrap();
                assert!(c.quadros(BlockId(1)).is_empty());
            }
        }
    }
}
