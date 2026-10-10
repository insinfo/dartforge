//! Extração conservadora de restrições da HIR após SSA (§28.2).
//! Fatos nominais de alocação/layout são premissas explícitas do produtor.
//! Não exporta certificados, resumos SDK ou política de memória.

use super::{modelo::*, points_to::*};
use crate::hir::*;
use std::collections::{BTreeSet, HashMap};

/// Origem/esquema dos campos de AllocObject ou da fábrica reservada zerada.
/// O produtor deve preservar origem/contexto nos clones e conferir layouts.
/// Índices SSA não são usados como identidade persistente do nó abstrato.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::analise::{hir::OrigemObjeto, modelo::*, points_to::*};
/// let origem = OrigemObjeto {
///     no: NoAbstrato::Alocacao { sitio: SitioArc { funcao: "f".into(), origem: 1, especializacao: "".into() }, contexto: "".into() },
///     campos: vec![CampoArc { layout: "No:v1".into(), posicao: 0 }],
/// };
/// assert_eq!(origem.campos.len(), 1);
/// ```
#[derive(Debug, Clone)]
pub struct OrigemObjeto {
    /// Nó nominal produzido para a alocação, nunca placeholder de parâmetro.
    pub no: NoAbstrato,
    /// Campos nas posições físicas do inicializador, incluindo campos triviais.
    pub campos: Vec<CampoArc>,
}

/// Fatos nominais da mesma versão da função/layout, ainda fornecidos explicitamente.
/// Fatos ausentes causam desconhecimento; fatos obsoletos/incompatíveis são erros.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::analise::hir::FatosHir;
/// assert!(FatosHir::default().objetos.is_empty());
/// ```
#[derive(Debug, Default)]
pub struct FatosHir {
    /// Origem/esquema de cada alocação de objeto identificada pelo produtor.
    pub objetos: HashMap<ValueId, OrigemObjeto>,
    /// Campo nominal de cada GetField/SetField, identificado pelo resultado SSA.
    pub acessos: HashMap<ValueId, CampoArc>,
}

/// Resultado local imutável; IDs desconhecidos identificam operações sem transferência precisa.
/// Não certifica cobertura completa do programa, escape, observadores ou singleton.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::analise::hir::*};
/// let f = Function { symbol: "f".into(), name: "f".into(), depuracao: None,
/// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
/// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
/// assert!(analisar(&f, &FatosHir::default(), 8)?.desconhecidos().is_empty());
/// # Ok::<(), String>(())
/// ```
#[derive(Debug)]
pub struct AnaliseHir {
    /// Solução do sistema extraído, com desconhecimento explícito.
    pontos: ResultadoPointsTo,
    /// Operações sem contrato de heap preciso; não confundir com ausência de efeito.
    desconhecidos: Vec<ValueId>,
    ids: HashMap<ValueId, usize>,
    pub(crate) corpo: blake3::Hash,
    pub(crate) dependencias_corpos: HashMap<String, blake3::Hash>,
    pub(crate) dependencias_tabelas: HashMap<String, blake3::Hash>,
    pub(crate) premissas_modulo: Option<blake3::Hash>,
    pub(crate) limite: usize,
    pub(crate) esquemas: HashMap<NoAbstrato, BTreeSet<CampoArc>>,
}

impl AnaliseHir {
    /// Consulta a solução sem permitir substituir ou refinar conjuntos.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, otimizar::arc::analise::hir::*};
    /// let f = Function { symbol: "f".into(), name: "f".into(), depuracao: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
    /// let a = analisar(&f, &FatosHir::default(), 8)?;
    /// assert!(a.pontos().variavel(0).unwrap().nos().unwrap().is_empty());
    /// # Ok::<(), String>(())
    /// ```
    pub fn pontos(&self) -> &ResultadoPointsTo {
        &self.pontos
    }

    /// Consulta operações opacas sem permitir apagar evidência de efeitos.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, otimizar::arc::analise::hir::*};
    /// let f = Function { symbol: "f".into(), name: "f".into(), depuracao: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
    /// assert!(analisar(&f, &FatosHir::default(), 8)?.desconhecidos().is_empty());
    /// # Ok::<(), String>(())
    /// ```
    ///
    /// ```compile_fail
    /// use dartforge_emit_native::{hir::*, otimizar::arc::analise::hir::*};
    /// let f = Function { symbol: "f".into(), name: "f".into(), depuracao: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
    /// let mut a = analisar(&f, &FatosHir::default(), 8).unwrap();
    /// a.desconhecidos().clear();
    /// ```
    pub fn desconhecidos(&self) -> &[ValueId] {
        &self.desconhecidos
    }

    /// Consulta aliases de um valor SSA, ou None se o ID não existe.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, otimizar::arc::analise::hir::*};
    /// let f = Function { symbol: "f".into(), name: "f".into(), depuracao: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
    /// assert!(analisar(&f, &FatosHir::default(), 8)?.valor(ValueId(0)).is_none());
    /// # Ok::<(), String>(())
    /// ```
    pub fn valor(&self, id: ValueId) -> Option<&ConjuntoPontos> {
        self.ids.get(&id).and_then(|&i| self.pontos.variavel(i))
    }
}

/// Extrai e resolve inclusão de aliases da HIR verificada estruturalmente.
/// Parâmetros Ref/I64/Ptr são desconhecidos sem resumo do chamador; largura
/// I64 não prova valor trivial. Null e constantes F64/I1/I8 não geram nós;
/// constantes inteiras usadas como handles/endereços permanecem desconhecidas.
/// Calls/operações não cobertas invalidam todos os campos nominais fornecidos;
/// resultados opacos são topo. Stores conhecidos são sempre inclusivos.
/// Bitcast só conserva aliases se origem e resultado forem Ref.
///
/// # Erros
/// SSA inválida ou fato obsoleto/esquema incompatível com o inicializador/acesso.
/// A função e os fatos não são modificados.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::analise::hir::*};
/// let f = Function { symbol: "id".into(), name: "id".into(), depuracao: None,
/// params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
/// blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
/// terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
/// assert!(analisar(&f, &FatosHir::default(), 8)?.valor(ValueId(0)).unwrap().nos().is_none());
/// # Ok::<(), String>(())
/// ```
pub fn analisar(f: &Function, fatos: &FatosHir, limite: usize) -> Result<AnaliseHir, String> {
    analisar_com_resumos(f, fatos, limite, &HashMap::new())
}

pub(crate) fn analisar_com_resumos(
    f: &Function,
    fatos: &FatosHir,
    limite: usize,
    resumos: &HashMap<String, (&Function, super::resumos::ResumoHeapArc)>,
) -> Result<AnaliseHir, String> {
    for (simbolo, (corpo, resumo)) in resumos {
        if simbolo != &corpo.symbol {
            return Err("points-to: símbolo de resumo incompatível".into());
        }
        resumo.conferir(corpo)?;
    }
    let mut dependencias_corpos = HashMap::new();
    super::super::ssa::verificar(f)?;
    let instrucoes: HashMap<_, _> = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .map(|(v, i, _)| (*v, i))
        .collect();
    let constantes = constantes_inteiras(f);
    for (v, origem) in &fatos.objetos {
        let quantidade = match instrucoes.get(v) {
            Some(Instruction::AllocObject { fields, .. }) => fields.len(),
            Some(i) => fabrica_zerada(i, &constantes)
                .map(|(_, n)| n)
                .ok_or_else(|| format!("points-to HIR: origem obsoleta v{}", v.0))?,
            None => return Err(format!("points-to HIR: origem obsoleta v{}", v.0)),
        };
        if quantidade != origem.campos.len()
            || origem
                .campos
                .iter()
                .enumerate()
                .any(|(i, c)| c.posicao as usize != i)
            || !matches!(
                origem.no,
                NoAbstrato::Alocacao { .. } | NoAbstrato::UnidadeSintetica { .. }
            )
        {
            return Err(format!(
                "points-to HIR: esquema/origem incompatível v{}",
                v.0
            ));
        }
    }
    for (v, campo) in &fatos.acessos {
        let indice = match instrucoes.get(v) {
            Some(Instruction::GetField { index, .. } | Instruction::SetField { index, .. }) => {
                *index
            }
            _ => return Err(format!("points-to HIR: campo obsoleto v{}", v.0)),
        };
        if campo.posicao as usize != indice {
            return Err(format!("points-to HIR: índice incompatível v{}", v.0));
        }
    }
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
    let vazio = ids.len();
    let opaco = vazio + 1;
    let mut rs = vec![Restricao::Desconhecer { destino: opaco }];
    let campos: BTreeSet<_> = fatos
        .acessos
        .values()
        .cloned()
        .chain(fatos.objetos.values().flat_map(|o| o.campos.clone()))
        .collect();
    let operando = |o: &Operand| match o {
        Operand::Val(v) => ids[v],
        Operand::Constant(Constant::Null) => vazio,
        _ => opaco,
    };
    for (v, _, ty) in &f.params {
        if matches!(ty, Type::Ref | Type::I64 | Type::Ptr) {
            rs.push(Restricao::Desconhecer { destino: ids[v] });
        }
    }
    let mut desconhecidos = Vec::new();
    for (v, i, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
        let destino = ids[v];
        match i {
            Instruction::Const(_) if matches!(ty, Type::F64 | Type::I1 | Type::I8) => {}
            Instruction::Const(c) => rs.push(Restricao::Copiar {
                origem: operando(&Operand::Constant(c.clone())),
                destino,
            }),
            Instruction::ArcCopy { value }
            | Instruction::ArcMove { value }
            | Instruction::CheckNotNull(value) => rs.push(Restricao::Copiar {
                origem: operando(value),
                destino,
            }),
            Instruction::Phi { incoming, .. } => {
                for (_, o) in incoming {
                    rs.push(Restricao::Copiar {
                        origem: operando(o),
                        destino,
                    });
                }
            }
            Instruction::Bitcast {
                op: Operand::Val(origem),
                ..
            } if *ty == Type::Ref && tipos[origem] == Type::Ref => rs.push(Restricao::Copiar {
                origem: ids[origem],
                destino,
            }),
            i if fatos.objetos.contains_key(v) => {
                let origem = &fatos.objetos[v];
                rs.push(Restricao::Semear {
                    destino,
                    no: origem.no.clone(),
                });
                let iniciais = match i {
                    Instruction::AllocObject { fields, .. } => fields.clone(),
                    _ => vec![Operand::Constant(Constant::Null); origem.campos.len()],
                };
                for (o, campo) in iniciais.iter().zip(&origem.campos) {
                    rs.push(Restricao::Gravar {
                        objeto: destino,
                        campo: campo.clone(),
                        valor: operando(o),
                    });
                }
            }
            Instruction::CallStatic {
                symbol,
                args,
                ret_ty,
            } if resumos.contains_key(symbol) => {
                let (callee, resumo) = &resumos[symbol];
                if args.len() != callee.params.len() {
                    return Err(format!("points-to: aridade de {symbol}"));
                }
                let abi_exata = *ty == *ret_ty
                    && *ret_ty == callee.return_ty
                    && args
                        .iter()
                        .zip(&callee.params)
                        .all(|(o, (_, _, t))| super::resumos::argumento_exato(o, *t, &tipos));
                if let super::resumos::RetornoHeapArc::Aliases { parametros, .. } = resumo.retorno()
                    && *ty == Type::Ref
                    && *ret_ty == Type::Ref
                {
                    for &p in parametros {
                        if let Operand::Val(arg) = &args[p] {
                            if tipos[arg] != Type::Ref {
                                return Err(format!(
                                    "points-to: argumento Ref incompatível em {symbol}"
                                ));
                            }
                        }
                        rs.push(Restricao::Copiar {
                            origem: operando(&args[p]),
                            destino,
                        });
                    }
                } else if *ty != Type::Void {
                    rs.push(Restricao::Desconhecer { destino });
                }
                dependencias_corpos.insert(symbol.clone(), assinatura_corpo(callee));
                // Cobertura de campos é separada do alias normal. Conversões
                // de argumentos podem alocar/reentrar antes do corpo coberto.
                if !abi_exata
                    || resumo.publicacoes() != super::resumos::PublicacoesHeapArc::SomenteResultado
                {
                    desconhecidos.push(*v);
                }
                if !abi_exata || resumo.campos() != super::resumos::CamposHeapArc::Preservados {
                    for campo in &campos {
                        rs.push(Restricao::DesconhecerCampo {
                            campo: campo.clone(),
                        });
                    }
                }
            }
            Instruction::GetField { object, .. } if fatos.acessos.contains_key(v) => {
                rs.push(Restricao::Ler {
                    objeto: operando(object),
                    campo: fatos.acessos[v].clone(),
                    destino,
                })
            }
            Instruction::SetField { object, value, .. } if fatos.acessos.contains_key(v) => rs
                .push(Restricao::Gravar {
                    objeto: operando(object),
                    campo: fatos.acessos[v].clone(),
                    valor: operando(value),
                }),
            Instruction::Add(..)
            | Instruction::Sub(..)
            | Instruction::Mul(..)
            | Instruction::SDiv(..)
            | Instruction::SRem(..)
            | Instruction::Shl(..)
            | Instruction::AShr(..)
            | Instruction::LShr(..)
            | Instruction::And(..)
            | Instruction::Or(..)
            | Instruction::Xor(..)
            | Instruction::Neg(..)
            | Instruction::Not(..)
            | Instruction::FAdd(..)
            | Instruction::FSub(..)
            | Instruction::FMul(..)
            | Instruction::FDiv(..)
            | Instruction::FNeg(..)
            | Instruction::ICmp(..)
            | Instruction::FCmp(..)
            | Instruction::LNot(..)
            | Instruction::IntToDouble(..)
            | Instruction::DoubleToInt(..)
            | Instruction::ZExt { .. }
            | Instruction::Trunc { .. }
                if matches!(ty, Type::I64 | Type::F64 | Type::I1 | Type::I8) => {}
            _ => {
                desconhecidos.push(*v);
                if *ty != Type::Void {
                    rs.push(Restricao::Desconhecer { destino });
                }
                for campo in &campos {
                    rs.push(Restricao::DesconhecerCampo {
                        campo: campo.clone(),
                    });
                }
            }
        }
    }
    let pontos = resolver(ids.len() + 2, limite, &rs)?;
    let mut esquemas: HashMap<_, BTreeSet<_>> = HashMap::new();
    for origem in fatos.objetos.values() {
        esquemas
            .entry(origem.no.clone())
            .or_default()
            .extend(origem.campos.iter().cloned());
    }
    Ok(AnaliseHir {
        pontos,
        desconhecidos,
        ids,
        corpo: assinatura_corpo(f),
        dependencias_corpos,
        dependencias_tabelas: HashMap::new(),
        premissas_modulo: None,
        limite,
        esquemas,
    })
}

// Identidade local para rejeitar consumo de uma solução de outra versão.
// Não é formato canônico de resumo SDK nem certificado de recarga.
pub(crate) fn assinatura_corpo(f: &Function) -> blake3::Hash {
    blake3::hash(format!("{f:?}").as_bytes())
}

// Só metadados de emissão usados para a cobertura local de campos.
// Não substitui versões de layouts, runtime ou geração JIT.
pub(crate) fn assinatura_tabela(m: &Module, indice: usize) -> blake3::Hash {
    let tabela = if m.excecoes_por_tabelas {
        m.tabelas.get(indice)
    } else {
        None
    };
    blake3::hash(format!("{}:{tabela:?}", m.excecoes_por_tabelas).as_bytes())
}

pub(super) fn constantes_inteiras(f: &Function) -> HashMap<ValueId, i64> {
    f.blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter_map(|(v, i, ty)| {
            if let Instruction::Const(Constant::Int(n)) = i
                && *ty == Type::I64
            {
                Some((*v, *n))
            } else {
                None
            }
        })
        .collect()
}

// Somente a fábrica reservada de instância zerada, sem callback ou construtor.
// Classe/contagem dinâmicas não são inferidas por largura ou nome de usuário.
pub(super) fn fabrica_zerada(
    i: &Instruction,
    constantes: &HashMap<ValueId, i64>,
) -> Option<(u32, usize)> {
    let Instruction::CallRuntime { name, args, ret_ty } = i else {
        return None;
    };
    if name != "dartforge_object_new"
        || args.len() != 2
        || *ret_ty != Type::Ref
        || args.iter().any(|(_, t)| *t != Type::I64)
    {
        return None;
    }
    let inteiro = |o: &Operand| match o {
        Operand::Constant(Constant::Int(n)) => Some(*n),
        Operand::Val(v) => constantes.get(v).copied(),
        _ => None,
    };
    Some((
        u32::try_from(inteiro(&args[0].0)?).ok()?,
        usize::try_from(inteiro(&args[1].0)?).ok()?,
    ))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn fixture() -> (Function, FatosHir) {
        let campo = CampoArc {
            layout: "No:v1".into(),
            posicao: 0,
        };
        let mut fatos = FatosHir::default();
        for v in [0, 1] {
            fatos.objetos.insert(
                ValueId(v),
                OrigemObjeto {
                    no: NoAbstrato::Alocacao {
                        sitio: SitioArc {
                            funcao: "ligar".into(),
                            origem: v as u64,
                            especializacao: "".into(),
                        },
                        contexto: "".into(),
                    },
                    campos: vec![campo.clone()],
                },
            );
        }
        fatos.acessos.insert(ValueId(2), campo.clone());
        fatos.acessos.insert(ValueId(3), campo);
        let f = Function {
            symbol: "ligar".into(),
            name: "ligar".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(0),
                        Instruction::AllocObject {
                            class_id: 1,
                            fields: vec![Operand::Constant(Constant::Null)],
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
                        Instruction::SetField {
                            object: Operand::Val(ValueId(0)),
                            index: 0,
                            value: Operand::Val(ValueId(1)),
                        },
                        Type::Void,
                    ),
                    (
                        ValueId(3),
                        Instruction::GetField {
                            object: Operand::Val(ValueId(0)),
                            index: 0,
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
            }],
        };
        (f, fatos)
    }

    #[test]
    fn inicializadores_e_stores_da_hir_preservam_ciclo_e_identidade() {
        let (f, fatos) = fixture();
        let a = analisar(&f, &fatos, 8).unwrap();
        assert!(a.desconhecidos.is_empty());
        assert_eq!(a.valor(ValueId(3)), a.valor(ValueId(1)));
        assert_eq!(
            a.pontos
                .campo(&fatos.objetos[&ValueId(1)].no, &fatos.acessos[&ValueId(3)]),
            a.valor(ValueId(0))
        );
    }

    #[test]
    fn chamadas_opacas_e_fatos_ausentes_nunca_provam_ausencia_de_aliases() {
        let (mut f, mut fatos) = fixture();
        f.blocks[0].instructions.insert(
            3,
            (
                ValueId(4),
                Instruction::CallRuntime {
                    name: "externa_sem_resumo".into(),
                    args: vec![],
                    ret_ty: Type::Void,
                },
                Type::Void,
            ),
        );
        let a = analisar(&f, &fatos, 8).unwrap();
        assert_eq!(a.desconhecidos, vec![ValueId(4)]);
        assert!(a.valor(ValueId(3)).unwrap().nos().is_none());
        f.blocks[0].instructions.remove(3);
        fatos.objetos.remove(&ValueId(1));
        let a = analisar(&f, &fatos, 8).unwrap();
        assert!(a.valor(ValueId(1)).unwrap().nos().is_none());
        assert!(a.valor(ValueId(3)).unwrap().nos().is_none());
    }

    #[test]
    fn fatos_obsoletos_e_esquemas_incompativeis_sao_recusados() {
        let (f, mut fatos) = fixture();
        fatos.objetos.get_mut(&ValueId(0)).unwrap().campos.clear();
        assert!(analisar(&f, &fatos, 8).is_err());
        let (f, mut fatos) = fixture();
        fatos.acessos.get_mut(&ValueId(3)).unwrap().posicao = 1;
        assert!(analisar(&f, &fatos, 8).is_err());
    }

    #[test]
    fn bits_inteiros_e_campos_iniciais_sem_tipo_nao_provam_ausencia_de_handle() {
        let (mut f, fatos) = fixture();
        if let Instruction::AllocObject { fields, .. } = &mut f.blocks[0].instructions[0].1 {
            fields[0] = Operand::Constant(Constant::Int(42));
        }
        let a = analisar(&f, &fatos, 8).unwrap();
        assert!(a.valor(ValueId(3)).unwrap().nos().is_none());
        let f = Function {
            symbol: "bits".into(),
            name: "bits".into(),
            depuracao: None,
            params: vec![(ValueId(0), "bits".into(), Type::I64)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(1),
                    Instruction::Bitcast {
                        op: Operand::Val(ValueId(0)),
                        to: Type::Ref,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
            }],
        };
        let a = analisar(&f, &FatosHir::default(), 8).unwrap();
        assert!(a.valor(ValueId(0)).unwrap().nos().is_none());
        assert!(a.valor(ValueId(1)).unwrap().nos().is_none());
    }
}
