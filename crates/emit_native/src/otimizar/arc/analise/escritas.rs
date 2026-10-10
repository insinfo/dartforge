//! Inclusões de arestas fortes produzidas por stores parametrizados (§28.3).
//! Não são cobertura de vida/publicação: campos opacos e caminhos implícitos
//! continuam exigindo ARC geral mesmo quando estas inclusões são conhecidas.

use super::{modelo::*, points_to::*, resumos::*};
use crate::hir::*;
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct EscritaParametro {
    pub objeto: usize,
    pub indice: usize,
    pub valor: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum InclusoesEscritas {
    Conhecidas(BTreeSet<EscritaParametro>),
    Desconhecidas,
}

impl InclusoesEscritas {
    pub(super) fn vazio() -> Self {
        Self::Conhecidas(BTreeSet::new())
    }

    // Topo é absorvente; orçamento vale para o resumo inteiro, não por store.
    pub(super) fn unir(&mut self, outro: &Self, limite: usize) -> bool {
        let antes = self.clone();
        match (&mut *self, outro) {
            (Self::Conhecidas(a), Self::Conhecidas(b)) => {
                a.extend(b.iter().cloned());
                if a.len() > limite {
                    *self = Self::Desconhecidas;
                }
            }
            (_, Self::Desconhecidas) => *self = Self::Desconhecidas,
            _ => {}
        }
        *self != antes
    }
}

// Aliases reais dos argumentos substituem placeholders; nenhuma chamada
// cria um objeto fresco. Campos lidos/alocações/externos não são parâmetros
// por aproximação: sem uma representação de caminho, conservam topo.
pub(super) fn extrair<'a>(
    f: &Function,
    limite: usize,
    chamada: impl Fn(&str) -> Option<(&'a Function, &'a RetornoHeapArc, &'a InclusoesEscritas)>,
) -> Result<InclusoesEscritas, String> {
    super::super::ssa::verificar(f)?;
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
    let mut valores: Vec<_> = tipos.keys().copied().collect();
    valores.sort_by_key(|v| v.0);
    let ids: HashMap<_, _> = valores.iter().enumerate().map(|(i, v)| (*v, i)).collect();
    let nulo = ids.len();
    let opaco = nulo + 1;
    let operando = |o: &Operand| match o {
        Operand::Val(v) => ids[v],
        Operand::Constant(Constant::Null) => nulo,
        _ => opaco,
    };
    let mut rs = vec![Restricao::Desconhecer { destino: opaco }];
    for (p, (v, _, t)) in f.params.iter().enumerate() {
        rs.push(if *t == Type::Ref {
            Restricao::Semear {
                destino: ids[v],
                no: NoAbstrato::Parametro {
                    funcao: f.symbol.clone(),
                    indice: u32::try_from(p).map_err(|_| "escritas ARC: parâmetros demais")?,
                },
            }
        } else {
            Restricao::Desconhecer { destino: ids[v] }
        });
    }
    for (v, i, t) in f.blocks.iter().flat_map(|b| &b.instructions) {
        let destino = ids[v];
        match i {
            Instruction::Const(Constant::Null) if *t == Type::Ref => {}
            Instruction::ArcCopy { value } | Instruction::ArcMove { value }
                if *t == Type::Ref && argumento_exato(value, Type::Ref, &tipos) =>
            {
                rs.push(Restricao::Copiar {
                    origem: operando(value),
                    destino,
                })
            }
            Instruction::Bitcast { op, to: Type::Ref }
                if *t == Type::Ref && argumento_exato(op, Type::Ref, &tipos) =>
            {
                rs.push(Restricao::Copiar {
                    origem: operando(op),
                    destino,
                })
            }
            Instruction::Phi {
                incoming,
                ty: Type::Ref,
            } if *t == Type::Ref
                && incoming
                    .iter()
                    .all(|(_, o)| argumento_exato(o, Type::Ref, &tipos)) =>
            {
                rs.extend(incoming.iter().map(|(_, o)| Restricao::Copiar {
                    origem: operando(o),
                    destino,
                }));
            }
            Instruction::CallStatic {
                symbol,
                args,
                ret_ty,
            } => {
                let Some((callee, retorno, _)) = chamada(symbol) else {
                    return Ok(InclusoesEscritas::Desconhecidas);
                };
                if *t != *ret_ty
                    || *ret_ty != callee.return_ty
                    || args.len() != callee.params.len()
                    || !args
                        .iter()
                        .zip(&callee.params)
                        .all(|(o, (_, _, ty))| argumento_exato(o, *ty, &tipos))
                {
                    return Ok(InclusoesEscritas::Desconhecidas);
                }
                if *t == Type::Ref {
                    if let RetornoHeapArc::Aliases { parametros, .. } = retorno {
                        for &p in parametros {
                            let arg = args
                                .get(p)
                                .ok_or("escritas ARC: parâmetro de retorno inválido")?;
                            rs.push(Restricao::Copiar {
                                origem: operando(arg),
                                destino,
                            });
                        }
                    } else {
                        rs.push(Restricao::Desconhecer { destino });
                    }
                }
            }
            Instruction::SetField { object, value, .. }
                if *t == Type::Void
                    && argumento_exato(object, Type::Ref, &tipos)
                    && argumento_exato(value, Type::Ref, &tipos) => {}
            Instruction::Const(c) if argumento_exato(&Operand::Constant(c.clone()), *t, &tipos) => {
                rs.push(Restricao::Desconhecer { destino });
            }
            i if super::hir::escalar_puro(i, *t, &tipos) => {
                rs.push(Restricao::Desconhecer { destino });
            }
            _ => return Ok(InclusoesEscritas::Desconhecidas),
        }
    }
    let pontos = resolver(ids.len() + 2, limite, &rs)?;
    let mut resultado = InclusoesEscritas::vazio();
    let mut incluir = |object: &Operand, indice, value: &Operand| -> bool {
        let (Some(origens), Some(destinos)) = (
            pontos.variavel(operando(object)).unwrap().nos(),
            pontos.variavel(operando(value)).unwrap().nos(),
        ) else {
            return false;
        };
        for origem in origens {
            for destino in destinos {
                let (
                    NoAbstrato::Parametro { indice: a, .. },
                    NoAbstrato::Parametro { indice: b, .. },
                ) = (origem, destino)
                else {
                    return false;
                };
                resultado.unir(
                    &InclusoesEscritas::Conhecidas(BTreeSet::from([EscritaParametro {
                        objeto: *a as usize,
                        indice,
                        valor: *b as usize,
                    }])),
                    limite,
                );
            }
        }
        true
    };
    for (_, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
        match i {
            Instruction::SetField {
                object,
                index,
                value,
            } => {
                if !incluir(object, *index, value) {
                    return Ok(InclusoesEscritas::Desconhecidas);
                }
            }
            Instruction::CallStatic { symbol, args, .. } => {
                let (_, _, escritas) = chamada(symbol).unwrap();
                let InclusoesEscritas::Conhecidas(escritas) = escritas else {
                    return Ok(InclusoesEscritas::Desconhecidas);
                };
                for escrita in escritas {
                    let object = args
                        .get(escrita.objeto)
                        .ok_or("escritas ARC: receiver inválido")?;
                    let value = args
                        .get(escrita.valor)
                        .ok_or("escritas ARC: valor inválido")?;
                    if !incluir(object, escrita.indice, value) {
                        return Ok(InclusoesEscritas::Desconhecidas);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(resultado)
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::collections::HashSet;

    fn ligar(symbol: &str, invertido: bool) -> Function {
        Function {
            symbol: symbol.into(),
            name: symbol.into(),
            depuracao: None,
            params: vec![
                (ValueId(0), "a".into(), Type::Ref),
                (ValueId(1), "b".into(), Type::Ref),
            ],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(2),
                    Instruction::SetField {
                        object: Operand::Val(ValueId(u32::from(invertido))),
                        index: 0,
                        value: Operand::Val(ValueId(u32::from(!invertido))),
                    },
                    Type::Void,
                )],
                terminator: Terminator::Return(None),
            }],
        }
    }

    fn esperadas(pares: &[(usize, usize)]) -> InclusoesEscritas {
        InclusoesEscritas::Conhecidas(
            pares
                .iter()
                .map(|&(objeto, valor)| EscritaParametro {
                    objeto,
                    indice: 0,
                    valor,
                })
                .collect(),
        )
    }

    #[test]
    fn escrita_parametrica_nao_promove_cobertura_e_null_nao_cria_aresta() {
        let mut f = ligar("ligar", false);
        assert_eq!(extrair(&f, 8, |_| None).unwrap(), esperadas(&[(0, 1)]));
        let r = super::super::resumos::extrair(&f, 8).unwrap();
        assert_eq!(r.campos(), CamposHeapArc::Desconhecidos);
        assert_eq!(r.publicacoes(), PublicacoesHeapArc::Desconhecidas);
        let Instruction::SetField { value, .. } = &mut f.blocks[0].instructions[0].1 else {
            unreachable!()
        };
        *value = Operand::Constant(Constant::Null);
        assert_eq!(extrair(&f, 8, |_| None).unwrap(), esperadas(&[]));
        assert_eq!(
            super::super::resumos::extrair(&f, 8).unwrap().campos(),
            CamposHeapArc::Desconhecidos
        );
        f.blocks[0].instructions.push((
            ValueId(3),
            Instruction::CallRuntime {
                name: "externo".into(),
                args: vec![],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
        assert_eq!(
            extrair(&f, 8, |_| None).unwrap(),
            InclusoesEscritas::Desconhecidas
        );
    }

    #[test]
    fn aliases_por_copy_e_retorno_local_instanciam_os_argumentos() {
        let mut f = ligar("ligar", false);
        f.blocks[0].instructions.insert(
            0,
            (
                ValueId(3),
                Instruction::ArcCopy {
                    value: Operand::Val(ValueId(1)),
                },
                Type::Ref,
            ),
        );
        f.blocks[0].instructions.insert(
            1,
            (
                ValueId(4),
                Instruction::CallStatic {
                    symbol: "id".into(),
                    args: vec![Operand::Val(ValueId(3))],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ),
        );
        let Instruction::SetField { value, .. } = &mut f.blocks[0].instructions[2].1 else {
            unreachable!()
        };
        *value = Operand::Val(ValueId(4));
        let id = Function {
            symbol: "id".into(),
            name: "id".into(),
            depuracao: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        };
        let mut m = Module::new();
        m.memoria_arc = true;
        m.functions = vec![f, id];
        let roots = HashSet::from(["ligar"]);
        let r = super::super::chamadas::resolver(&m, &roots, 8).unwrap();
        assert_eq!(r["ligar"].1.inclusoes_escritas, esperadas(&[(0, 1)]));
        // Representação igual em bits não autoriza converter um argumento
        // I64 em Ref para sustentar o resumo de stores.
        m.functions[0].params[1].2 = Type::I64;
        assert!(super::super::chamadas::resolver(&m, &roots, 8).is_err());
        m.functions[0].params[1].2 = Type::Ref;
        let Instruction::CallStatic { args, .. } = &mut m.functions[0].blocks[0].instructions[1].1
        else {
            unreachable!()
        };
        args[0] = Operand::Constant(Constant::Int(7));
        let r = super::super::chamadas::resolver(&m, &roots, 8).unwrap();
        assert_eq!(
            r["ligar"].1.inclusoes_escritas,
            InclusoesEscritas::Desconhecidas
        );
    }

    #[test]
    fn phi_conserva_produto_de_aliases_e_budget_do_resumo_inteiro() {
        let mut f = ligar("phi", false);
        f.params.push((ValueId(2), "cond".into(), Type::I1));
        f.blocks = vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::CondBranch {
                    cond: Operand::Val(ValueId(2)),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                },
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(3)),
            },
            BasicBlock {
                id: BlockId(3),
                instructions: vec![
                    (
                        ValueId(3),
                        Instruction::Phi {
                            incoming: vec![
                                (BlockId(1), Operand::Val(ValueId(0))),
                                (BlockId(2), Operand::Val(ValueId(1))),
                            ],
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(4),
                        Instruction::ArcCopy {
                            value: Operand::Val(ValueId(3)),
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(5),
                        Instruction::SetField {
                            object: Operand::Val(ValueId(3)),
                            index: 0,
                            value: Operand::Val(ValueId(4)),
                        },
                        Type::Void,
                    ),
                ],
                terminator: Terminator::Return(None),
            },
        ];
        assert_eq!(
            extrair(&f, 4, |_| None).unwrap(),
            esperadas(&[(0, 0), (0, 1), (1, 0), (1, 1)])
        );
        assert_eq!(
            extrair(&f, 3, |_| None).unwrap(),
            InclusoesEscritas::Desconhecidas
        );
        // Um caminho null não se torna uma alocação ou argumento fresco.
        let Instruction::Phi { incoming, .. } = &mut f.blocks[3].instructions[0].1 else {
            unreachable!()
        };
        incoming[1].1 = Operand::Constant(Constant::Null);
        assert_eq!(extrair(&f, 4, |_| None).unwrap(), esperadas(&[(0, 0)]));
    }

    #[test]
    fn cadeia_de_mil_escritores_compõe_sem_depender_da_ordem_dos_corpos() {
        let mut m = Module::new();
        m.memoria_arc = true;
        for n in 0..1000 {
            let mut f = ligar(&format!("s{n:04}"), false);
            if n != 999 {
                f.blocks[0].instructions[0].1 = Instruction::CallStatic {
                    symbol: format!("s{:04}", n + 1),
                    args: vec![Operand::Val(ValueId(1)), Operand::Val(ValueId(0))],
                    ret_ty: Type::Void,
                };
            }
            m.functions.push(f);
        }
        for ordem in [false, true] {
            if ordem {
                m.functions.reverse();
            }
            let r = super::super::chamadas::resolver(&m, &HashSet::from(["s0000"]), 8).unwrap();
            assert_eq!(r.len(), 1000);
            for n in 0..1000 {
                let esperado = if (999 - n) % 2 == 0 { (0, 1) } else { (1, 0) };
                assert_eq!(
                    r[&format!("s{n:04}")].1.inclusoes_escritas,
                    esperadas(&[esperado])
                );
            }
        }
    }

    #[test]
    fn scc_de_escritores_compõe_permutacao_e_eleva_budget_a_topo() {
        let mut f = ligar("f", false);
        f.blocks[0].instructions.push((
            ValueId(3),
            Instruction::CallStatic {
                symbol: "g".into(),
                args: vec![Operand::Val(ValueId(1)), Operand::Val(ValueId(0))],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
        let mut g = ligar("g", false);
        g.blocks[0].instructions[0].1 = Instruction::CallStatic {
            symbol: "f".into(),
            args: vec![Operand::Val(ValueId(0)), Operand::Val(ValueId(1))],
            ret_ty: Type::Void,
        };
        let mut m = Module::new();
        m.memoria_arc = true;
        m.functions = vec![f, g];
        for ordem in [false, true] {
            if ordem {
                m.functions.reverse();
            }
            let r = super::super::chamadas::resolver(&m, &HashSet::from(["f"]), 8).unwrap();
            for (_, resumo) in r.values() {
                assert_eq!(resumo.inclusoes_escritas, esperadas(&[(0, 1), (1, 0)]));
                assert_eq!(resumo.campos(), CamposHeapArc::Desconhecidos);
                assert_eq!(resumo.publicacoes(), PublicacoesHeapArc::Desconhecidas);
            }
            let r = super::super::chamadas::resolver(&m, &HashSet::from(["f"]), 1).unwrap();
            assert!(
                r.values()
                    .all(|(_, r)| r.inclusoes_escritas == InclusoesEscritas::Desconhecidas)
            );
        }
        let f = m.functions.iter_mut().find(|f| f.symbol == "f").unwrap();
        f.blocks[0].instructions.push((
            ValueId(9),
            Instruction::CallStatic {
                symbol: "fora".into(),
                args: vec![],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
        let r = super::super::chamadas::resolver(&m, &HashSet::from(["f"]), 8).unwrap();
        assert!(
            r.values()
                .all(|(_, r)| r.inclusoes_escritas == InclusoesEscritas::Desconhecidas)
        );
    }
}
