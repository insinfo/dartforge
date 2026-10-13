//! Conservative extraction of constraints from the HIR after SSA (§28.2).
//! Nominal allocation/layout facts are explicit premises of the producer.
//! Exports no certificates, SDK summaries or memory policy.

use super::{model::*, points_to::*};
use crate::hir::*;
use std::collections::{BTreeSet, HashMap};

/// Origin/schema of the fields of AllocObject or of the reserved zeroed factory.
/// The producer must preserve origin/context in the clones and check layouts.
/// SSA indices are not used as the persistent identity of the abstract node.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analysis::{hir::ObjectOrigin, model::*, points_to::*};
/// let origin = ObjectOrigin {
///     node: AbstractNode::Allocation { site: ArcSite { function: "f".into(), origin: 1, specialization: "".into() }, context: "".into() },
///     fields: vec![ArcField { layout: "No:v1".into(), position: 0 }],
/// };
/// assert_eq!(origin.fields.len(), 1);
/// ```
#[derive(Debug, Clone)]
pub struct ObjectOrigin {
    /// Nominal node produced for the allocation, never a parameter placeholder.
    pub node: AbstractNode,
    /// Fields in the physical positions of the initializer, trivial fields included.
    pub fields: Vec<ArcField>,
}

/// Nominal facts of the same version of the function/layout, still supplied explicitly.
/// Missing facts cause lack of knowledge; stale/incompatible facts are errors.
///
/// ```
/// use dartforge_emit_native::optimize::arc::analysis::hir::HirFacts;
/// assert!(HirFacts::default().objects.is_empty());
/// ```
#[derive(Debug, Default)]
pub struct HirFacts {
    /// Origin/schema of each object allocation identified by the producer.
    pub objects: HashMap<ValueId, ObjectOrigin>,
    /// Nominal field of each GetField/SetField, identified by the SSA result.
    pub accesses: HashMap<ValueId, ArcField>,
}

// Mirrors only the physical reference slots of the synthetic units.
// None is a scalar/address slot, not a proven null reference.
pub(super) fn synthetic_captures(
    i: &Instruction,
    types: &HashMap<ValueId, Type>,
) -> Option<(&'static str, Vec<Option<Operand>>)> {
    let reference =
        |o: &Operand| super::summaries::exact_argument(o, Type::Ref, types).then(|| o.clone());
    match i {
        Instruction::AllocCell { value } => Some(("celula:v1", vec![reference(value)])),
        Instruction::AllocEnv { values } if !values.is_empty() => {
            Some(("ambiente:v1", values.iter().map(reference).collect()))
        }
        Instruction::AllocClosure { env, .. } | Instruction::AllocClosureTyped { env, .. }
            if reference(env).is_some() =>
        {
            Some(("closure:v1", vec![None, reference(env), None, None]))
        }
        _ => None,
    }
}

pub(super) fn field_read(i: &Instruction) -> Option<(&Operand, usize)> {
    match i {
        Instruction::GetField { object, index } => Some((object, *index)),
        Instruction::EnvGet { env, index } => Some((env, *index)),
        Instruction::CellGet { cell } => Some((cell, 0)),
        _ => None,
    }
}

pub(super) fn field_write(i: &Instruction) -> Option<(&Operand, usize, &Operand)> {
    match i {
        Instruction::SetField {
            object,
            index,
            value,
        } => Some((object, *index, value)),
        Instruction::CellSet { cell, value } => Some((cell, 0, value)),
        _ => None,
    }
}

// The Dart attachment keeps the action as a strong edge of the owner, outside the fields.
// Recognizing this inclusion does not cover weak targets, reentry or safepoints.
pub(super) fn dart_attachment<'a>(
    i: &'a Instruction,
    ty: Type,
    types: &HashMap<ValueId, Type>,
) -> Option<(&'a Operand, &'a Operand)> {
    let Instruction::CallRuntime { name, args, ret_ty } = i else {
        return None;
    };
    if name != "dartforge_native_DartForge_finalizer_attach"
        || ty != Type::Void
        || *ret_ty != Type::Void
        || args.len() != 4
        || !args
            .iter()
            .all(|(o, t)| *t == Type::Ref && super::summaries::exact_argument(o, Type::Ref, types))
    {
        return None;
    }
    Some((&args[0].0, &args[2].0))
}

/// Immutable local result; unknown IDs identify operations without a precise transfer.
/// Does not certify full program coverage, escape, observers or singleton.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::analysis::hir::*};
/// let f = Function { symbol: "f".into(), name: "f".into(), debug_info: None,
/// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
/// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
/// assert!(analyze(&f, &HirFacts::default(), 8)?.unknown_set().is_empty());
/// # Ok::<(), String>(())
/// ```
#[derive(Debug)]
pub struct HirAnalysis {
    /// Solution of the extracted system, with explicit lack of knowledge.
    points: PointsToResult,
    /// Operations without a precise heap contract; do not confuse with absence of effect.
    unknown_set: Vec<ValueId>,
    ids: HashMap<ValueId, usize>,
    pub(crate) body: blake3::Hash,
    pub(crate) body_dependencies: HashMap<String, blake3::Hash>,
    pub(crate) table_dependencies: HashMap<String, ArcTableState>,
    pub(crate) module_premises: Option<blake3::Hash>,
    // Paths inserted by the emitter that have no heap summary yet.
    pub(crate) opaque_implicit_emission: bool,
    pub(crate) limit: usize,
    pub(crate) schemas: HashMap<AbstractNode, BTreeSet<ArcField>>,
    // Inclusions through real call arguments; they do not refine top fields nor
    // replace the coverage of mutations/lifetimes/emission paths.
    pub(crate) write_inclusions: Vec<(usize, usize)>,
    pub(crate) publication_inclusions: Vec<(super::escape::EscapeCause, usize)>,
}

impl HirAnalysis {
    /// Queries the solution without allowing sets to be replaced or refined.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, optimize::arc::analysis::hir::*};
    /// let f = Function { symbol: "f".into(), name: "f".into(), debug_info: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
    /// let a = analyze(&f, &HirFacts::default(), 8)?;
    /// assert!(a.points().variable(0).unwrap().nodes().unwrap().is_empty());
    /// # Ok::<(), String>(())
    /// ```
    pub fn points(&self) -> &PointsToResult {
        &self.points
    }

    /// Queries opaque operations without allowing evidence of effects to be erased.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, optimize::arc::analysis::hir::*};
    /// let f = Function { symbol: "f".into(), name: "f".into(), debug_info: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
    /// assert!(analyze(&f, &HirFacts::default(), 8)?.unknown_set().is_empty());
    /// # Ok::<(), String>(())
    /// ```
    ///
    /// ```compile_fail
    /// use dartforge_emit_native::{hir::*, optimize::arc::analysis::hir::*};
    /// let f = Function { symbol: "f".into(), name: "f".into(), debug_info: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
    /// let mut a = analyze(&f, &HirFacts::default(), 8).unwrap();
    /// a.unknown_set().clear();
    /// ```
    pub fn unknown_set(&self) -> &[ValueId] {
        &self.unknown_set
    }

    /// Queries the aliases of an SSA value, or None if the ID does not exist.
    ///
    /// ```
    /// use dartforge_emit_native::{hir::*, optimize::arc::analysis::hir::*};
    /// let f = Function { symbol: "f".into(), name: "f".into(), debug_info: None,
    /// params: vec![], return_ty: Type::Void, blocks: vec![BasicBlock {
    /// id: BlockId(0), instructions: vec![], terminator: Terminator::Return(None) }] };
    /// assert!(analyze(&f, &HirFacts::default(), 8)?.value(ValueId(0)).is_none());
    /// # Ok::<(), String>(())
    /// ```
    pub fn value(&self, id: ValueId) -> Option<&PointSet> {
        self.ids.get(&id).and_then(|&i| self.points.variable(i))
    }
}

/// Extracts and solves alias inclusion from the structurally verified HIR.
/// Ref/I64/Ptr parameters are unknown without a caller summary; an I64
/// width does not prove a trivial value. Null and F64/I1/I8 constants generate no nodes;
/// integer constants used as handles/addresses stay unknown.
/// Uncovered calls/operations invalidate every supplied nominal field;
/// opaque results are top. Known stores are always inclusive.
/// Bitcast only keeps aliases if origin and result are Ref.
///
/// # Errors
/// Invalid SSA or a stale fact/schema incompatible with the initializer/access.
/// The function and the facts are not modified.
///
/// ```
/// use dartforge_emit_native::{hir::*, optimize::arc::analysis::hir::*};
/// let f = Function { symbol: "id".into(), name: "id".into(), debug_info: None,
/// params: vec![(ValueId(0), "x".into(), Type::Ref)], return_ty: Type::Ref,
/// blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![],
/// terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] };
/// assert!(analyze(&f, &HirFacts::default(), 8)?.value(ValueId(0)).unwrap().nodes().is_none());
/// # Ok::<(), String>(())
/// ```
pub fn analyze(f: &Function, facts: &HirFacts, limit: usize) -> Result<HirAnalysis, String> {
    analyze_with_summaries(f, facts, limit, &HashMap::new(), false)
}

pub(crate) fn analyze_with_summaries(
    f: &Function,
    facts: &HirFacts,
    limit: usize,
    summaries: &HashMap<String, (&Function, super::summaries::ArcHeapSummary)>,
    select_by_schema: bool,
) -> Result<HirAnalysis, String> {
    for (sym_name, (body, summary)) in summaries {
        if sym_name != &body.symbol {
            return Err("points-to: incompatible summary symbol".into());
        }
        summary.check(body)?;
    }
    let mut body_dependencies = HashMap::new();
    super::super::ssa::verify(f)?;
    let instructions: HashMap<_, _> = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .map(|(v, i, _)| (*v, i))
        .collect();
    let constants = integer_constants(f);
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
    for (v, origin) in &facts.objects {
        let quantity = match instructions.get(v) {
            Some(Instruction::AllocObject { fields, .. }) => fields.len(),
            Some(i) => synthetic_captures(i, &types)
                .map(|(_, field_list)| field_list.len())
                .or_else(|| zeroed_factory(i, &constants).map(|(_, n)| n))
                .ok_or_else(|| format!("points-to HIR: origem obsoleta v{}", v.0))?,
            None => return Err(format!("points-to HIR: origem obsoleta v{}", v.0)),
        };
        if quantity != origin.fields.len()
            || origin
                .fields
                .iter()
                .enumerate()
                .any(|(i, c)| c.position as usize != i)
            || !matches!(
                origin.node,
                AbstractNode::Allocation { .. } | AbstractNode::SyntheticUnit { .. }
            )
        {
            return Err(format!(
                "HIR points-to: incompatible schema/origin v{}",
                v.0
            ));
        }
    }
    for (v, field) in &facts.accesses {
        let idx_value = match instructions.get(v) {
            Some(Instruction::GetField { index, .. } | Instruction::SetField { index, .. }) => {
                *index
            }
            _ => return Err(format!("HIR points-to: stale field v{}", v.0)),
        };
        if field.position as usize != idx_value {
            return Err(format!("HIR points-to: incompatible index v{}", v.0));
        }
    }
    let mut value_list: Vec<_> = types.keys().copied().collect();
    value_list.sort_by_key(|v| v.0);
    let ids: HashMap<_, _> = value_list.iter().enumerate().map(|(i, v)| (*v, i)).collect();
    let empty = ids.len();
    let opaque = empty + 1;
    let mut rs = vec![Constraint::Unknown { dest: opaque }];
    let mut write_inclusions = Vec::new();
    let mut publication_inclusions = Vec::new();
    let mut variable_count = ids.len() + 2;
    let operand = |o: &Operand| match o {
        Operand::Val(v) => ids[v],
        Operand::Constant(Constant::Null) => empty,
        _ => opaque,
    };
    for (v, _, ty) in &f.params {
        if matches!(ty, Type::Ref | Type::I64 | Type::Ptr) {
            rs.push(Constraint::Unknown { dest: ids[v] });
        }
    }
    let mut unknown_set = Vec::new();
    let mut schemas_by_index = std::collections::BTreeMap::new();
    for origin in facts.objects.values() {
        if schemas_by_index
            .insert(origin.node.clone(), origin.fields.clone())
            .is_some_and(|previous| previous != origin.fields)
        {
            return Err("HIR points-to: conflicting schemas for the same origin".into());
        }
    }
    for (_, summary) in summaries.values() {
        let mut paths = Vec::new();
        if let Some(returns) = &summary.return_paths.paths {
            paths.extend(returns.iter());
        }
        if let super::writes::PublicationInclusions::Known(publications) =
            &summary.publication_inclusions
        {
            paths.extend(publications.values().filter_map(Option::as_ref).flatten());
        }
        if let Some(writes) = summary.write_inclusions.positive_set() {
            for write in writes {
                paths.extend([&write.object, &write.value]);
            }
        }
        for path in paths {
            if let super::writes::PathOrigin::Allocation { node, fields: field_list } = &path.origin {
                if schemas_by_index
                    .insert(node.clone(), field_list.clone())
                    .is_some_and(|previous| previous != *field_list)
                {
                    return Err("HIR points-to: conflicting imported allocation schema".into());
                }
                // The identity of the result does not summarize the state of the fields
                // of the callee. Do not let a missing initialization look like null.
                rs.extend(
                    field_list
                        .iter()
                        .cloned()
                        .map(|field| Constraint::UnknownField { field }),
                );
            }
        }
    }
    let field_list: BTreeSet<_> = facts
        .accesses
        .values()
        .cloned()
        .chain(schemas_by_index.values().flatten().cloned())
        .collect();
    let schemas_by_index = std::sync::Arc::new(schemas_by_index);
    let mut selected_accesses = Vec::new();
    for (v, i, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
        let dest = ids[v];
        match i {
            Instruction::Const(Constant::String(_) | Constant::StringWtf8(_)) => {
                rs.push(Constraint::Unknown { dest });
                invalidate_fields(*v, &field_list, &mut unknown_set, &mut rs);
            }
            Instruction::Const(_) if matches!(ty, Type::F64 | Type::I1 | Type::I8) => {}
            Instruction::Const(c) => rs.push(Constraint::Copy {
                origin: operand(&Operand::Constant(c.clone())),
                dest,
            }),
            Instruction::ArcCopy { value } | Instruction::ArcMove { value } => {
                rs.push(Constraint::Copy {
                    origin: operand(value),
                    dest,
                })
            }
            Instruction::CheckNotNull(value) => {
                rs.push(Constraint::Copy {
                    origin: operand(value),
                    dest,
                });
                invalidate_fields(*v, &field_list, &mut unknown_set, &mut rs);
            }
            Instruction::Phi { incoming, .. } => {
                for (_, o) in incoming {
                    rs.push(Constraint::Copy {
                        origin: operand(o),
                        dest,
                    });
                }
                if incoming
                    .iter()
                    .any(|(_, o)| !super::summaries::exact_argument(o, *ty, &types))
                {
                    rs.push(Constraint::Unknown { dest });
                    invalidate_fields(*v, &field_list, &mut unknown_set, &mut rs);
                }
            }
            Instruction::Bitcast {
                op: Operand::Val(origin),
                ..
            } if *ty == Type::Ref && types[origin] == Type::Ref => rs.push(Constraint::Copy {
                origin: ids[origin],
                dest,
            }),
            i if facts.objects.contains_key(v) => {
                let origin = &facts.objects[v];
                rs.push(Constraint::Seed {
                    dest,
                    node: origin.node.clone(),
                });
                let initial_set = match i {
                    Instruction::AllocObject { fields, .. } => {
                        fields.iter().cloned().map(Some).collect()
                    }
                    _ => synthetic_captures(i, &types)
                        .map(|(_, field_list)| field_list)
                        .unwrap_or_else(|| {
                            vec![Some(Operand::Constant(Constant::Null)); origin.fields.len()]
                        }),
                };
                for (o, field) in initial_set.iter().zip(&origin.fields) {
                    let Some(o) = o else {
                        rs.push(Constraint::UnknownField {
                            field: field.clone(),
                        });
                        continue;
                    };
                    rs.push(Constraint::Write {
                        object: dest,
                        field: field.clone(),
                        value: operand(o),
                    });
                    if *ty == Type::Ref && super::summaries::exact_argument(o, Type::Ref, &types) {
                        // A witnessed inclusion survives opaque writers;
                        // it does not refine the complete state of the field nor its lifetime.
                        write_inclusions.push((dest, operand(o)));
                    }
                }
            }
            Instruction::AllocEnv { values } if values.is_empty() && *ty == Type::Ref => {}
            Instruction::CallStatic {
                symbol,
                args,
                ret_ty,
            } if summaries.contains_key(symbol) => {
                let (callee, summary) = &summaries[symbol];
                if args.len() != callee.params.len() {
                    return Err(format!("points-to: arity of {symbol}"));
                }
                let exact_abi = *ty == *ret_ty
                    && *ret_ty == callee.return_ty
                    && args
                        .iter()
                        .zip(&callee.params)
                        .all(|(o, (_, _, t))| super::summaries::exact_argument(o, *t, &types));
                if let Some(paths) = &summary.return_paths.paths
                    && exact_abi
                    && *ty == Type::Ref
                    && *ret_ty == Type::Ref
                {
                    for path in paths {
                        let origin = materialize_summary(
                            path,
                            args,
                            &operand,
                            &schemas_by_index,
                            &mut variable_count,
                            &mut rs,
                        )?;
                        rs.push(Constraint::Copy { origin, dest });
                    }
                } else if *ty != Type::Void {
                    rs.push(Constraint::Unknown { dest });
                }
                body_dependencies.insert(symbol.clone(), body_signature(callee));
                if exact_abi
                    && let super::writes::PublicationInclusions::Known(globals) =
                        &summary.publication_inclusions
                {
                    for (sym_name, paths) in globals {
                        if let Some(paths) = paths {
                            if paths.is_empty() {
                                publication_inclusions.push((sym_name.clone(), empty));
                            }
                            for path in paths {
                                let origin = materialize_summary(
                                    path,
                                    args,
                                    &operand,
                                    &schemas_by_index,
                                    &mut variable_count,
                                    &mut rs,
                                )?;
                                publication_inclusions.push((sym_name.clone(), origin));
                            }
                        } else {
                            publication_inclusions.push((sym_name.clone(), opaque));
                        }
                    }
                }
                if exact_abi && let Some(writes) = summary.write_inclusions.positive_set() {
                    for write in writes {
                        let obj_ref = materialize_summary(
                            &write.object,
                            args,
                            &operand,
                            &schemas_by_index,
                            &mut variable_count,
                            &mut rs,
                        )?;
                        let val_ref = materialize_summary(
                            &write.value,
                            args,
                            &operand,
                            &schemas_by_index,
                            &mut variable_count,
                            &mut rs,
                        )?;
                        if let Some(idx_value) = write.index {
                            rs.push(Constraint::WriteBySchema {
                                object: obj_ref,
                                schemas: schemas_by_index.clone(),
                                index: idx_value,
                                value: val_ref,
                            });
                        }
                        write_inclusions.push((obj_ref, val_ref));
                    }
                }
                // Field coverage is separate from the normal alias. Conversions
                // of arguments may allocate/reenter before the covered body.
                if !exact_abi
                    || summary.publications() != super::summaries::ArcHeapPublications::ResultOnly
                {
                    unknown_set.push(*v);
                }
                if !exact_abi || summary.fields() != super::summaries::ArcHeapFields::Preserved {
                    for field in &field_list {
                        rs.push(Constraint::UnknownField {
                            field: field.clone(),
                        });
                    }
                }
            }
            i if select_by_schema
                && *ty == Type::Ref
                && field_read(i).is_some_and(|(object, _)| {
                    super::summaries::exact_argument(object, Type::Ref, &types)
                }) =>
            {
                let (object, index) = field_read(i).unwrap();
                selected_accesses.push((*v, operand(object), index));
                rs.push(Constraint::ReadBySchema {
                    object: operand(object),
                    schemas: schemas_by_index.clone(),
                    index: index,
                    dest,
                });
            }
            i if select_by_schema
                && *ty == Type::Void
                && field_write(i).is_some_and(|(object, _, value)| {
                    super::summaries::exact_argument(object, Type::Ref, &types)
                        && super::summaries::exact_argument(value, Type::Ref, &types)
                }) =>
            {
                let (object, index, value) = field_write(i).unwrap();
                selected_accesses.push((*v, operand(object), index));
                rs.push(Constraint::WriteBySchema {
                    object: operand(object),
                    schemas: schemas_by_index.clone(),
                    index: index,
                    value: operand(value),
                });
                write_inclusions.push((operand(object), operand(value)));
            }
            Instruction::GetField { object, .. } if facts.accesses.contains_key(v) => {
                rs.push(Constraint::Read {
                    object: operand(object),
                    field: facts.accesses[v].clone(),
                    dest,
                })
            }
            Instruction::SetField { object, value, .. } if facts.accesses.contains_key(v) => {
                rs.push(Constraint::Write {
                    object: operand(object),
                    field: facts.accesses[v].clone(),
                    value: operand(value),
                });
                if *ty == Type::Void
                    && super::summaries::exact_argument(object, Type::Ref, &types)
                    && super::summaries::exact_argument(value, Type::Ref, &types)
                {
                    write_inclusions.push((operand(object), operand(value)));
                }
            }
            i if pure_scalar(i, *ty, &types) => {}
            _ => {
                if let Some((owner, action)) = dart_attachment(i, *ty, &types) {
                    write_inclusions.push((operand(owner), operand(action)));
                }
                unknown_set.push(*v);
                if *ty != Type::Void {
                    rs.push(Constraint::Unknown { dest });
                }
                for field in &field_list {
                    rs.push(Constraint::UnknownField {
                        field: field.clone(),
                    });
                }
            }
        }
    }
    let points = solve(variable_count, limit, &rs)?;
    for (v, obj_ref, idx_value) in selected_accesses {
        if points.variable(obj_ref).unwrap().nodes().is_none_or(|ns| {
            ns.is_empty()
                || ns.iter().any(|node| {
                    schemas_by_index
                        .get(node)
                        .and_then(|cs| cs.get(idx_value))
                        .is_none()
                })
        }) {
            unknown_set.push(v);
        }
    }
    unknown_set.sort_by_key(|v| v.0);
    unknown_set.dedup();
    let mut schemas: HashMap<_, BTreeSet<_>> = HashMap::new();
    for (node, field_list) in schemas_by_index.iter() {
        schemas
            .entry(node.clone())
            .or_default()
            .extend(field_list.iter().cloned());
    }
    Ok(HirAnalysis {
        points,
        unknown_set,
        ids,
        body: body_signature(f),
        body_dependencies,
        table_dependencies: HashMap::new(),
        module_premises: None,
        opaque_implicit_emission: false,
        limit,
        schemas,
        write_inclusions,
        publication_inclusions,
    })
}

fn materialize_summary(
    path: &super::writes::SummaryPath,
    args: &[Operand],
    operand: &impl Fn(&Operand) -> usize,
    schemas: &std::sync::Arc<std::collections::BTreeMap<AbstractNode, Vec<ArcField>>>,
    variable_count: &mut usize,
    rs: &mut Vec<Constraint>,
) -> Result<usize, String> {
    let origin = match &path.origin {
        super::writes::PathOrigin::Parameter(p) => operand(
            args.get(*p)
                .ok_or("points-to: invalid summary parameter")?,
        ),
        super::writes::PathOrigin::Allocation { node, .. } => {
            let origin = *variable_count;
            *variable_count = variable_count
                .checked_add(1)
                .ok_or("points-to: too many auxiliary variables")?;
            rs.push(Constraint::Seed {
                dest: origin,
                node: node.clone(),
            });
            origin
        }
    };
    materialize_path(origin, &path.fields, schemas, variable_count, rs)
}

// Paths use the same weak fixpoint as nominal stores and layouts,
// including effects of other calls. Do not freeze a read before
// convergence nor invent a site identity for an auxiliary variable.
fn materialize_path(
    origin: usize,
    field_list: &[usize],
    schemas: &std::sync::Arc<std::collections::BTreeMap<AbstractNode, Vec<ArcField>>>,
    variable_count: &mut usize,
    rs: &mut Vec<Constraint>,
) -> Result<usize, String> {
    let mut obj_ref = origin;
    for &idx_value in field_list {
        let dest = *variable_count;
        *variable_count = variable_count
            .checked_add(1)
            .ok_or("points-to: too many auxiliary variables")?;
        rs.push(Constraint::ReadBySchema {
            object: obj_ref,
            schemas: schemas.clone(),
            index: idx_value,
            dest,
        });
        obj_ref = dest;
    }
    Ok(obj_ref)
}

fn invalidate_fields(
    v: ValueId,
    field_list: &BTreeSet<ArcField>,
    unknown_set: &mut Vec<ValueId>,
    rs: &mut Vec<Constraint>,
) {
    unknown_set.push(v);
    rs.extend(
        field_list
            .iter()
            .cloned()
            .map(|field| Constraint::UnknownField { field }),
    );
}

// A positive list per operation and input/output representations. Do not assume
// the absence of materialization/conversion just because the result occupies I64/F64.
pub(super) fn pure_scalar(i: &Instruction, t: Type, types: &HashMap<ValueId, Type>) -> bool {
    let exact = |o: &Operand, expected| super::summaries::exact_argument(o, expected, types);
    match i {
        Instruction::Add(a, b)
        | Instruction::Sub(a, b)
        | Instruction::Mul(a, b)
        | Instruction::Shl(a, b)
        | Instruction::AShr(a, b)
        | Instruction::LShr(a, b)
        | Instruction::And(a, b)
        | Instruction::Or(a, b)
        | Instruction::Xor(a, b) => t == Type::I64 && exact(a, Type::I64) && exact(b, Type::I64),
        Instruction::FAdd(a, b)
        | Instruction::FSub(a, b)
        | Instruction::FMul(a, b)
        | Instruction::FDiv(a, b) => t == Type::F64 && exact(a, Type::F64) && exact(b, Type::F64),
        Instruction::ICmp(_, a, b) => t == Type::I1 && exact(a, Type::I64) && exact(b, Type::I64),
        Instruction::FCmp(_, a, b) => t == Type::I1 && exact(a, Type::F64) && exact(b, Type::F64),
        Instruction::Neg(a) | Instruction::Not(a) => t == Type::I64 && exact(a, Type::I64),
        Instruction::FNeg(a) => t == Type::F64 && exact(a, Type::F64),
        Instruction::LNot(a) => t == Type::I1 && exact(a, Type::I1),
        Instruction::IntToDouble(a) => t == Type::F64 && exact(a, Type::I64),
        Instruction::ZExt { op, from, to } | Instruction::Trunc { op, from, to } => {
            *to == t
                && matches!(from, Type::I64 | Type::I1 | Type::I8)
                && matches!(to, Type::I64 | Type::I1 | Type::I8)
                && exact(op, *from)
        }
        // Division/remainder, toInt and other fallible operations require their own
        // coverage of the error paths before waiving opaque effects.
        _ => false,
    }
}

// A local identity to reject consuming a solution of another version.
// It is neither a canonical SDK summary format nor a reload certificate.
pub(crate) fn body_signature(f: &Function) -> blake3::Hash {
    blake3::hash(format!("{f:?}").as_bytes())
}

// Only emission metadata used for the local field coverage.
// It does not replace versions of layouts, the runtime or JIT generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArcTableState {
    tables_mode: bool,
    table: Option<FunctionTables>,
}

pub(crate) fn table_state(m: &Module, idx_value: usize) -> ArcTableState {
    let table = if m.table_exceptions {
        m.tables.get(idx_value)
    } else {
        None
    };
    ArcTableState {
        tables_mode: m.table_exceptions,
        table: table.cloned(),
    }
}

pub(super) fn integer_constants(f: &Function) -> HashMap<ValueId, i64> {
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

// Only the reserved factory of a zeroed instance, without a callback or constructor.
// Dynamic class/count is not inferred from width or user name.
pub(super) fn zeroed_factory(
    i: &Instruction,
    constants: &HashMap<ValueId, i64>,
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
    let integer = |o: &Operand| match o {
        Operand::Constant(Constant::Int(n)) => Some(*n),
        Operand::Val(v) => constants.get(v).copied(),
        _ => None,
    };
    Some((
        u32::try_from(integer(&args[0].0)?).ok()?,
        usize::try_from(integer(&args[1].0)?).ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instantiated_path_includes_late_stores_and_both_previous_destinations() {
        let nodes = [
            AbstractNode::Global("a".into()),
            AbstractNode::Global("b".into()),
            AbstractNode::Global("c".into()),
        ];
        let field_list: Vec<_> = ["A", "B", "C"]
            .into_iter()
            .map(|layout| ArcField {
                layout: layout.into(),
                position: 0,
            })
            .collect();
        let schemas = std::sync::Arc::new(
            nodes.iter()
                .cloned()
                .zip(field_list.iter().cloned().map(|c| vec![c]))
                .collect(),
        );
        let mut rs: Vec<_> = nodes
            .iter()
            .enumerate()
            .map(|(dest, node)| Constraint::Seed {
                dest,
                node: node.clone(),
            })
            .collect();
        let mut variables = 3;
        let read = materialize_path(0, &[0, 0], &schemas, &mut variables, &mut rs).unwrap();
        for (obj_ref, val_ref) in [(0, 1), (1, 2), (2, 0), (0, 2)] {
            rs.push(Constraint::WriteBySchema {
                object: obj_ref,
                schemas: schemas.clone(),
                index: 0,
                value: val_ref,
            });
        }
        // a.child may be b or c; a.child.child may be c or a. A
        // read frozen before the late store a.child=c would lose a.
        let expected = BTreeSet::from([nodes[0].clone(), nodes[2].clone()]);
        for order in [false, true] {
            if order {
                rs.reverse();
            }
            let p = solve(variables, 8, &rs).unwrap();
            assert_eq!(p.variable(read).unwrap().nodes().unwrap(), &expected);
            super::super::points_to::verify(variables, 8, &rs, &p).unwrap();
        }
        let p = solve(variables, 1, &rs).unwrap();
        assert!(p.variable(read).unwrap().nodes().is_none());
    }

    fn fixture() -> (Function, HirFacts) {
        let field = ArcField {
            layout: "No:v1".into(),
            position: 0,
        };
        let mut facts = HirFacts::default();
        for v in [0, 1] {
            facts.objects.insert(
                ValueId(v),
                ObjectOrigin {
                    node: AbstractNode::Allocation {
                        site: ArcSite {
                            function: "ligar".into(),
                            origin: v as u64,
                            specialization: "".into(),
                        },
                        context: "".into(),
                    },
                    fields: vec![field.clone()],
                },
            );
        }
        facts.accesses.insert(ValueId(2), field.clone());
        facts.accesses.insert(ValueId(3), field);
        let f = Function {
            symbol: "ligar".into(),
            name: "ligar".into(),
            debug_info: None,
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
        (f, facts)
    }

    #[test]
    fn materialization_and_fallible_operation_do_not_preserve_fields_like_pure_arithmetic() {
        let (base, facts) = fixture();
        let cases = [
            (
                Instruction::Const(Constant::String("texto".into())),
                Type::Ref,
            ),
            (
                Instruction::Add(
                    Operand::Constant(Constant::String("texto".into())),
                    Operand::Constant(Constant::Int(1)),
                ),
                Type::I64,
            ),
            (
                Instruction::Add(
                    Operand::Val(ValueId(0)),
                    Operand::Constant(Constant::Int(1)),
                ),
                Type::I64,
            ),
            (
                Instruction::CheckNotNull(Operand::Constant(Constant::Null)),
                Type::Ref,
            ),
            (
                Instruction::SDiv(
                    Operand::Constant(Constant::Int(1)),
                    Operand::Constant(Constant::Int(0)),
                ),
                Type::I64,
            ),
        ];
        for (operation, t) in cases {
            let mut f = base.clone();
            f.blocks[0]
                .instructions
                .insert(3, (ValueId(4), operation, t));
            let a = analyze(&f, &facts, 8).unwrap();
            assert_eq!(a.unknown_set(), &[ValueId(4)]);
            assert!(a.value(ValueId(3)).unwrap().nodes().is_none());
            assert!(
                super::super::escape::compute(&f, &a).unwrap().publications
                    [&super::super::escape::EscapeCause::OpaqueOperation(4)]
                    .nodes()
                    .is_none()
            );
        }
        let mut f = base;
        f.blocks[0].instructions.insert(
            3,
            (
                ValueId(4),
                Instruction::Add(
                    Operand::Constant(Constant::Int(1)),
                    Operand::Constant(Constant::Int(2)),
                ),
                Type::I64,
            ),
        );
        let a = analyze(&f, &facts, 8).unwrap();
        assert!(a.unknown_set().is_empty());
        assert!(a.value(ValueId(4)).unwrap().nodes().unwrap().is_empty());
        assert_eq!(a.value(ValueId(3)), a.value(ValueId(1)));
    }

    #[test]
    fn phi_with_materialized_string_is_not_effectless_copy() {
        let (mut f, facts) = fixture();
        let read = f.blocks[0].instructions.pop().unwrap();
        f.blocks[0].terminator = Terminator::CondBranch {
            cond: Operand::Constant(Constant::Bool(true)),
            then_block: BlockId(1),
            else_block: BlockId(2),
        };
        for id in [1, 2] {
            f.blocks.push(BasicBlock {
                id: BlockId(id),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(3)),
            });
        }
        f.blocks.push(BasicBlock {
            id: BlockId(3),
            instructions: vec![
                (
                    ValueId(4),
                    Instruction::Phi {
                        incoming: vec![
                            (BlockId(1), Operand::Val(ValueId(0))),
                            (
                                BlockId(2),
                                Operand::Constant(Constant::String("texto".into())),
                            ),
                        ],
                        ty: Type::I64,
                    },
                    Type::I64,
                ),
                read,
            ],
            terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
        });
        let a = analyze(&f, &facts, 8).unwrap();
        assert_eq!(a.unknown_set(), &[ValueId(4)]);
        assert!(a.value(ValueId(3)).unwrap().nodes().is_none());
        f.blocks[3].instructions[0].2 = Type::Ref;
        if let Instruction::Phi { ty, .. } = &mut f.blocks[3].instructions[0].1 {
            *ty = Type::Ref;
        }
        assert!(analyze(&f, &facts, 8).is_err());
        if let Instruction::Phi { incoming, .. } = &mut f.blocks[3].instructions[0].1 {
            incoming[1].1 = Operand::Val(ValueId(1));
        }
        let a = analyze(&f, &facts, 8).unwrap();
        assert!(a.unknown_set().is_empty());
        assert_eq!(a.value(ValueId(4)).unwrap().nodes().unwrap().len(), 2);
        assert_eq!(a.value(ValueId(3)), a.value(ValueId(1)));
    }

    #[test]
    fn hir_initializers_and_stores_preserve_cycle_and_identity() {
        let (f, facts) = fixture();
        let a = analyze(&f, &facts, 8).unwrap();
        assert!(a.unknown_set.is_empty());
        assert_eq!(a.value(ValueId(3)), a.value(ValueId(1)));
        assert_eq!(
            a.points
                .field(&facts.objects[&ValueId(1)].node, &facts.accesses[&ValueId(3)]),
            a.value(ValueId(0))
        );
    }

    #[test]
    fn opaque_calls_and_missing_facts_never_prove_alias_absence() {
        let (mut f, mut facts) = fixture();
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
        let a = analyze(&f, &facts, 8).unwrap();
        assert_eq!(a.unknown_set, vec![ValueId(4)]);
        assert!(a.value(ValueId(3)).unwrap().nodes().is_none());
        f.blocks[0].instructions.remove(3);
        facts.objects.remove(&ValueId(1));
        let a = analyze(&f, &facts, 8).unwrap();
        assert!(a.value(ValueId(1)).unwrap().nodes().is_none());
        assert!(a.value(ValueId(3)).unwrap().nodes().is_none());
    }

    #[test]
    fn stale_facts_and_incompatible_schemas_are_rejected() {
        let (f, mut facts) = fixture();
        facts.objects.get_mut(&ValueId(0)).unwrap().fields.clear();
        assert!(analyze(&f, &facts, 8).is_err());
        let (f, mut facts) = fixture();
        facts.accesses.get_mut(&ValueId(3)).unwrap().position = 1;
        assert!(analyze(&f, &facts, 8).is_err());
    }

    #[test]
    fn integer_bits_and_untyped_initial_fields_do_not_prove_handle_absence() {
        let (mut f, facts) = fixture();
        if let Instruction::AllocObject { fields, .. } = &mut f.blocks[0].instructions[0].1 {
            fields[0] = Operand::Constant(Constant::Int(42));
        }
        let a = analyze(&f, &facts, 8).unwrap();
        assert!(a.value(ValueId(3)).unwrap().nodes().is_none());
        let f = Function {
            symbol: "bits".into(),
            name: "bits".into(),
            debug_info: None,
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
        let a = analyze(&f, &HirFacts::default(), 8).unwrap();
        assert!(a.value(ValueId(0)).unwrap().nodes().is_none());
        assert!(a.value(ValueId(1)).unwrap().nodes().is_none());
    }
}
