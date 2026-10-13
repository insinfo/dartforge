//! Resumos iniciais de aliases de retorno normal (§28.3).
//! Preservação de campos por cobertura positiva de corpos e chamadas diretas.
//! Publicação só pelo resultado exige cobertura positiva do corpo inteiro.
//! Não certificam políticas, vidas ou efeitos completos de callbacks/SDK.

use super::{hir::body_signature, model::*, points_to::*};
use crate::hir::*;
use std::collections::{BTreeSet, HashMap, VecDeque};

/// Resultado normal paramétrico; parâmetros são substituídos pelos aliases
/// reais do chamador, nunca por objetos frescos independentes.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analise::resumos::RetornoHeapArc;
/// assert!(matches!(RetornoHeapArc::Desconhecido, RetornoHeapArc::Desconhecido));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArcHeapReturn {
    /// Retorno normal pode coincidir com qualquer parâmetro listado; `null`
    /// registra null adicional literal. Parâmetros podem ser null mesmo com
    /// essa marca falsa: ela não prova não nulabilidade dos argumentos.
    Aliases {
        parameters: BTreeSet<usize>,
        null: bool,
    },
    /// Precisão insuficiente, representação opaca ou budget excedido.
    UnknownValue,
}

/// Efeito conservador sobre campos existentes, em todas as saídas do corpo.
/// Não cobre vida, retenção, publicação ou a convenção Owned/Borrowed.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analise::resumos::CamposHeapArc;
/// assert_ne!(CamposHeapArc::Preservados, CamposHeapArc::Desconhecidos);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcHeapFields {
    /// Corpo coberto preserva campos, incluindo callees com a mesma cobertura.
    Preserved,
    /// Escrita possível ou operação/caminho implícito ainda não coberto.
    UnknownSet,
}

/// Publicações possíveis do corpo, separadas dos aliases do resultado.
/// Consumo requer conferir corpo, ABI e caminhos implícitos de emissão.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analise::resumos::PublicacoesHeapArc;
/// assert_ne!(PublicacoesHeapArc::SomenteResultado, PublicacoesHeapArc::Desconhecidas);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcHeapPublications {
    /// Referências só podem sair pelos valores devolvidos ao chamador.
    /// Não há publicação em global/campo, retenção, callback ou throw no corpo.
    ResultOnly,
    /// Cobertura insuficiente de qualquer saída/operação, incluindo implícitas.
    UnknownValues,
}

/// Resumo de uma versão local do corpo, separado de alcance e efeitos.tsv.
/// Só o extrator constrói seus campos. Aliases são de retorno normal;
/// preservação de campos e publicação pelo resultado exigem cobertura própria
/// do corpo inteiro. Demais corpos conservam efeitos desconhecidos.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::analise::resumos::*};
/// let f = Function { symbol: "id".into(), name: "id".into(), depuracao: None,
/// params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
/// blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
/// terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
/// let resumo = extrair(&f, 8)?;
/// assert!(matches!(resumo.retorno(), RetornoHeapArc::Aliases { nulo: false, .. }));
/// # Ok::<(), String>(())
/// ```
#[derive(Debug)]
pub struct ArcHeapSummary {
    body: blake3::Hash,
    ret: ArcHeapReturn,
    fields: ArcHeapFields,
    publications: ArcHeapPublications,
    pub(super) write_inclusions: super::writes::WriteInclusions,
    pub(super) return_paths: super::writes::ReturnPaths,
    pub(super) publication_inclusions: super::writes::PublicationInclusions,
}

impl ArcHeapSummary {
    /// Consulta publicações sem confundir alias normal com ausência de retenção.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, optimize::arc::analise::resumos::*};
    /// let f = Function { symbol: "id".into(), name: "id".into(), depuracao: None,
    /// params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
    /// blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
    /// terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
    /// assert_eq!(extrair(&f, 8)?.publicacoes(), PublicacoesHeapArc::SomenteResultado);
    /// # Ok::<(), String>(())
    /// ```
    pub fn publications(&self) -> ArcHeapPublications {
        self.publications
    }

    /// Consulta preservação de campos; isso não fornece noescape ou vida.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, optimize::arc::analise::resumos::*};
    /// let f = Function { symbol: "id".into(), name: "id".into(), depuracao: None,
    /// params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
    /// blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
    /// terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
    /// assert_eq!(extrair(&f, 8)?.campos(), CamposHeapArc::Preservados);
    /// # Ok::<(), String>(())
    /// ```
    pub fn fields(&self) -> ArcHeapFields {
        self.fields
    }

    pub(super) fn invalidate_coverage(&mut self) {
        self.fields = ArcHeapFields::UnknownSet;
        self.publications = ArcHeapPublications::UnknownValues;
    }

    pub(super) fn mark_coverage(&mut self) {
        self.fields = ArcHeapFields::Preserved;
        self.publications = ArcHeapPublications::ResultOnly;
    }

    /// Consulta aliases normais; isso não fornece noescape ou noheapmutation.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, optimize::arc::analise::resumos::*};
    /// let f = Function { symbol: "n".into(), name: "n".into(), depuracao: None,
    /// params: vec![], return_ty: Type::Ref, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) }] };
    /// assert!(matches!(extrair(&f, 8)?.retorno(), RetornoHeapArc::Aliases { nulo: true, .. }));
    /// # Ok::<(), String>(())
    /// ```
    pub fn ret(&self) -> &ArcHeapReturn {
        &self.ret
    }

    /// Recusa consumo após alteração do corpo. Guarda local, sem validade SDK/JIT.
    ///
    /// # Erros
    /// Assinatura local diferente da versão resumida.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, optimize::arc::analise::resumos::*};
    /// let f = Function { symbol: "v".into(), name: "v".into(), depuracao: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
    /// extrair(&f, 8)?.conferir(&f)?;
    /// # Ok::<(), String>(())
    /// ```
    pub fn check(&self, f: &Function) -> Result<(), String> {
        if self.body == body_signature(f) {
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
    /// use dartforge_emit_native::{hir::*, optimize::arc::analise::{resumos::*, modelo::*}};
    /// let f = Function { symbol: "id".into(), name: "id".into(), depuracao: None,
    /// params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
    /// blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
    /// terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
    /// let mut arg = ConjuntoPontos::vazio(8); arg.incluir(NoAbstrato::Global("raiz".into()));
    /// assert_eq!(extrair(&f, 8)?.instanciar(&f, &[arg.clone()], 8)?, arg);
    /// # Ok::<(), String>(())
    /// ```
    pub fn instantiate(
        &self,
        f: &Function,
        arguments: &[PointSet],
        limit: usize,
    ) -> Result<PointSet, String> {
        self.check(f)?;
        if arguments.len() != f.params.len() {
            return Err("resumo heap ARC: aridade incompatível".into());
        }
        let ArcHeapReturn::Aliases { parameters, .. } = &self.ret else {
            return Ok(PointSet::unknown(limit));
        };
        let mut ret = PointSet::empty(limit);
        for &p in parameters {
            ret.join_set(&arguments[p]);
        }
        Ok(ret)
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
/// use dartforge_emit_native::{hir::*, optimize::arc::analise::resumos::*};
/// let f = Function { symbol: "v".into(), name: "v".into(), depuracao: None,
/// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
/// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
/// assert_eq!(extrair(&f, 8)?.retorno(), &RetornoHeapArc::Desconhecido);
/// # Ok::<(), String>(())
/// ```
pub fn extract(f: &Function, limit: usize) -> Result<ArcHeapSummary, String> {
    extract_with_calls(f, limit, false, |_| None)
}

// Vazio sem null é bottom somente durante a solução interprocedural.
// O solver o eleva a desconhecido antes de disponibilizar o resultado.
pub(super) fn extract_with_calls<'a>(
    f: &Function,
    limit: usize,
    internal: bool,
    call: impl Fn(&str) -> Option<(&'a Function, &'a ArcHeapReturn)>,
) -> Result<ArcHeapSummary, String> {
    super::super::ssa::verify(f)?;
    let types: HashMap<_, _> = f
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
    let mut values: Vec<_> = types.keys().copied().collect();
    values.sort_by_key(|v| v.0);
    let ids: HashMap<_, _> = values.iter().enumerate().map(|(i, v)| (*v, i)).collect();
    let null_val = ids.len();
    let opaque = null_val + 1;
    let mut rs = vec![Constraint::Unknown { dest: opaque }];
    let op = |o: &Operand| match o {
        Operand::Val(v) => ids[v],
        Operand::Constant(Constant::Null) => null_val,
        _ => opaque,
    };
    for (index, (v, _, t)) in f.params.iter().enumerate() {
        if *t == Type::Ref {
            rs.push(Constraint::Seed {
                dest: ids[v],
                node: AbstractNode::Parameter {
                    function: f.symbol.clone(),
                    index: u32::try_from(index).map_err(|_| "resumo: parâmetros demais")?,
                },
            });
        } else {
            rs.push(Constraint::Unknown { dest: ids[v] });
        }
    }
    let mut copies = Vec::new();
    for (v, i, t) in f.blocks.iter().flat_map(|b| &b.instructions) {
        let dest = ids[v];
        match i {
            Instruction::Const(Constant::Null) => copies.push((null_val, dest)),
            Instruction::ArcCopy { value } | Instruction::ArcMove { value } if *t == Type::Ref => {
                copies.push((op(value), dest))
            }
            Instruction::Bitcast {
                op: Operand::Val(origin),
                ..
            } if *t == Type::Ref && types[origin] == Type::Ref => {
                copies.push((ids[origin], dest))
            }
            Instruction::Phi { incoming, .. } if *t == Type::Ref => {
                for (_, o) in incoming {
                    copies.push((op(o), dest));
                }
            }
            Instruction::CallStatic {
                symbol,
                args,
                ret_ty,
            } if *t == Type::Ref && *ret_ty == Type::Ref => match call(symbol) {
                Some((
                    callee,
                    ArcHeapReturn::Aliases {
                        parameters,
                        null,
                    },
                )) => {
                    if args.len() != callee.params.len() {
                        return Err(format!("resumo: aridade de {symbol}"));
                    }
                    for &p in parameters {
                        if let Operand::Val(v) = &args[p]
                            && types[v] != Type::Ref
                        {
                            return Err(format!("resumo: argumento Ref de {symbol}"));
                        }
                        copies.push((op(&args[p]), dest));
                    }
                    if *null {
                        copies.push((null_val, dest));
                    }
                }
                _ => rs.push(Constraint::Unknown { dest }),
            },
            _ => rs.push(Constraint::Unknown { dest }),
        }
    }
    for &(origin, dest) in &copies {
        rs.push(Constraint::Copy { origin, dest });
    }
    let points = solve(ids.len() + 2, limit, &rs)?;
    // Nulabilidade tem worklist própria e não cria um nó físico para null.
    let mut dependents = vec![Vec::new(); ids.len() + 2];
    for (origin, dest) in copies {
        dependents[origin].push(dest);
    }
    let mut nulls = vec![false; ids.len() + 2];
    nulls[null_val] = true;
    let mut queue = VecDeque::from([null_val]);
    while let Some(v) = queue.pop_front() {
        for &dest in &dependents[v] {
            if !nulls[dest] {
                nulls[dest] = true;
                queue.push_back(dest);
            }
        }
    }
    let mut parameters = BTreeSet::new();
    let mut may_be_null = false;
    let mut normal = false;
    let mut unknown = f.return_ty != Type::Ref;
    for b in &f.blocks {
        if let Terminator::Return(Some(o)) = &b.terminator {
            normal = true;
            let id = op(o);
            may_be_null |= nulls[id];
            match points.variable(id).unwrap().nodes() {
                None => unknown = true,
                Some(nodes) => {
                    for node in nodes {
                        if let AbstractNode::Parameter { index, .. } = node {
                            parameters.insert(*index as usize);
                        } else {
                            unknown = true;
                        }
                    }
                }
            }
        }
    }
    let ret = if unknown
        || parameters.len() > limit
        || (!internal && (!normal || (parameters.is_empty() && !may_be_null)))
    {
        ArcHeapReturn::UnknownValue
    } else {
        ArcHeapReturn::Aliases {
            parameters,
            null: may_be_null,
        }
    };
    let covered = value_coverage(f, &types, |_| None);
    let (return_paths, write_inclusions, publication_inclusions) =
        super::writes::extract_with_effects(f, limit, internal, |_| None)?;
    Ok(ArcHeapSummary {
        body: body_signature(f),
        ret,
        write_inclusions,
        return_paths,
        publication_inclusions,
        fields: if covered {
            ArcHeapFields::Preserved
        } else {
            ArcHeapFields::UnknownSet
        },
        publications: if covered {
            ArcHeapPublications::ResultOnly
        } else {
            ArcHeapPublications::UnknownValues
        },
    })
}

// Lista positiva independente dos aliases normais. Excluir chamadas opacas, throws,
// stores, alocações, constantes de texto e ARC que possa exigir contexto:
// conferência implícita de pilha pode construir erro pelo SDK/reentrar.
fn value_coverage<'a>(
    f: &Function,
    types: &HashMap<ValueId, Type>,
    call: impl Fn(&str) -> Option<(&'a Function, &'a ArcHeapSummary)>,
) -> bool {
    let exact = |o: &Operand, t: Type| exact_argument(o, t, types);
    let covered_body = f.blocks.iter().all(|b| {
        let exit = match &b.terminator {
            Terminator::Return(Some(o)) => exact(o, f.return_ty),
            Terminator::Return(None) => f.return_ty == Type::Void,
            Terminator::CondBranch { cond, .. } => exact(cond, Type::I1),
            Terminator::Switch { val, .. } => exact(val, Type::I64),
            Terminator::Branch(_) | Terminator::Unreachable => true,
            Terminator::Throw(_) => false,
        };
        exit
            && b.instructions.iter().all(|(_, i, t)| match i {
                Instruction::Const(c) => exact(&Operand::Constant(c.clone()), *t),
                Instruction::Phi { incoming, ty } => {
                    ty == t && incoming.iter().all(|(_, o)| exact(o, *t))
                }
                Instruction::CallStatic {
                    symbol,
                    args,
                    ret_ty,
                } => call(symbol).is_some_and(|(callee, resumo)| {
                    resumo.fields == ArcHeapFields::Preserved
                        && resumo.publications == ArcHeapPublications::ResultOnly
                        && *t == *ret_ty
                        && *ret_ty == callee.return_ty
                        && args.len() == callee.params.len()
                        && args
                            .iter()
                            .zip(&callee.params)
                            .all(|(o, (_, _, ty))| exact(o, *ty))
                }),
                i => super::hir::pure_scalar(i, *t, types),
            })
    });
    covered_body
}

// Cobertura composta exige todos os callees já cobertos; desconhecimento
// não é hipótese coindutiva para aprovar uma SCC recursiva sem uma base.
pub(super) fn coverage_with_calls<'a>(
    f: &Function,
    call: impl Fn(&str) -> Option<(&'a Function, &'a ArcHeapSummary)>,
) -> bool {
    let types = f
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
    value_coverage(f, &types, call)
}

// Igualdade física não permite converter I64 em Ref ou materializar texto.
pub(super) fn exact_argument(o: &Operand, t: Type, types: &HashMap<ValueId, Type>) -> bool {
    match o {
        Operand::Val(v) => types.get(v) == Some(&t),
        Operand::Constant(Constant::Null) => t == Type::Ref,
        Operand::Constant(Constant::Int(_)) => t == Type::I64,
        Operand::Constant(Constant::Double(_)) => t == Type::F64,
        Operand::Constant(Constant::Bool(_)) => t == Type::I1,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn identity() -> Function {
        Function {
            symbol: "id".into(),
            name: "id".into(),
            debug_info: None,
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
    fn scalar_coverage_requires_exact_operation_and_representations() {
        let x = Operand::Val(ValueId(0));
        let y = Operand::Val(ValueId(1));
        let cases = vec![
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
        for (i, entry, exit) in cases {
            let mut f = Function {
                symbol: "calculo".into(),
                name: "calculo".into(),
                debug_info: None,
                params: vec![
                    (ValueId(0), "x".into(), entry),
                    (ValueId(1), "y".into(), entry),
                ],
                return_ty: exit,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(ValueId(2), i.clone(), exit)],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(2)))),
                }],
            };
            let r = extract(&f, 8).unwrap();
            assert_eq!(r.fields(), ArcHeapFields::Preserved, "{i:?}");
            assert_eq!(
                r.publications(),
                ArcHeapPublications::ResultOnly,
                "{i:?}"
            );
            // Mesmo um resultado escalar não fornece alias Ref nem frescor.
            assert_eq!(r.ret(), &ArcHeapReturn::UnknownValue);
            let mut m = Module::new();
            m.arc_memory = true;
            m.functions.push(f.clone());
            let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
            // O módulo também contém portas/registro do runtime; a obrigação
            // de ausência de chamada vale para o corpo resumido, não para elas.
            let body: Vec<_> = ir
                .lines()
                .skip_while(|l| !(l.starts_with("define ") && l.contains("@calculo(")))
                .take_while(|l| *l != "}")
                .collect();
            assert!(!body.is_empty(), "corpo não emitido: {i:?}");
            assert!(!body.iter().any(|l| l.contains("pilha.estouro:")), "{i:?}");
            assert!(
                !body
                    .iter()
                    .any(|l| l.trim_start().starts_with("call ") || l.contains(" = call ")),
                "{i:?}"
            );
            f.params[0].2 = Type::Ref;
            let r = extract(&f, 8).unwrap();
            assert_eq!(r.fields(), ArcHeapFields::UnknownSet, "entrada {i:?}");
            assert_eq!(r.publications(), ArcHeapPublications::UnknownValues);
            f.params[0].2 = entry;
            f.blocks[0].instructions[0].2 = Type::Ref;
            f.return_ty = Type::Ref;
            assert_eq!(
                extract(&f, 8).unwrap().fields(),
                ArcHeapFields::UnknownSet,
                "saída {i:?}"
            );
        }
        for (i, entry) in [
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
            let mut f = identity();
            f.params[0].2 = entry;
            f.params.push((ValueId(1), "y".into(), entry));
            f.return_ty = Type::I64;
            f.blocks[0]
                .instructions
                .push((ValueId(2), i.clone(), Type::I64));
            f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(2))));
            assert_eq!(
                extract(&f, 8).unwrap().fields(),
                ArcHeapFields::UnknownSet,
                "falível {i:?}"
            );
        }
    }

    #[test]
    fn field_coverage_does_not_derive_from_alias_and_includes_exceptional_exits() {
        let mut f = identity();
        assert_eq!(extract(&f, 8).unwrap().fields(), ArcHeapFields::Preserved);
        f.blocks[0].terminator = Terminator::Throw(Operand::Val(ValueId(0)));
        assert_eq!(
            extract(&f, 8).unwrap().fields(),
            ArcHeapFields::UnknownSet
        );
        f = identity();
        f.blocks[0].instructions.push((
            ValueId(1),
            Instruction::ArcCopy {
                value: Operand::Val(ValueId(0)),
            },
            Type::Ref,
        ));
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
        let resumo = extract(&f, 8).unwrap();
        assert!(matches!(resumo.ret(), ArcHeapReturn::Aliases { .. }));
        assert_eq!(resumo.fields(), ArcHeapFields::UnknownSet);
        assert_eq!(resumo.publications(), ArcHeapPublications::UnknownValues);
        // Uma constante de texto aloca no JIT, mesmo sem chamada HIR explícita.
        f = identity();
        f.blocks[0].instructions.push((
            ValueId(1),
            Instruction::Const(Constant::String("texto".into())),
            Type::Ref,
        ));
        assert_eq!(
            extract(&f, 8).unwrap().fields(),
            ArcHeapFields::UnknownSet
        );
    }

    #[test]
    fn identity_preserves_placeholder_and_rejects_stale_version() {
        let mut f = identity();
        let r = extract(&f, 8).unwrap();
        assert_eq!(
            r.ret(),
            &ArcHeapReturn::Aliases {
                parameters: BTreeSet::from([0]),
                null: false
            }
        );
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Constant(Constant::Null)));
        assert!(r.check(&f).is_err());
        assert_eq!(
            extract(&f, 8).unwrap().ret(),
            &ArcHeapReturn::Aliases {
                parameters: BTreeSet::new(),
                null: true
            }
        );
    }
    #[test]
    fn opaque_producer_width_and_budget_do_not_forge_parameter_alias() {
        let mut f = identity();
        assert_eq!(
            extract(&f, 0).unwrap().ret(),
            &ArcHeapReturn::UnknownValue
        );
        f.params[0].2 = Type::I64;
        f.return_ty = Type::I64;
        assert_eq!(
            extract(&f, 8).unwrap().ret(),
            &ArcHeapReturn::UnknownValue
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
            extract(&f, 8).unwrap().ret(),
            &ArcHeapReturn::UnknownValue
        );
    }

    #[test]
    fn instantiation_preserves_caller_aliases_and_loses_precision_by_budget() {
        let f = identity();
        let r = extract(&f, 8).unwrap();
        let mut arg = PointSet::empty(8);
        arg.include(AbstractNode::Global("a".into()));
        arg.include(AbstractNode::Global("b".into()));
        assert_eq!(r.instantiate(&f, &[arg.clone()], 8).unwrap(), arg);
        assert!(r.instantiate(&f, &[arg], 1).unwrap().nodes().is_none());
        assert!(
            r.instantiate(&f, &[PointSet::unknown(8)], 8)
                .unwrap()
                .nodes()
                .is_none()
        );
        assert!(r.instantiate(&f, &[], 8).is_err());
    }

    #[test]
    fn phi_joins_parameters_and_null_without_turning_placeholder_into_fresh_object() {
        let mut f = identity();
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
            extract(&f, 8).unwrap().ret(),
            &ArcHeapReturn::Aliases {
                parameters: BTreeSet::from([0]),
                null: true
            }
        );
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[3].instructions[0].1 {
            incoming[1].1 = Operand::Val(ValueId(1));
        }
        assert_eq!(
            extract(&f, 8).unwrap().ret(),
            &ArcHeapReturn::Aliases {
                parameters: BTreeSet::from([0, 1]),
                null: false
            }
        );
    }
}
