//! Grafo local de sítios/campos e SCCs de objetos (§29.1).
//! Ausência de ciclo conhecido não é certificado durante toda a vida do objeto.

use super::{hir::AnaliseHir, modelo::NoAbstrato};
use crate::hir::{Function, Module, Type};
use std::collections::{BTreeMap, BTreeSet};

/// Fotografia imutável das arestas locais inferidas, incluindo fronteiras opacas.
/// SCC representa possibilidade abstrata, não grupo de instâncias concreto.
/// Não seleciona política nem certifica NaoParticipa/AciclicoProvado: mutações
/// futuras, entradas externas e observadores exigem a cobertura global da §29.
///
/// ```
/// use dartforge_emit_native::{hir::Module, otimizar::arc::analise::grafo::*};
/// assert!(construir_local_no_modulo(&Module::new(), "f", 8)?.is_none());
/// # Ok::<(), String>(())
/// ```
#[derive(Debug)]
pub struct GrafoHeapArc {
    arestas: BTreeMap<NoAbstrato, BTreeSet<NoAbstrato>>,
    componentes: Vec<(Vec<NoAbstrato>, bool)>,
    opaco: bool,
}

impl GrafoHeapArc {
    /// Consulta arestas conhecidas sem apagar as fronteiras opacas.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, otimizar::arc::analise::grafo::*};
    /// let mut m = Module::new(); m.memoria_arc = true;
    /// m.functions.push(Function { symbol: "f".into(), name: "f".into(), depuracao: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] });
    /// assert!(construir_local_no_modulo(&m, "f", 8)?.unwrap().arestas().is_empty());
    /// # Ok::<(), String>(())
    /// ```
    pub fn arestas(&self) -> &BTreeMap<NoAbstrato, BTreeSet<NoAbstrato>> {
        &self.arestas
    }

    /// Consulta se faltam arestas/esquemas ou efeitos da fotografia local.
    /// Falso não prova cobertura durante toda a vida de nenhuma instância.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, otimizar::arc::analise::grafo::*};
    /// let mut m = Module::new(); m.memoria_arc = true;
    /// m.functions.push(Function { symbol: "f".into(), name: "f".into(), depuracao: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] });
    /// assert!(!construir_local_no_modulo(&m, "f", 8)?.unwrap().opaco());
    /// # Ok::<(), String>(())
    /// ```
    pub fn opaco(&self) -> bool {
        self.opaco
    }

    /// Enumera SCCs e a presença de ciclo abstrato nas arestas conhecidas.
    /// Componente unitário é cíclico somente quando contém autoaresta.
    /// Um falso não exclui ciclos por fronteiras opacas/mutações futuras.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, otimizar::arc::analise::grafo::*};
    /// let mut m = Module::new(); m.memoria_arc = true;
    /// m.functions.push(Function { symbol: "f".into(), name: "f".into(), depuracao: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] });
    /// assert_eq!(construir_local_no_modulo(&m, "f", 8)?.unwrap().componentes().count(), 0);
    /// # Ok::<(), String>(())
    /// ```
    pub fn componentes(&self) -> impl Iterator<Item = (&[NoAbstrato], bool)> {
        self.componentes
            .iter()
            .map(|(nos, ciclo)| (nos.as_slice(), *ciclo))
    }
}

/// Analisa o corpo e extrai SCCs de arestas de campos, sem usar SCCs de chamadas.
/// Só no módulo ARC; tracing devolve None. O resultado é local à versão atual,
/// não certificado de política, grupo concreto, vida ou cobertura do programa.
///
/// # Erros
/// Corpo/SSA ou premissas de sítio/layout/resumos inválidos na análise do módulo.
///
/// ```
/// use dartforge_emit_native::{hir::Module, otimizar::arc::analise::grafo::*};
/// assert!(construir_local_no_modulo(&Module::new(), "ausente", 8)?.is_none());
/// # Ok::<(), String>(())
/// ```
pub fn construir_local_no_modulo(
    m: &Module,
    simbolo: &str,
    limite: usize,
) -> Result<Option<GrafoHeapArc>, String> {
    Ok(super::modulo::analisar_no_modulo(m, simbolo, limite)?
        .as_ref()
        .map(|a| {
            construir(
                m.functions
                    .iter()
                    .find(|f| f.symbol == simbolo)
                    .expect("corpo analisado"),
                a,
            )
        }))
}

fn construir(f: &Function, a: &AnaliseHir) -> GrafoHeapArc {
    let mut arestas: BTreeMap<_, BTreeSet<_>> = a
        .esquemas
        .keys()
        .cloned()
        .map(|no| (no, BTreeSet::new()))
        .collect();
    let mut opaco = !a.desconhecidos().is_empty();
    // Valores que podem ser handles/endereços sem origem fechada conservam
    // uma fronteira, mesmo quando nenhuma aresta desse valor foi materializada.
    for (v, t) in f.params.iter().map(|(v, _, t)| (v, t)).chain(
        f.blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .map(|(v, _, t)| (v, t)),
    ) {
        if matches!(t, Type::Ref | Type::I64 | Type::Ptr)
            && a.valor(*v).is_none_or(|p| p.nos().is_none())
        {
            opaco = true;
        }
    }
    for (no, campos) in &a.esquemas {
        for campo in campos {
            let Some(destinos) = a.pontos().campo(no, campo).and_then(|p| p.nos()) else {
                opaco = true;
                continue;
            };
            for destino in destinos {
                arestas
                    .entry(no.clone())
                    .or_default()
                    .insert(destino.clone());
                arestas.entry(destino.clone()).or_default();
                if !a.esquemas.contains_key(destino) {
                    opaco = true;
                }
            }
        }
    }
    let nos: Vec<_> = arestas.keys().cloned().collect();
    let ids: BTreeMap<_, _> = nos.iter().enumerate().map(|(i, no)| (no, i)).collect();
    let adjacentes: Vec<Vec<_>> = nos
        .iter()
        .map(|no| arestas[no].iter().map(|destino| ids[destino]).collect())
        .collect();
    let componentes = crate::otimizar::scc::componentes(&adjacentes)
        .into_iter()
        .map(|c| {
            let ciclo = c.len() > 1 || adjacentes[c[0]].contains(&c[0]);
            let mut membros: Vec<_> = c.into_iter().map(|v| nos[v].clone()).collect();
            membros.sort();
            (membros, ciclo)
        })
        .collect();
    GrafoHeapArc {
        arestas,
        componentes,
        opaco,
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::hir::*;

    fn modulo(mascara: usize) -> Module {
        let mut m = Module::new();
        m.memoria_arc = true;
        m.layouts_campos_arc.insert(1, vec![Type::Ref]);
        let mut instructions: Vec<_> = (0..3)
            .map(|v| {
                (
                    ValueId(v),
                    Instruction::AllocObject {
                        class_id: 1,
                        fields: vec![Operand::Constant(Constant::Null)],
                    },
                    Type::Ref,
                )
            })
            .collect();
        for v in 0..3 {
            for w in 0..3 {
                if mascara & (1 << (3 * v + w)) != 0 {
                    instructions.push((
                        ValueId(instructions.len() as u32),
                        Instruction::SetField {
                            object: Operand::Val(ValueId(v)),
                            index: 0,
                            value: Operand::Val(ValueId(w)),
                        },
                        Type::Void,
                    ));
                }
            }
        }
        m.functions.push(Function {
            symbol: "f".into(),
            name: "f".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions,
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        });
        super::super::origens::registrar(&mut m, 0, true);
        m
    }

    #[test]
    fn scc_de_campos_confere_com_floyd_nos_512_grafos_hir() {
        for mascara in 0..512 {
            let m = modulo(mascara);
            let g = construir_local_no_modulo(&m, "f", 8).unwrap().unwrap();
            assert!(!g.opaco());
            assert_eq!(g.arestas().len(), 3);
            let nos: Vec<_> = (0..3)
                .map(|v| NoAbstrato::Alocacao {
                    sitio: m.sitios_arc["f"][&ValueId(v)].clone(),
                    contexto: "".into(),
                })
                .collect();
            let mut alcance = [[false; 3]; 3];
            for (v, linha) in alcance.iter_mut().enumerate() {
                for (w, destino) in linha.iter_mut().enumerate() {
                    *destino = mascara & (1 << (3 * v + w)) != 0;
                    assert_eq!(g.arestas()[&nos[v]].contains(&nos[w]), *destino);
                }
            }
            // Fecho sem identidade: diagonal só verdadeira quando há ciclo.
            for k in 0..3 {
                for v in 0..3 {
                    for w in 0..3 {
                        alcance[v][w] |= alcance[v][k] && alcance[k][w];
                    }
                }
            }
            let mut donos = [usize::MAX; 3];
            for (id, (membros, ciclo)) in g.componentes().enumerate() {
                for no in membros {
                    let v = nos.iter().position(|n| n == no).unwrap();
                    assert_eq!(donos[v], usize::MAX);
                    donos[v] = id;
                    assert_eq!(ciclo, alcance[v][v], "grafo {mascara}, nó {v}");
                }
            }
            for v in 0..3 {
                for w in 0..3 {
                    assert_eq!(
                        donos[v] == donos[w],
                        v == w || (alcance[v][w] && alcance[w][v])
                    );
                }
            }
        }
    }

    #[test]
    fn fronteiras_opacas_e_budget_nao_se_tornam_grafo_fechado() {
        let mut m = modulo(511);
        assert!(
            construir_local_no_modulo(&m, "f", 1)
                .unwrap()
                .unwrap()
                .opaco()
        );
        assert!(
            construir_local_no_modulo(&m, "f", 0)
                .unwrap()
                .unwrap()
                .opaco()
        );
        m.layouts_campos_arc.clear();
        let g = construir_local_no_modulo(&m, "f", 8).unwrap().unwrap();
        assert!(g.opaco());
        assert!(g.arestas().is_empty());
        m.memoria_arc = false;
        assert!(construir_local_no_modulo(&m, "f", 8).unwrap().is_none());
    }

    #[test]
    fn instancias_resumidas_no_mesmo_sitio_nao_provam_singleton_ou_ausencia_de_ciclo() {
        let mut m = modulo(1 << 1); // Instância 0 aponta para instância 1.
        let origem = m.sitios_arc["f"][&ValueId(0)].clone();
        m.sitios_arc
            .get_mut("f")
            .unwrap()
            .insert(ValueId(1), origem);
        let g = construir_local_no_modulo(&m, "f", 8).unwrap().unwrap();
        assert_eq!(g.arestas().len(), 2);
        assert!(g.componentes().any(|(_, ciclo)| ciclo));
        assert!(!g.opaco());
    }

    #[test]
    fn parametro_opaco_nao_vira_grafo_vazio_fechado() {
        let mut m = Module::new();
        m.memoria_arc = true;
        m.functions.push(Function {
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
        });
        let g = construir_local_no_modulo(&m, "id", 8).unwrap().unwrap();
        assert!(g.opaco());
        assert!(g.arestas().is_empty());
    }

    #[test]
    fn recursao_de_chamadas_nao_fabrica_autoaresta_de_objeto() {
        let mut m = modulo(0);
        m.functions[0].blocks[0].instructions.push((
            ValueId(3),
            Instruction::CallStatic {
                symbol: "f".into(),
                args: vec![],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        ));
        let g = construir_local_no_modulo(&m, "f", 8).unwrap().unwrap();
        assert!(g.opaco());
        assert!(g.componentes().all(|(_, ciclo)| !ciclo));
        assert_eq!(g.arestas().len(), 3);
    }
}
