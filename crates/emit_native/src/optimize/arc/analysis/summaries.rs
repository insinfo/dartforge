//! Initial summaries of normal-return aliases (§28.3).
//! Field preservation by positive coverage of bodies and direct calls.
//! Publication only through the result requires positive coverage of the whole body.
//! They do not certify policies, lifetimes or complete effects of callbacks/SDK.

use super::{hir::body_signature, model::*, points_to::*};
use crate::hir::*;
use std::collections::{BTreeSet, HashMap, VecDeque};

/// Parametric normal result; parameters are replaced by the real aliases
/// of the caller, never by independent fresh objects.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analysis::summaries::ArcHeapReturn;
/// assert!(matches!(ArcHeapReturn::UnknownValue, ArcHeapReturn::UnknownValue));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArcHeapReturn {
    /// A normal return may coincide with any listed parameter; `null`
    /// records an additional literal null. Parameters may be null even with
    /// that mark false: it does not prove the non-nullability of the arguments.
    Aliases {
        parameters: BTreeSet<usize>,
        null: bool,
    },
    /// Insufficient precision, opaque representation or exceeded budget.
    UnknownValue,
}

/// Conservative effect on existing fields, at every exit of the body.
/// Does not cover lifetime, retention, publication or the Owned/Borrowed convention.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analysis::summaries::ArcHeapFields;
/// assert_ne!(ArcHeapFields::Preserved, ArcHeapFields::UnknownSet);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcHeapFields {
    /// A covered body preserves fields, including callees with the same coverage.
    Preserved,
    /// A possible write or an operation/implicit path not covered yet.
    UnknownSet,
}

/// Possible publications of the body, separate from the aliases of the result.
/// Consumption requires checking the body, the ABI and the implicit emission paths.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analysis::summaries::ArcHeapPublications;
/// assert_ne!(ArcHeapPublications::ResultOnly, ArcHeapPublications::UnknownValues);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcHeapPublications {
    /// References can only leave through the values returned to the caller.
    /// There is no publication into a global/field, retention, callback or throw in the body.
    ResultOnly,
    /// Insufficient coverage of any exit/operation, implicit ones included.
    UnknownValues,
}

/// Summary of a local version of the body, separate from reach and effects.tsv.
/// Only the extractor builds its fields. Aliases are of the normal return;
/// field preservation and publication through the result require coverage of the
/// whole body. Other bodies keep unknown effects.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::analysis::summaries::*};
/// let f = Function { symbol: "id".into(), name: "id".into(), debug_info: None,
/// params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
/// blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
/// terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
/// let summary = extract(&f, 8)?;
/// assert!(matches!(summary.ret(), ArcHeapReturn::Aliases { null: false, .. }));
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
    /// Queries publications without confusing a normal alias with absence of retention.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, optimize::arc::analysis::summaries::*};
    /// let f = Function { symbol: "id".into(), name: "id".into(), debug_info: None,
    /// params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
    /// blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
    /// terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
    /// assert_eq!(extract(&f, 8)?.publications(), ArcHeapPublications::ResultOnly);
    /// # Ok::<(), String>(())
    /// ```
    pub fn publications(&self) -> ArcHeapPublications {
        self.publications
    }

    /// Queries field preservation; this provides neither noescape nor lifetime.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, optimize::arc::analysis::summaries::*};
    /// let f = Function { symbol: "id".into(), name: "id".into(), debug_info: None,
    /// params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
    /// blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
    /// terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
    /// assert_eq!(extract(&f, 8)?.fields(), ArcHeapFields::Preserved);
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

    /// Queries normal aliases; this provides neither noescape nor noheapmutation.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, optimize::arc::analysis::summaries::*};
    /// let f = Function { symbol: "n".into(), name: "n".into(), debug_info: None,
    /// params: vec![], return_ty: Type::Ref, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) }] };
    /// assert!(matches!(extract(&f, 8)?.ret(), ArcHeapReturn::Aliases { null: true, .. }));
    /// # Ok::<(), String>(())
    /// ```
    pub fn ret(&self) -> &ArcHeapReturn {
        &self.ret
    }

    /// Rejects consumption after a change of the body. A local guard, without SDK/JIT validity.
    ///
    /// # Errors
    /// A local signature different from the summarized version.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, optimize::arc::analysis::summaries::*};
    /// let f = Function { symbol: "v".into(), name: "v".into(), debug_info: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
    /// extract(&f, 8)?.check(&f)?;
    /// # Ok::<(), String>(())
    /// ```
    pub fn check(&self, f: &Function) -> Result<(), String> {
        if self.body == body_signature(f) {
            Ok(())
        } else {
            Err("ARC heap summary: body changed".into())
        }
    }

    /// Replaces placeholders with the real sets of the arguments, without freshness.
    /// Null creates no node; an empty set does not certify the absence of effects.
    /// An exceeded budget/unknown argument propagates top. It is still a
    /// normal result, without eliminating heap effects or escapes of the call.
    ///
    /// # Errors
    /// A stale body or a number of arguments different from the summarized ABI.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, optimize::arc::analysis::{summaries::*, model::*}};
    /// let f = Function { symbol: "id".into(), name: "id".into(), debug_info: None,
    /// params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
    /// blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
    /// terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
    /// let mut arg = PointSet::empty(8); arg.include(AbstractNode::Global("root".into()));
    /// assert_eq!(extract(&f, 8)?.instantiate(&f, &[arg.clone()], 8)?, arg);
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
            return Err("ARC heap summary: incompatible arity".into());
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

/// Extracts the inclusion of parameter/null aliases at the normal Ref exits.
/// Copy/Move/Phi and a Ref→Ref bitcast propagate aliases; other producers become
/// unknown. Placeholders serve only the summary, they are not allocations.
/// The summary does not eliminate effects nor change the Owned/Borrowed contract of the ABI.
///
/// # Errors
/// Invalid SSA or a number of parameters outside the index domain.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::analysis::summaries::*};
/// let f = Function { symbol: "v".into(), name: "v".into(), debug_info: None,
/// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
/// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
/// assert_eq!(extract(&f, 8)?.ret(), &ArcHeapReturn::UnknownValue);
/// # Ok::<(), String>(())
/// ```
pub fn extract(f: &Function, limit: usize) -> Result<ArcHeapSummary, String> {
    extract_with_calls(f, limit, false, |_| None)
}

// Empty without null is bottom only during the interprocedural solution.
// The solver raises it to unknown before making the result available.
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
                    index: u32::try_from(index).map_err(|_| "summary: too many parameters")?,
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
                        return Err(format!("summary: arity of {symbol}"));
                    }
                    for &p in parameters {
                        if let Operand::Val(v) = &args[p]
                            && types[v] != Type::Ref
                        {
                            return Err(format!("summary: Ref argument of {symbol}"));
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
    // Nullability has its own worklist and creates no physical node for null.
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

// A positive list independent of the normal aliases. Exclude opaque calls, throws,
// stores, allocations, string constants and ARC that may require a context:
// the implicit stack check may build an error through the SDK/reenter.
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
                } => call(symbol).is_some_and(|(callee, summary)| {
                    summary.fields == ArcHeapFields::Preserved
                        && summary.publications == ArcHeapPublications::ResultOnly
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

// Composite coverage requires every callee already covered; lack of knowledge
// is not a coinductive hypothesis to approve a recursive SCC without a base.
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

// Physical equality does not allow converting I64 into Ref or materializing a string.
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
            // Even a scalar result provides neither a Ref alias nor freshness.
            assert_eq!(r.ret(), &ArcHeapReturn::UnknownValue);
            let mut m = Module::new();
            m.arc_memory = true;
            m.functions.push(f.clone());
            let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
            // The module also contains runtime ports/registration; the obligation
            // of absence of calls holds for the summarized body, not for them.
            let body: Vec<_> = ir
                .lines()
                .skip_while(|l| !(l.starts_with("define ") && l.contains("@calculo(")))
                .take_while(|l| *l != "}")
                .collect();
            assert!(!body.is_empty(), "body not emitted: {i:?}");
            assert!(!body.iter().any(|l| l.contains("pilha.estouro:")), "{i:?}");
            assert!(
                !body
                    .iter()
                    .any(|l| l.trim_start().starts_with("call ") || l.contains(" = call ")),
                "{i:?}"
            );
            f.params[0].2 = Type::Ref;
            let r = extract(&f, 8).unwrap();
            assert_eq!(r.fields(), ArcHeapFields::UnknownSet, "entry {i:?}");
            assert_eq!(r.publications(), ArcHeapPublications::UnknownValues);
            f.params[0].2 = entry;
            f.blocks[0].instructions[0].2 = Type::Ref;
            f.return_ty = Type::Ref;
            assert_eq!(
                extract(&f, 8).unwrap().fields(),
                ArcHeapFields::UnknownSet,
                "exit {i:?}"
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
                "fallible {i:?}"
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
        let summary = extract(&f, 8).unwrap();
        assert!(matches!(summary.ret(), ArcHeapReturn::Aliases { .. }));
        assert_eq!(summary.fields(), ArcHeapFields::UnknownSet);
        assert_eq!(summary.publications(), ArcHeapPublications::UnknownValues);
        // A string constant allocates in the JIT, even without an explicit HIR call.
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
