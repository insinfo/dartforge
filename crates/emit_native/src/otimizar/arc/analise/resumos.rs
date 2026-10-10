//! Resumos iniciais de aliases de retorno normal (§28.3).
//! Preservação de campos por cobertura positiva de corpos e chamadas diretas.
//! Publicação só pelo resultado exige cobertura positiva do corpo inteiro.
//! Não certificam políticas, vidas ou efeitos completos de callbacks/SDK.

use super::{hir::assinatura_corpo, modelo::*, points_to::*};
use crate::hir::*;
use std::collections::{BTreeSet, HashMap, VecDeque};

/// Resultado normal paramétrico; parâmetros são substituídos pelos aliases
/// reais do chamador, nunca por objetos frescos independentes.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::analise::resumos::RetornoHeapArc;
/// assert!(matches!(RetornoHeapArc::Desconhecido, RetornoHeapArc::Desconhecido));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetornoHeapArc {
    /// Retorno normal pode coincidir com qualquer parâmetro listado; `nulo`
    /// registra null adicional literal. Parâmetros podem ser null mesmo com
    /// essa marca falsa: ela não prova não nulabilidade dos argumentos.
    Aliases {
        parametros: BTreeSet<usize>,
        nulo: bool,
    },
    /// Precisão insuficiente, representação opaca ou budget excedido.
    Desconhecido,
}

/// Efeito conservador sobre campos existentes, em todas as saídas do corpo.
/// Não cobre vida, retenção, publicação ou a convenção Owned/Borrowed.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::analise::resumos::CamposHeapArc;
/// assert_ne!(CamposHeapArc::Preservados, CamposHeapArc::Desconhecidos);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CamposHeapArc {
    /// Corpo coberto preserva campos, incluindo callees com a mesma cobertura.
    Preservados,
    /// Escrita possível ou operação/caminho implícito ainda não coberto.
    Desconhecidos,
}

/// Publicações possíveis do corpo, separadas dos aliases do resultado.
/// Consumo requer conferir corpo, ABI e caminhos implícitos de emissão.
///
/// ```
/// use dartforge_emit_native::otimizar::arc::analise::resumos::PublicacoesHeapArc;
/// assert_ne!(PublicacoesHeapArc::SomenteResultado, PublicacoesHeapArc::Desconhecidas);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicacoesHeapArc {
    /// Referências só podem sair pelos valores devolvidos ao chamador.
    /// Não há publicação em global/campo, retenção, callback ou throw no corpo.
    SomenteResultado,
    /// Cobertura insuficiente de qualquer saída/operação, incluindo implícitas.
    Desconhecidas,
}

/// Resumo de uma versão local do corpo, separado de alcance e efeitos.tsv.
/// Só o extrator constrói seus campos. Aliases são de retorno normal;
/// preservação de campos e publicação pelo resultado exigem cobertura própria
/// do corpo inteiro. Demais corpos conservam efeitos desconhecidos.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::analise::resumos::*};
/// let f = Function { symbol: "id".into(), name: "id".into(), depuracao: None,
/// params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
/// blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
/// terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
/// let resumo = extrair(&f, 8)?;
/// assert!(matches!(resumo.retorno(), RetornoHeapArc::Aliases { nulo: false, .. }));
/// # Ok::<(), String>(())
/// ```
#[derive(Debug)]
pub struct ResumoHeapArc {
    corpo: blake3::Hash,
    retorno: RetornoHeapArc,
    campos: CamposHeapArc,
    publicacoes: PublicacoesHeapArc,
    pub(super) inclusoes_escritas: super::escritas::InclusoesEscritas,
}

impl ResumoHeapArc {
    /// Consulta publicações sem confundir alias normal com ausência de retenção.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, otimizar::arc::analise::resumos::*};
    /// let f = Function { symbol: "id".into(), name: "id".into(), depuracao: None,
    /// params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
    /// blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
    /// terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
    /// assert_eq!(extrair(&f, 8)?.publicacoes(), PublicacoesHeapArc::SomenteResultado);
    /// # Ok::<(), String>(())
    /// ```
    pub fn publicacoes(&self) -> PublicacoesHeapArc {
        self.publicacoes
    }

    /// Consulta preservação de campos; isso não fornece noescape ou vida.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, otimizar::arc::analise::resumos::*};
    /// let f = Function { symbol: "id".into(), name: "id".into(), depuracao: None,
    /// params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
    /// blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
    /// terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
    /// assert_eq!(extrair(&f, 8)?.campos(), CamposHeapArc::Preservados);
    /// # Ok::<(), String>(())
    /// ```
    pub fn campos(&self) -> CamposHeapArc {
        self.campos
    }

    pub(super) fn invalidar_cobertura(&mut self) {
        self.campos = CamposHeapArc::Desconhecidos;
        self.publicacoes = PublicacoesHeapArc::Desconhecidas;
    }

    pub(super) fn marcar_cobertura(&mut self) {
        self.campos = CamposHeapArc::Preservados;
        self.publicacoes = PublicacoesHeapArc::SomenteResultado;
    }

    /// Consulta aliases normais; isso não fornece noescape ou noheapmutation.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, otimizar::arc::analise::resumos::*};
    /// let f = Function { symbol: "n".into(), name: "n".into(), depuracao: None,
    /// params: vec![], return_ty: Type::Ref, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) }] };
    /// assert!(matches!(extrair(&f, 8)?.retorno(), RetornoHeapArc::Aliases { nulo: true, .. }));
    /// # Ok::<(), String>(())
    /// ```
    pub fn retorno(&self) -> &RetornoHeapArc {
        &self.retorno
    }

    /// Recusa consumo após alteração do corpo. Guarda local, sem validade SDK/JIT.
    ///
    /// # Erros
    /// Assinatura local diferente da versão resumida.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, otimizar::arc::analise::resumos::*};
    /// let f = Function { symbol: "v".into(), name: "v".into(), depuracao: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
    /// extrair(&f, 8)?.conferir(&f)?;
    /// # Ok::<(), String>(())
    /// ```
    pub fn conferir(&self, f: &Function) -> Result<(), String> {
        if self.corpo == assinatura_corpo(f) {
            Ok(())
        } else {
            Err("resumo heap ARC: corpo mudou".into())
        }
    }

    /// Substitui placeholders pelos conjuntos reais dos argumentos, sem frescor.
    /// Null não cria nó; conjunto vazio não certifica ausência de efeitos.
    /// Budget excedido/argumento desconhecido propaga topo. Continua sendo
    /// resultado normal, sem eliminar efeitos de heap ou escapes da chamada.
    ///
    /// # Erros
    /// Corpo obsoleto ou quantidade de argumentos diferente da ABI resumida.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, otimizar::arc::analise::{resumos::*, modelo::*}};
    /// let f = Function { symbol: "id".into(), name: "id".into(), depuracao: None,
    /// params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
    /// blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
    /// terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
    /// let mut arg = ConjuntoPontos::vazio(8); arg.incluir(NoAbstrato::Global("raiz".into()));
    /// assert_eq!(extrair(&f, 8)?.instanciar(&f, &[arg.clone()], 8)?, arg);
    /// # Ok::<(), String>(())
    /// ```
    pub fn instanciar(
        &self,
        f: &Function,
        argumentos: &[ConjuntoPontos],
        limite: usize,
    ) -> Result<ConjuntoPontos, String> {
        self.conferir(f)?;
        if argumentos.len() != f.params.len() {
            return Err("resumo heap ARC: aridade incompatível".into());
        }
        let RetornoHeapArc::Aliases { parametros, .. } = &self.retorno else {
            return Ok(ConjuntoPontos::desconhecido(limite));
        };
        let mut retorno = ConjuntoPontos::vazio(limite);
        for &p in parametros {
            retorno.unir(&argumentos[p]);
        }
        Ok(retorno)
    }
}

/// Extrai inclusão de aliases de parâmetros/null nas saídas normais Ref.
/// Copy/Move/Phi e bitcast Ref→Ref propagam aliases; demais produtores viram
/// desconhecidos. Placeholders servem apenas ao resumo, não são alocações.
/// Resumo não elimina efeitos ou muda o contrato Owned/Borrowed da ABI.
///
/// # Erros
/// SSA inválida ou número de parâmetros fora do domínio de índices.
///
/// ```
/// use dartforge_emit_native::{hir::*, otimizar::arc::analise::resumos::*};
/// let f = Function { symbol: "v".into(), name: "v".into(), depuracao: None,
/// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
/// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
/// assert_eq!(extrair(&f, 8)?.retorno(), &RetornoHeapArc::Desconhecido);
/// # Ok::<(), String>(())
/// ```
pub fn extrair(f: &Function, limite: usize) -> Result<ResumoHeapArc, String> {
    extrair_com_chamadas(f, limite, false, |_| None)
}

// Vazio sem null é bottom somente durante a solução interprocedural.
// O solver o eleva a desconhecido antes de disponibilizar o resultado.
pub(super) fn extrair_com_chamadas<'a>(
    f: &Function,
    limite: usize,
    interno: bool,
    chamada: impl Fn(&str) -> Option<(&'a Function, &'a RetornoHeapArc)>,
) -> Result<ResumoHeapArc, String> {
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
    let mut rs = vec![Restricao::Desconhecer { destino: opaco }];
    let op = |o: &Operand| match o {
        Operand::Val(v) => ids[v],
        Operand::Constant(Constant::Null) => nulo,
        _ => opaco,
    };
    for (indice, (v, _, t)) in f.params.iter().enumerate() {
        if *t == Type::Ref {
            rs.push(Restricao::Semear {
                destino: ids[v],
                no: NoAbstrato::Parametro {
                    funcao: f.symbol.clone(),
                    indice: u32::try_from(indice).map_err(|_| "resumo: parâmetros demais")?,
                },
            });
        } else {
            rs.push(Restricao::Desconhecer { destino: ids[v] });
        }
    }
    let mut copias = Vec::new();
    for (v, i, t) in f.blocks.iter().flat_map(|b| &b.instructions) {
        let destino = ids[v];
        match i {
            Instruction::Const(Constant::Null) => copias.push((nulo, destino)),
            Instruction::ArcCopy { value } | Instruction::ArcMove { value } if *t == Type::Ref => {
                copias.push((op(value), destino))
            }
            Instruction::Bitcast {
                op: Operand::Val(origem),
                ..
            } if *t == Type::Ref && tipos[origem] == Type::Ref => {
                copias.push((ids[origem], destino))
            }
            Instruction::Phi { incoming, .. } if *t == Type::Ref => {
                for (_, o) in incoming {
                    copias.push((op(o), destino));
                }
            }
            Instruction::CallStatic {
                symbol,
                args,
                ret_ty,
            } if *t == Type::Ref && *ret_ty == Type::Ref => match chamada(symbol) {
                Some((
                    callee,
                    RetornoHeapArc::Aliases {
                        parametros,
                        nulo: null,
                    },
                )) => {
                    if args.len() != callee.params.len() {
                        return Err(format!("resumo: aridade de {symbol}"));
                    }
                    for &p in parametros {
                        if let Operand::Val(v) = &args[p]
                            && tipos[v] != Type::Ref
                        {
                            return Err(format!("resumo: argumento Ref de {symbol}"));
                        }
                        copias.push((op(&args[p]), destino));
                    }
                    if *null {
                        copias.push((nulo, destino));
                    }
                }
                _ => rs.push(Restricao::Desconhecer { destino }),
            },
            _ => rs.push(Restricao::Desconhecer { destino }),
        }
    }
    for &(origem, destino) in &copias {
        rs.push(Restricao::Copiar { origem, destino });
    }
    let pontos = resolver(ids.len() + 2, limite, &rs)?;
    // Nulabilidade tem worklist própria e não cria um nó físico para null.
    let mut dependentes = vec![Vec::new(); ids.len() + 2];
    for (origem, destino) in copias {
        dependentes[origem].push(destino);
    }
    let mut nulos = vec![false; ids.len() + 2];
    nulos[nulo] = true;
    let mut fila = VecDeque::from([nulo]);
    while let Some(v) = fila.pop_front() {
        for &destino in &dependentes[v] {
            if !nulos[destino] {
                nulos[destino] = true;
                fila.push_back(destino);
            }
        }
    }
    let mut parametros = BTreeSet::new();
    let mut pode_null = false;
    let mut normal = false;
    let mut desconhecido = f.return_ty != Type::Ref;
    for b in &f.blocks {
        if let Terminator::Return(Some(o)) = &b.terminator {
            normal = true;
            let id = op(o);
            pode_null |= nulos[id];
            match pontos.variavel(id).unwrap().nos() {
                None => desconhecido = true,
                Some(nos) => {
                    for no in nos {
                        if let NoAbstrato::Parametro { indice, .. } = no {
                            parametros.insert(*indice as usize);
                        } else {
                            desconhecido = true;
                        }
                    }
                }
            }
        }
    }
    let retorno = if desconhecido
        || parametros.len() > limite
        || (!interno && (!normal || (parametros.is_empty() && !pode_null)))
    {
        RetornoHeapArc::Desconhecido
    } else {
        RetornoHeapArc::Aliases {
            parametros,
            nulo: pode_null,
        }
    };
    let coberto = cobertura_de_valores(f, &tipos, |_| None);
    Ok(ResumoHeapArc {
        corpo: assinatura_corpo(f),
        retorno,
        inclusoes_escritas: super::escritas::extrair(f, limite, |_| None)?,
        campos: if coberto {
            CamposHeapArc::Preservados
        } else {
            CamposHeapArc::Desconhecidos
        },
        publicacoes: if coberto {
            PublicacoesHeapArc::SomenteResultado
        } else {
            PublicacoesHeapArc::Desconhecidas
        },
    })
}

// Lista positiva independente dos aliases normais. Excluir chamadas opacas, throws,
// stores, alocações, constantes de texto e ARC que possa exigir contexto:
// conferência implícita de pilha pode construir erro pelo SDK/reentrar.
fn cobertura_de_valores<'a>(
    f: &Function,
    tipos: &HashMap<ValueId, Type>,
    chamada: impl Fn(&str) -> Option<(&'a Function, &'a ResumoHeapArc)>,
) -> bool {
    let exato = |o: &Operand, t: Type| argumento_exato(o, t, tipos);
    let corpo_coberto = f.blocks.iter().all(|b| {
        let saida = match &b.terminator {
            Terminator::Return(Some(o)) => exato(o, f.return_ty),
            Terminator::Return(None) => f.return_ty == Type::Void,
            Terminator::CondBranch { cond, .. } => exato(cond, Type::I1),
            Terminator::Switch { val, .. } => exato(val, Type::I64),
            Terminator::Branch(_) | Terminator::Unreachable => true,
            Terminator::Throw(_) => false,
        };
        saida
            && b.instructions.iter().all(|(_, i, t)| match i {
                Instruction::Const(c) => exato(&Operand::Constant(c.clone()), *t),
                Instruction::Phi { incoming, ty } => {
                    ty == t && incoming.iter().all(|(_, o)| exato(o, *t))
                }
                Instruction::CallStatic {
                    symbol,
                    args,
                    ret_ty,
                } => chamada(symbol).is_some_and(|(callee, resumo)| {
                    resumo.campos == CamposHeapArc::Preservados
                        && resumo.publicacoes == PublicacoesHeapArc::SomenteResultado
                        && *t == *ret_ty
                        && *ret_ty == callee.return_ty
                        && args.len() == callee.params.len()
                        && args
                            .iter()
                            .zip(&callee.params)
                            .all(|(o, (_, _, ty))| exato(o, *ty))
                }),
                i => super::hir::escalar_puro(i, *t, tipos),
            })
    });
    corpo_coberto
}

// Cobertura composta exige todos os callees já cobertos; desconhecimento
// não é hipótese coindutiva para aprovar uma SCC recursiva sem uma base.
pub(super) fn cobertura_com_chamadas<'a>(
    f: &Function,
    chamada: impl Fn(&str) -> Option<(&'a Function, &'a ResumoHeapArc)>,
) -> bool {
    let tipos = f
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
    cobertura_de_valores(f, &tipos, chamada)
}

// Igualdade física não permite converter I64 em Ref ou materializar texto.
pub(super) fn argumento_exato(o: &Operand, t: Type, tipos: &HashMap<ValueId, Type>) -> bool {
    match o {
        Operand::Val(v) => tipos.get(v) == Some(&t),
        Operand::Constant(Constant::Null) => t == Type::Ref,
        Operand::Constant(Constant::Int(_)) => t == Type::I64,
        Operand::Constant(Constant::Double(_)) => t == Type::F64,
        Operand::Constant(Constant::Bool(_)) => t == Type::I1,
        _ => false,
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    fn identidade() -> Function {
        Function {
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
        }
    }
    #[test]
    fn cobertura_escalar_exige_operacao_e_representacoes_exatas() {
        let x = Operand::Val(ValueId(0));
        let y = Operand::Val(ValueId(1));
        let casos = vec![
            (Instruction::Add(x.clone(), y.clone()), Type::I64, Type::I64),
            (Instruction::Sub(x.clone(), y.clone()), Type::I64, Type::I64),
            (Instruction::Mul(x.clone(), y.clone()), Type::I64, Type::I64),
            (Instruction::Shl(x.clone(), y.clone()), Type::I64, Type::I64),
            (
                Instruction::AShr(x.clone(), y.clone()),
                Type::I64,
                Type::I64,
            ),
            (
                Instruction::LShr(x.clone(), y.clone()),
                Type::I64,
                Type::I64,
            ),
            (Instruction::And(x.clone(), y.clone()), Type::I64, Type::I64),
            (Instruction::Or(x.clone(), y.clone()), Type::I64, Type::I64),
            (Instruction::Xor(x.clone(), y.clone()), Type::I64, Type::I64),
            (Instruction::Neg(x.clone()), Type::I64, Type::I64),
            (Instruction::Not(x.clone()), Type::I64, Type::I64),
            (
                Instruction::ICmp(ICmpOp::Eq, x.clone(), y.clone()),
                Type::I64,
                Type::I1,
            ),
            (
                Instruction::FAdd(x.clone(), y.clone()),
                Type::F64,
                Type::F64,
            ),
            (
                Instruction::FSub(x.clone(), y.clone()),
                Type::F64,
                Type::F64,
            ),
            (
                Instruction::FMul(x.clone(), y.clone()),
                Type::F64,
                Type::F64,
            ),
            (
                Instruction::FDiv(x.clone(), y.clone()),
                Type::F64,
                Type::F64,
            ),
            (Instruction::FNeg(x.clone()), Type::F64, Type::F64),
            (
                Instruction::FCmp(FCmpOp::Eq, x.clone(), y.clone()),
                Type::F64,
                Type::I1,
            ),
            (Instruction::LNot(x.clone()), Type::I1, Type::I1),
            (Instruction::IntToDouble(x.clone()), Type::I64, Type::F64),
            (
                Instruction::ZExt {
                    op: x.clone(),
                    from: Type::I1,
                    to: Type::I64,
                },
                Type::I1,
                Type::I64,
            ),
            (
                Instruction::Trunc {
                    op: x.clone(),
                    from: Type::I64,
                    to: Type::I8,
                },
                Type::I64,
                Type::I8,
            ),
        ];
        for (i, entrada, saida) in casos {
            let mut f = Function {
                symbol: "calculo".into(),
                name: "calculo".into(),
                depuracao: None,
                params: vec![
                    (ValueId(0), "x".into(), entrada),
                    (ValueId(1), "y".into(), entrada),
                ],
                return_ty: saida,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(ValueId(2), i.clone(), saida)],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))),
                }],
            };
            let r = extrair(&f, 8).unwrap();
            assert_eq!(r.campos(), CamposHeapArc::Preservados, "{i:?}");
            assert_eq!(
                r.publicacoes(),
                PublicacoesHeapArc::SomenteResultado,
                "{i:?}"
            );
            // Mesmo um resultado escalar não fornece alias Ref nem frescor.
            assert_eq!(r.retorno(), &RetornoHeapArc::Desconhecido);
            let mut m = Module::new();
            m.memoria_arc = true;
            m.functions.push(f.clone());
            let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
            // O módulo também contém portas/registro do runtime; a obrigação
            // de ausência de chamada vale para o corpo resumido, não para elas.
            let corpo: Vec<_> = ir
                .lines()
                .skip_while(|l| !(l.starts_with("define ") && l.contains("@calculo(")))
                .take_while(|l| *l != "}")
                .collect();
            assert!(!corpo.is_empty(), "corpo não emitido: {i:?}");
            assert!(!corpo.iter().any(|l| l.contains("pilha.estouro:")), "{i:?}");
            assert!(
                !corpo
                    .iter()
                    .any(|l| l.trim_start().starts_with("call ") || l.contains(" = call ")),
                "{i:?}"
            );
            f.params[0].2 = Type::Ref;
            let r = extrair(&f, 8).unwrap();
            assert_eq!(r.campos(), CamposHeapArc::Desconhecidos, "entrada {i:?}");
            assert_eq!(r.publicacoes(), PublicacoesHeapArc::Desconhecidas);
            f.params[0].2 = entrada;
            f.blocks[0].instructions[0].2 = Type::Ref;
            f.return_ty = Type::Ref;
            assert_eq!(
                extrair(&f, 8).unwrap().campos(),
                CamposHeapArc::Desconhecidos,
                "saída {i:?}"
            );
        }
        for (i, entrada) in [
            (Instruction::SDiv(x.clone(), y.clone()), Type::I64),
            (Instruction::SRem(x.clone(), y.clone()), Type::I64),
            (Instruction::DoubleToInt(x.clone()), Type::F64),
            (
                Instruction::Box {
                    op: x.clone(),
                    from: Type::I64,
                },
                Type::I64,
            ),
            (
                Instruction::Unbox {
                    op: x,
                    to: Type::I64,
                },
                Type::Ref,
            ),
        ] {
            let mut f = identidade();
            f.params[0].2 = entrada;
            f.params.push((ValueId(1), "y".into(), entrada));
            f.return_ty = Type::I64;
            f.blocks[0]
                .instructions
                .push((ValueId(2), i.clone(), Type::I64));
            f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(2))));
            assert_eq!(
                extrair(&f, 8).unwrap().campos(),
                CamposHeapArc::Desconhecidos,
                "falível {i:?}"
            );
        }
    }

    #[test]
    fn cobertura_de_campos_nao_deriva_de_alias_e_inclui_saidas_excepcionais() {
        let mut f = identidade();
        assert_eq!(extrair(&f, 8).unwrap().campos(), CamposHeapArc::Preservados);
        f.blocks[0].terminator = Terminator::Throw(Operand::Val(ValueId(0)));
        assert_eq!(
            extrair(&f, 8).unwrap().campos(),
            CamposHeapArc::Desconhecidos
        );
        f = identidade();
        f.blocks[0].instructions.push((
            ValueId(1),
            Instruction::ArcCopy {
                value: Operand::Val(ValueId(0)),
            },
            Type::Ref,
        ));
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
        let resumo = extrair(&f, 8).unwrap();
        assert!(matches!(resumo.retorno(), RetornoHeapArc::Aliases { .. }));
        assert_eq!(resumo.campos(), CamposHeapArc::Desconhecidos);
        assert_eq!(resumo.publicacoes(), PublicacoesHeapArc::Desconhecidas);
        // Uma constante de texto aloca no JIT, mesmo sem chamada HIR explícita.
        f = identidade();
        f.blocks[0].instructions.push((
            ValueId(1),
            Instruction::Const(Constant::String("texto".into())),
            Type::Ref,
        ));
        assert_eq!(
            extrair(&f, 8).unwrap().campos(),
            CamposHeapArc::Desconhecidos
        );
    }

    #[test]
    fn identidade_preserva_placeholder_e_recusa_versao_obsoleta() {
        let mut f = identidade();
        let r = extrair(&f, 8).unwrap();
        assert_eq!(
            r.retorno(),
            &RetornoHeapArc::Aliases {
                parametros: BTreeSet::from([0]),
                nulo: false
            }
        );
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Constant(Constant::Null)));
        assert!(r.conferir(&f).is_err());
        assert_eq!(
            extrair(&f, 8).unwrap().retorno(),
            &RetornoHeapArc::Aliases {
                parametros: BTreeSet::new(),
                nulo: true
            }
        );
    }
    #[test]
    fn produtor_opaco_largura_e_budget_nao_fabricam_alias_de_parametro() {
        let mut f = identidade();
        assert_eq!(
            extrair(&f, 0).unwrap().retorno(),
            &RetornoHeapArc::Desconhecido
        );
        f.params[0].2 = Type::I64;
        f.return_ty = Type::I64;
        assert_eq!(
            extrair(&f, 8).unwrap().retorno(),
            &RetornoHeapArc::Desconhecido
        );
        f.params[0].2 = Type::Ref;
        f.return_ty = Type::Ref;
        f.blocks[0].instructions.push((
            ValueId(1),
            Instruction::CallRuntime {
                name: "opaca".into(),
                args: vec![],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        ));
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
        assert_eq!(
            extrair(&f, 8).unwrap().retorno(),
            &RetornoHeapArc::Desconhecido
        );
    }

    #[test]
    fn instanciacao_preserva_aliases_do_chamador_e_perde_precisao_por_budget() {
        let f = identidade();
        let r = extrair(&f, 8).unwrap();
        let mut arg = ConjuntoPontos::vazio(8);
        arg.incluir(NoAbstrato::Global("a".into()));
        arg.incluir(NoAbstrato::Global("b".into()));
        assert_eq!(r.instanciar(&f, &[arg.clone()], 8).unwrap(), arg);
        assert!(r.instanciar(&f, &[arg], 1).unwrap().nos().is_none());
        assert!(
            r.instanciar(&f, &[ConjuntoPontos::desconhecido(8)], 8)
                .unwrap()
                .nos()
                .is_none()
        );
        assert!(r.instanciar(&f, &[], 8).is_err());
    }

    #[test]
    fn phi_une_parametros_e_null_sem_transformar_placeholder_em_objeto_fresco() {
        let mut f = identidade();
        f.params.extend([
            (ValueId(1), "y".into(), Type::Ref),
            (ValueId(2), "cond".into(), Type::I1),
        ]);
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
                instructions: vec![(
                    ValueId(3),
                    Instruction::Phi {
                        incoming: vec![
                            (BlockId(1), Operand::Val(ValueId(0))),
                            (BlockId(2), Operand::Constant(Constant::Null)),
                        ],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
            },
        ];
        assert_eq!(
            extrair(&f, 8).unwrap().retorno(),
            &RetornoHeapArc::Aliases {
                parametros: BTreeSet::from([0]),
                nulo: true
            }
        );
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[3].instructions[0].1 {
            incoming[1].1 = Operand::Val(ValueId(1));
        }
        assert_eq!(
            extrair(&f, 8).unwrap().retorno(),
            &RetornoHeapArc::Aliases {
                parametros: BTreeSet::from([0, 1]),
                nulo: false
            }
        );
    }
}
