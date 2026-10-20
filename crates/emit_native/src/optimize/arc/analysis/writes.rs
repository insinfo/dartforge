//! Strong-edge inclusions produced by parameterized stores (§28.3).
//! They are not liveness/publication coverage: opaque fields and implicit paths
//! still require general ARC even when these inclusions are known.

use super::escape::EscapeCause;
use super::summaries::*;
use super::{hir::ObjectOrigin, model::AbstractNode, points_to::ArcField};
use crate::hir::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum PathOrigin {
    Parameter(usize),
    // Context-insensitive: repeated calls may represent many
    // instances. The schema follows the origin; it is not a proof of singleton.
    Allocation {
        node: AbstractNode,
        fields: Vec<ArcField>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct SummaryPath {
    pub origin: PathOrigin,
    pub fields: Vec<usize>,
}

impl SummaryPath {
    fn root(parameter: usize) -> Self {
        Self {
            origin: PathOrigin::Parameter(parameter),
            fields: Vec::new(),
        }
    }

    fn proceed(&self, field_list: &[usize], limit: usize) -> Option<Self> {
        if self.fields.len().checked_add(field_list.len())? > limit {
            return None;
        }
        let mut path = self.clone();
        path.fields.extend_from_slice(field_list);
        Some(path)
    }

    fn instantiate(
        &self,
        args: &[Operand],
        operand: impl Fn(&Operand) -> AliasPaths,
        limit: usize,
    ) -> Result<AliasPaths, String> {
        match &self.origin {
            PathOrigin::Parameter(p) => {
                let arg = args.get(*p).ok_or("ARC summary: invalid parameter")?;
                Ok(extend(operand(arg), &self.fields, limit))
            }
            PathOrigin::Allocation { .. } => Ok(Some(BTreeSet::from([self.clone()]))),
        }
    }
}

// A bottom without evidence of a normal result stays internal to the SCC solution.
// Null is kept apart from the paths: it never gets the identity of an abstract object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReturnPaths {
    pub paths: Option<BTreeSet<SummaryPath>>,
    pub null: bool,
}

impl ReturnPaths {
    pub(super) fn empty() -> Self {
        Self {
            paths: Some(BTreeSet::new()),
            null: false,
        }
    }

    pub(super) fn unknown() -> Self {
        Self {
            paths: None,
            null: true,
        }
    }

    pub(super) fn join_set(&mut self, other: &Self, limit: usize) -> bool {
        let before = self.clone();
        join_aliases(&mut self.paths, other.paths.clone(), limit);
        self.null |= other.null || self.paths.is_none();
        *self != before
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ParameterWrite {
    pub object: SummaryPath,
    // None represents a side edge, with no position in the body of the object.
    pub index: Option<usize>,
    pub value: SummaryPath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum WriteInclusions {
    Known(BTreeSet<ParameterWrite>),
    // Positive inclusions do not certify the absence of other writers.
    Partial(BTreeSet<ParameterWrite>),
    UnknownValues,
    // An exhausted budget does not resume accumulating facts inside the fixpoint.
    Exceeded,
}

impl WriteInclusions {
    pub(super) fn empty() -> Self {
        Self::Known(BTreeSet::new())
    }

    pub(super) fn positive_set(&self) -> Option<&BTreeSet<ParameterWrite>> {
        match self {
            Self::Known(pairs) | Self::Partial(pairs) => Some(pairs),
            Self::UnknownValues | Self::Exceeded => None,
        }
    }

    fn mark_opaque(&mut self) {
        self.join_set(&Self::UnknownValues, usize::MAX);
    }

    // Opacity is monotonic, but does not erase witnessed inclusions.
    // Only the exhausted budget is absorbing for these inclusions too.
    pub(super) fn join_set(&mut self, other: &Self, limit: usize) -> bool {
        let before = self.clone();
        if matches!(self, Self::Exceeded) || matches!(other, Self::Exceeded) {
            *self = Self::Exceeded;
            return *self != before;
        }
        let opaque_set = !matches!(self, Self::Known(_)) || !matches!(other, Self::Known(_));
        let mut pairs = match std::mem::replace(self, Self::empty()) {
            Self::Known(pairs) | Self::Partial(pairs) => pairs,
            Self::UnknownValues => BTreeSet::new(),
            Self::Exceeded => unreachable!(),
        };
        if let Some(others) = other.positive_set() {
            pairs.extend(others.iter().cloned());
        }
        *self = if pairs.len() > limit {
            Self::Exceeded
        } else if !opaque_set {
            Self::Known(pairs)
        } else if pairs.is_empty() {
            Self::UnknownValues
        } else {
            Self::Partial(pairs)
        };
        *self != before
    }
}

// Paths represent reads in the caller's weak superstate: their
// instantiation must include every store, without erasing previous values.
// The length and the number of aliases are bounded; growing beyond the
// budget produces top, including in Phis or recursive calls.
type AliasPaths = Option<BTreeSet<SummaryPath>>;

// Only explicit inclusions: an empty map does not certify the absence of
// publication by the SDK, a callback, a finalizer or an implicit emission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PublicationInclusions {
    Known(BTreeMap<EscapeCause, AliasPaths>),
    UnknownValues,
}

impl PublicationInclusions {
    pub(super) fn empty() -> Self {
        Self::Known(BTreeMap::new())
    }

    pub(super) fn join_set(&mut self, other: &Self, limit: usize) -> bool {
        let before = self.clone();
        match (&mut *self, other) {
            (Self::Known(a), Self::Known(b)) => {
                for (sym_name, paths) in b {
                    join_aliases(
                        a.entry(sym_name.clone())
                            .or_insert_with(|| Some(BTreeSet::new())),
                        paths.clone(),
                        limit,
                    );
                }
                // Destinations also consume budget, including the publication of null.
                let size = a.values().try_fold(a.len(), |n, c| {
                    n.checked_add(c.as_ref().map_or(0, BTreeSet::len))
                });
                if size.is_none_or(|n| n > limit) {
                    *self = Self::UnknownValues;
                }
            }
            (_, Self::UnknownValues) => *self = Self::UnknownValues,
            _ => {}
        }
        *self != before
    }
}

fn join_aliases(dest: &mut AliasPaths, origin: AliasPaths, limit: usize) -> bool {
    let before = dest.clone();
    match (&mut *dest, origin) {
        (Some(a), Some(b)) => {
            a.extend(b);
            if a.len() > limit {
                *dest = None;
            }
        }
        (_, None) => *dest = None,
        _ => {}
    }
    *dest != before
}

fn extend(aliases: AliasPaths, field_list: &[usize], limit: usize) -> AliasPaths {
    aliases?
        .into_iter()
        .map(|c| c.proceed(field_list, limit))
        .collect()
}

#[cfg(test)]
fn extract<'a>(
    f: &Function,
    limit: usize,
    call: impl Fn(&str) -> Option<(&'a Function, &'a ArcHeapReturn, &'a WriteInclusions)>,
) -> Result<WriteInclusions, String> {
    Ok(extract_with_result(f, limit, false, |s| {
        call(s).map(|(callee, ret, writes)| {
            let paths = match ret {
                ArcHeapReturn::Aliases { parameters, null } => ReturnPaths {
                    paths: Some(parameters.iter().copied().map(SummaryPath::root).collect()),
                    null: *null,
                },
                ArcHeapReturn::UnknownValue => ReturnPaths::unknown(),
            };
            (callee, paths, writes)
        })
    })?
    .1)
}

#[cfg(test)]
fn extract_with_result<'a>(
    f: &Function,
    limit: usize,
    internal: bool,
    call: impl Fn(&str) -> Option<(&'a Function, ReturnPaths, &'a WriteInclusions)>,
) -> Result<(ReturnPaths, WriteInclusions), String> {
    let (r, e, _) = extract_with_effects(f, limit, internal, |s| {
        call(s).map(|(f, r, e)| (f, r, e, PublicationInclusions::empty()))
    })?;
    Ok((r, e))
}

pub(super) fn extract_with_effects<'a>(
    f: &Function,
    limit: usize,
    internal: bool,
    call: impl Fn(
        &str,
    ) -> Option<(
        &'a Function,
        ReturnPaths,
        &'a WriteInclusions,
        PublicationInclusions,
    )>,
) -> Result<(ReturnPaths, WriteInclusions, PublicationInclusions), String> {
    extract_with_origins(f, limit, internal, &HashMap::new(), call)
}

pub(super) fn extract_with_origins<'a>(
    f: &Function,
    limit: usize,
    internal: bool,
    origins: &HashMap<ValueId, ObjectOrigin>,
    call: impl Fn(
        &str,
    ) -> Option<(
        &'a Function,
        ReturnPaths,
        &'a WriteInclusions,
        PublicationInclusions,
    )>,
) -> Result<(ReturnPaths, WriteInclusions, PublicationInclusions), String> {
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
    let mut value_list: Vec<_> = types.keys().copied().collect();
    value_list.sort_by_key(|v| v.0);
    let ids: HashMap<_, _> = value_list
        .iter()
        .enumerate()
        .map(|(i, v)| (*v, i))
        .collect();
    let mut aliases = vec![Some(BTreeSet::new()); ids.len()];
    let mut nulls = vec![false; ids.len()];
    let mut opaque_writes = false;
    for (p, (v, _, t)) in f.params.iter().enumerate() {
        aliases[ids[v]] = if *t == Type::Ref && limit > 0 {
            Some(BTreeSet::from([SummaryPath::root(p)]))
        } else {
            None
        };
    }
    let operand = |o: &Operand, aliases: &[AliasPaths]| match o {
        Operand::Val(v) => aliases[ids[v]].clone(),
        Operand::Constant(Constant::Null) => Some(BTreeSet::new()),
        _ => None,
    };
    let null = |o: &Operand, nulls: &[bool]| match o {
        Operand::Val(v) => nulls[ids[v]],
        Operand::Constant(Constant::Null) => true,
        _ => false,
    };
    // Bottom stays internal until convergence. Stores are only extracted after
    // stabilizing copies, paths and aliases of local normal results.
    let instructions: Vec<_> = f.blocks.iter().flat_map(|b| &b.instructions).collect();
    let mut dependents = vec![Vec::new(); ids.len()];
    for (idx_value, (_, i, _)) in instructions.iter().enumerate() {
        crate::optimize::operands::operands(i, &mut |o| {
            if let Operand::Val(v) = o {
                dependents[ids[v]].push(idx_value);
            }
        });
    }
    let mut queue: VecDeque<_> = (0..instructions.len()).collect();
    let mut queued = vec![true; instructions.len()];
    while let Some(idx_value) = queue.pop_front() {
        queued[idx_value] = false;
        let (v, i, t) = instructions[idx_value];
        let mut new_null = false;
        let new = match i {
            _ if *t == Type::Ref && origins.contains_key(v) => {
                // Knowing the identity of the normal result does not cover the
                // effects of the allocation, the construction or its fields.
                opaque_writes = true;
                if limit == 0 {
                    None
                } else {
                    let origin = &origins[v];
                    Some(BTreeSet::from([SummaryPath {
                        origin: PathOrigin::Allocation {
                            node: origin.node.clone(),
                            fields: origin.fields.clone(),
                        },
                        fields: Vec::new(),
                    }]))
                }
            }
            Instruction::Const(Constant::Null) if *t == Type::Ref => {
                new_null = true;
                Some(BTreeSet::new())
            }
            Instruction::AllocEnv { values } if values.is_empty() && *t == Type::Ref => {
                new_null = true;
                Some(BTreeSet::new())
            }
            Instruction::ArcCopy { value } | Instruction::ArcMove { value }
                if *t == Type::Ref && exact_argument(value, Type::Ref, &types) =>
            {
                new_null = null(value, &nulls);
                operand(value, &aliases)
            }
            Instruction::Bitcast { op, to: Type::Ref }
                if *t == Type::Ref && exact_argument(op, Type::Ref, &types) =>
            {
                new_null = null(op, &nulls);
                operand(op, &aliases)
            }
            Instruction::Phi {
                incoming,
                ty: Type::Ref,
            } if *t == Type::Ref
                && incoming
                    .iter()
                    .all(|(_, o)| exact_argument(o, Type::Ref, &types)) =>
            {
                let mut a = Some(BTreeSet::new());
                for (_, o) in incoming {
                    new_null |= null(o, &nulls);
                    join_aliases(&mut a, operand(o, &aliases), limit);
                }
                a
            }
            i if *t == Type::Ref
                && super::hir::field_read(i)
                    .is_some_and(|(object, _)| exact_argument(object, Type::Ref, &types)) =>
            {
                let (object, index) = super::hir::field_read(i).unwrap();
                let a = extend(operand(object, &aliases), &[index], limit);
                // A field may be null, but bottom without a receiver is not
                // evidence of normal execution in baseless recursion.
                new_null = a.as_ref().is_none_or(|c| !c.is_empty()) || null(object, &nulls);
                a
            }
            Instruction::CallStatic {
                symbol,
                args,
                ret_ty,
            } => {
                let callee = call(symbol);
                if let Some((callee, ret, _, _)) = callee
                    && *t == *ret_ty
                    && *ret_ty == callee.return_ty
                    && args.len() == callee.params.len()
                    && args
                        .iter()
                        .zip(&callee.params)
                        .all(|(o, (_, _, ty))| exact_argument(o, *ty, &types))
                {
                    if *t == Type::Ref {
                        new_null = ret.null;
                        if let Some(paths) = &ret.paths {
                            let mut a = Some(BTreeSet::new());
                            for path in paths {
                                if let PathOrigin::Parameter(p) = &path.origin {
                                    let arg =
                                        args.get(*p).ok_or("ARC summary: invalid parameter")?;
                                    new_null |= null(arg, &nulls);
                                }
                                join_aliases(
                                    &mut a,
                                    path.instantiate(args, |o| operand(o, &aliases), limit)?,
                                    limit,
                                );
                            }
                            a
                        } else {
                            None
                        }
                    } else {
                        Some(BTreeSet::new())
                    }
                } else {
                    opaque_writes = true;
                    None
                }
            }
            i if *t == Type::Void
                && super::hir::field_write(i).is_some_and(|(object, _, value)| {
                    exact_argument(object, Type::Ref, &types)
                        && exact_argument(value, Type::Ref, &types)
                }) =>
            {
                Some(BTreeSet::new())
            }
            Instruction::Const(c) if exact_argument(&Operand::Constant(c.clone()), *t, &types) => {
                None
            }
            i if super::hir::pure_scalar(i, *t, &types) => None,
            _ => {
                opaque_writes = true;
                None
            }
        };
        new_null |= *t == Type::Ref && new.is_none();
        let alias_changed = join_aliases(&mut aliases[ids[v]], new, limit);
        let null_changed = new_null && !nulls[ids[v]];
        nulls[ids[v]] |= new_null;
        if alias_changed || null_changed {
            for &dependent in &dependents[ids[v]] {
                if !queued[dependent] {
                    queued[dependent] = true;
                    queue.push_back(dependent);
                }
            }
        }
    }
    let mut ret = if f.return_ty == Type::Ref {
        ReturnPaths::empty()
    } else {
        ReturnPaths::unknown()
    };
    let mut normal = false;
    for b in &f.blocks {
        if let Terminator::Return(Some(o)) = &b.terminator {
            normal = true;
            ret.join_set(
                &ReturnPaths {
                    paths: if f.return_ty == Type::Ref && exact_argument(o, Type::Ref, &types) {
                        operand(o, &aliases)
                    } else {
                        None
                    },
                    null: null(o, &nulls),
                },
                limit,
            );
        }
    }
    if !internal && (!normal || ret == ReturnPaths::empty()) {
        ret = ReturnPaths::unknown();
    }
    let mut publications = PublicationInclusions::empty();
    for (_, i, t) in &instructions {
        let publication = match i {
            Instruction::StoreGlobal {
                symbol: sym_name,
                val,
                ty: Type::Ref,
                ..
            } if *t == Type::Void && exact_argument(val, Type::Ref, &types) => {
                Some((sym_name, val))
            }
            Instruction::ArcStoreStrong {
                slot: StrongSlot::Global { symbol: sym_name },
                value,
                ..
            } if *t == Type::Void && exact_argument(value, Type::Ref, &types) => {
                Some((sym_name, value))
            }
            _ => None,
        };
        if let Some((sym_name, val_ref)) = publication {
            publications.join_set(
                &PublicationInclusions::Known(BTreeMap::from([(
                    EscapeCause::Global(sym_name.clone()),
                    operand(val_ref, &aliases),
                )])),
                limit,
            );
        }
        if let Instruction::CallStatic {
            symbol,
            args,
            ret_ty,
        } = i
            && let Some((callee, _, _, inclusions)) = call(symbol)
            && *t == *ret_ty
            && *ret_ty == callee.return_ty
            && args.len() == callee.params.len()
            && args
                .iter()
                .zip(&callee.params)
                .all(|(o, (_, _, ty))| exact_argument(o, *ty, &types))
        {
            match inclusions {
                PublicationInclusions::UnknownValues => {
                    publications.join_set(&PublicationInclusions::UnknownValues, limit);
                }
                PublicationInclusions::Known(inclusions) => {
                    for (sym_name, paths) in inclusions {
                        let mut value_list = Some(BTreeSet::new());
                        if let Some(paths) = paths {
                            for path in paths {
                                join_aliases(
                                    &mut value_list,
                                    path.instantiate(args, |o| operand(o, &aliases), limit)?,
                                    limit,
                                );
                            }
                        } else {
                            value_list = None;
                        }
                        publications.join_set(
                            &PublicationInclusions::Known(BTreeMap::from([(sym_name, value_list)])),
                            limit,
                        );
                    }
                }
            }
        }
    }
    // Including every throw is conservative even when a table could
    // catch them in the body itself. This does not summarize SDK wrappers/errors.
    for b in &f.blocks {
        if let Terminator::Throw(val_ref) = &b.terminator {
            publications.join_set(
                &PublicationInclusions::Known(BTreeMap::from([(
                    EscapeCause::Exception,
                    operand(val_ref, &aliases),
                )])),
                limit,
            );
        }
    }
    let mut result = WriteInclusions::empty();
    if opaque_writes {
        result.mark_opaque();
    }
    for (v, i, t) in f.blocks.iter().flat_map(|b| &b.instructions) {
        if *t == Type::Ref
            && origins.contains_key(v)
            && let Some((_, field_list)) = super::hir::synthetic_captures(i, &types)
        {
            for (idx_value, value) in field_list.into_iter().enumerate() {
                if let Some(value) = value {
                    include_write(
                        &mut result,
                        operand(&Operand::Val(*v), &aliases),
                        Some(idx_value),
                        operand(&value, &aliases),
                        limit,
                    );
                }
            }
        }
        match i {
            i if super::hir::dart_attachment(i, *t, &types).is_some() => {
                let (owner, action) = super::hir::dart_attachment(i, *t, &types).unwrap();
                include_write(
                    &mut result,
                    operand(owner, &aliases),
                    None,
                    operand(action, &aliases),
                    limit,
                );
            }
            Instruction::AllocObject { fields, .. }
                if *t == Type::Ref && origins.contains_key(v) =>
            {
                // The origin/layout was validated by the module. Record each
                // possible initializer without closing the allocation boundary.
                for (idx_value, value) in fields.iter().enumerate() {
                    if exact_argument(value, Type::Ref, &types) {
                        include_write(
                            &mut result,
                            operand(&Operand::Val(*v), &aliases),
                            Some(idx_value),
                            operand(value, &aliases),
                            limit,
                        );
                    }
                }
            }
            i if *t == Type::Void
                && super::hir::field_write(i).is_some_and(|(object, _, value)| {
                    exact_argument(object, Type::Ref, &types)
                        && exact_argument(value, Type::Ref, &types)
                }) =>
            {
                let (object, index, value) = super::hir::field_write(i).unwrap();
                include_write(
                    &mut result,
                    operand(object, &aliases),
                    Some(index),
                    operand(value, &aliases),
                    limit,
                );
            }
            Instruction::CallStatic {
                symbol,
                args,
                ret_ty,
            } => {
                let Some((callee, _, writes, _)) = call(symbol) else {
                    result.mark_opaque();
                    continue;
                };
                if *t != *ret_ty
                    || *ret_ty != callee.return_ty
                    || args.len() != callee.params.len()
                    || !args
                        .iter()
                        .zip(&callee.params)
                        .all(|(o, (_, _, ty))| exact_argument(o, *ty, &types))
                {
                    result.mark_opaque();
                    continue;
                }
                if matches!(writes, WriteInclusions::Exceeded) {
                    result.join_set(writes, limit);
                    continue;
                }
                if !matches!(writes, WriteInclusions::Known(_)) {
                    result.mark_opaque();
                }
                let Some(writes) = writes.positive_set() else {
                    continue;
                };
                for write in writes {
                    include_write(
                        &mut result,
                        write
                            .object
                            .instantiate(args, |o| operand(o, &aliases), limit)?,
                        write.index,
                        write
                            .value
                            .instantiate(args, |o| operand(o, &aliases), limit)?,
                        limit,
                    );
                }
            }
            _ => {}
        }
    }
    Ok((ret, result, publications))
}

fn include_write(
    result: &mut WriteInclusions,
    origins: AliasPaths,
    idx_value: Option<usize>,
    dests: AliasPaths,
    limit: usize,
) {
    let (Some(origins), Some(dests)) = (origins, dests) else {
        result.mark_opaque();
        return;
    };
    for origin in origins {
        for dest in &dests {
            result.join_set(
                &WriteInclusions::Known(BTreeSet::from([ParameterWrite {
                    object: origin.clone(),
                    index: idx_value,
                    value: dest.clone(),
                }])),
                limit,
            );
            if matches!(result, WriteInclusions::Exceeded) {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn capture_read_and_write_compose_paths_without_certifying_coverage() {
        let mut f = link("capturar", false);
        f.blocks[0].instructions = vec![
            (
                ValueId(2),
                Instruction::EnvGet {
                    env: Operand::Val(ValueId(0)),
                    index: 3,
                },
                Type::Ref,
            ),
            (
                ValueId(3),
                Instruction::CellSet {
                    cell: Operand::Val(ValueId(2)),
                    value: Operand::Val(ValueId(1)),
                },
                Type::Void,
            ),
            (
                ValueId(4),
                Instruction::CellGet {
                    cell: Operand::Val(ValueId(2)),
                },
                Type::Ref,
            ),
        ];
        f.return_ty = Type::Ref;
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(4))));
        let mut wrapper = f.clone();
        wrapper.symbol = "wrapper".into();
        wrapper.blocks[0].instructions = vec![(
            ValueId(2),
            Instruction::CallStatic {
                symbol: "capturar".into(),
                args: vec![Operand::Val(ValueId(1)), Operand::Val(ValueId(0))],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        )];
        wrapper.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(2))));
        let mut m = Module::new();
        m.arc_memory = true;
        m.functions = vec![wrapper, f];
        for reverse in [false, true] {
            if reverse {
                m.functions.reverse();
            }
            let r = super::super::calls::solve(&m, &HashSet::from(["wrapper"]), 8).unwrap();
            assert_eq!(
                r["wrapper"].1.write_inclusions,
                WriteInclusions::Known(BTreeSet::from([ParameterWrite {
                    object: SummaryPath {
                        origin: PathOrigin::Parameter(1),
                        fields: vec![3]
                    },
                    index: Some(0),
                    value: SummaryPath::root(0),
                }]))
            );
            assert_eq!(
                r["wrapper"].1.return_paths.paths,
                Some(BTreeSet::from([SummaryPath {
                    origin: PathOrigin::Parameter(1),
                    fields: vec![3, 0]
                }]))
            );
            assert_eq!(r["wrapper"].1.fields(), ArcHeapFields::UnknownSet);
            assert_eq!(
                r["wrapper"].1.publications(),
                ArcHeapPublications::UnknownValues
            );
        }
        let f = m
            .functions
            .iter_mut()
            .find(|f| f.symbol == "capturar")
            .unwrap();
        if let Instruction::CellSet { value, .. } = &mut f.blocks[0].instructions[1].1 {
            *value = Operand::Constant(Constant::Int(41));
        }
        let r = super::super::calls::solve(&m, &HashSet::from(["wrapper"]), 8).unwrap();
        assert_eq!(
            r["wrapper"].1.write_inclusions,
            WriteInclusions::UnknownValues
        );
    }

    fn getter(name: &str, idx_value: usize) -> Function {
        Function {
            symbol: name.into(),
            name: name.into(),
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(1),
                    Instruction::GetField {
                        object: Operand::Val(ValueId(0)),
                        index: idx_value,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
            }],
        }
    }

    #[test]
    fn composite_exception_preserves_paths_and_does_not_collide_with_global_name() {
        let mut explode = getter("explode", 2);
        explode.return_ty = Type::Void;
        explode.blocks[0].terminator = Terminator::Throw(Operand::Val(ValueId(1)));
        let mut wrapper = getter("wrapper", 9);
        wrapper.blocks[0].instructions.push((
            ValueId(2),
            Instruction::CallStatic {
                symbol: "explode".into(),
                args: vec![Operand::Val(ValueId(1))],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
        wrapper.blocks[0].instructions.push((
            ValueId(3),
            Instruction::StoreGlobal {
                symbol: "Excecao".into(),
                val: Operand::Val(ValueId(1)),
                ty: Type::Ref,
                root: Some(0),
            },
            Type::Void,
        ));
        let mut m = Module::new();
        m.arc_memory = true;
        m.globals.push((0, Type::Ref, "Excecao".into()));
        m.functions = vec![wrapper, explode];
        for order in [false, true] {
            if order {
                m.functions.reverse();
            }
            let r = super::super::calls::solve(&m, &HashSet::from(["wrapper"]), 8).unwrap();
            assert_eq!(
                r["wrapper"].1.publication_inclusions,
                PublicationInclusions::Known(BTreeMap::from([
                    (
                        EscapeCause::Exception,
                        Some(BTreeSet::from([SummaryPath {
                            origin: PathOrigin::Parameter(0),
                            fields: vec![9, 2]
                        }]))
                    ),
                    (
                        EscapeCause::Global("Excecao".into()),
                        Some(BTreeSet::from([SummaryPath {
                            origin: PathOrigin::Parameter(0),
                            fields: vec![9]
                        }]))
                    ),
                ]))
            );
            assert_eq!(
                r["wrapper"].1.publications(),
                ArcHeapPublications::UnknownValues
            );
            let r = super::super::calls::solve(&m, &HashSet::from(["wrapper"]), 3).unwrap();
            assert_eq!(
                r["wrapper"].1.publication_inclusions,
                PublicationInclusions::UnknownValues
            );
        }
    }

    #[test]
    fn composite_global_publications_preserve_paths_and_null() {
        let mut g = getter("get", 2);
        g.blocks[0].instructions.push((
            ValueId(2),
            Instruction::ArcStoreStrong {
                slot: StrongSlot::Global { symbol: "g".into() },
                value: Operand::Val(ValueId(1)),
                mode: StrongStoreMode::Copy,
            },
            Type::Void,
        ));
        let mut wrapper = getter("wrapper", 9);
        wrapper.blocks[0].instructions.push((
            ValueId(2),
            Instruction::CallStatic {
                symbol: "get".into(),
                args: vec![Operand::Val(ValueId(1))],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        ));
        wrapper.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(2))));
        let mut m = Module::new();
        m.arc_memory = true;
        m.globals.push((0, Type::Ref, "g".into()));
        m.functions = vec![wrapper, g];
        for order in [false, true] {
            if order {
                m.functions.reverse();
            }
            let r = super::super::calls::solve(&m, &HashSet::from(["wrapper"]), 8).unwrap();
            assert_eq!(
                r["wrapper"].1.publication_inclusions,
                PublicationInclusions::Known(BTreeMap::from([(
                    EscapeCause::Global("g".into()),
                    Some(BTreeSet::from([SummaryPath {
                        origin: PathOrigin::Parameter(0),
                        fields: vec![9, 2]
                    }]))
                )]))
            );
            assert_eq!(
                r["wrapper"].1.publications(),
                ArcHeapPublications::UnknownValues
            );
        }
        let g = m.functions.iter_mut().find(|f| f.symbol == "get").unwrap();
        if let Instruction::ArcStoreStrong { value, .. } = &mut g.blocks[0].instructions[1].1 {
            *value = Operand::Constant(Constant::Null);
        }
        let r = super::super::calls::solve(&m, &HashSet::from(["wrapper"]), 8).unwrap();
        assert_eq!(
            r["wrapper"].1.publication_inclusions,
            PublicationInclusions::Known(BTreeMap::from([(
                EscapeCause::Global("g".into()),
                Some(BTreeSet::new())
            )]))
        );
    }

    #[test]
    fn scc_publishes_both_arguments_and_budget_does_not_hide_invalid_abi() {
        for variant in 0..3 {
            let mut f = link("pub", false);
            f.blocks[0].instructions[0].1 = match variant {
                0 => Instruction::StoreGlobal {
                    symbol: "g".into(),
                    val: Operand::Val(ValueId(0)),
                    ty: Type::Ref,
                    root: Some(0),
                },
                _ => Instruction::ArcStoreStrong {
                    slot: StrongSlot::Global { symbol: "g".into() },
                    value: Operand::Val(ValueId(0)),
                    mode: if variant == 1 {
                        StrongStoreMode::Copy
                    } else {
                        StrongStoreMode::Move
                    },
                },
            };
            f.blocks[0].instructions.push((
                ValueId(3),
                Instruction::CallStatic {
                    symbol: "pub".into(),
                    args: vec![Operand::Val(ValueId(1)), Operand::Val(ValueId(0))],
                    ret_ty: Type::Void,
                },
                Type::Void,
            ));
            let mut m = Module::new();
            m.arc_memory = true;
            m.functions.push(f);
            m.globals.push((0, Type::Ref, "g".into()));
            let r = super::super::calls::solve(&m, &HashSet::from(["pub"]), 3).unwrap();
            assert_eq!(
                r["pub"].1.publication_inclusions,
                PublicationInclusions::Known(BTreeMap::from([(
                    EscapeCause::Global("g".into()),
                    Some(BTreeSet::from([SummaryPath::root(0), SummaryPath::root(1)]))
                )]))
            );
            let r = super::super::calls::solve(&m, &HashSet::from(["pub"]), 2).unwrap();
            assert_eq!(
                r["pub"].1.publication_inclusions,
                PublicationInclusions::UnknownValues
            );
            m.globals.push((0, Type::Ref, "g".into()));
            assert!(super::super::calls::solve(&m, &HashSet::from(["pub"]), 0).is_err());
            m.globals.pop();
            m.globals[0].1 = Type::I64;
            assert!(super::super::calls::solve(&m, &HashSet::from(["pub"]), 8).is_err());
            m.globals.clear();
            assert!(super::super::calls::solve(&m, &HashSet::from(["pub"]), 8).is_err());
        }
    }

    #[test]
    fn result_alias_does_not_erase_external_effect_and_zero_budget_keeps_top() {
        let mut f = getter("externo", 0);
        f.blocks[0].instructions = vec![(
            ValueId(1),
            Instruction::CallStatic {
                symbol: "desconhecido".into(),
                args: vec![Operand::Val(ValueId(0))],
                ret_ty: Type::Void,
            },
            Type::Void,
        )];
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(0))));
        let (ret, writes) = extract_with_result(&f, 8, false, |_| None).unwrap();
        assert_eq!(
            ret,
            ReturnPaths {
                paths: Some(BTreeSet::from([SummaryPath::root(0)])),
                null: false,
            }
        );
        assert_eq!(writes, WriteInclusions::UnknownValues);
        let (ret, writes) = extract_with_result(&f, 0, false, |_| None).unwrap();
        assert_eq!(ret, ReturnPaths::unknown());
        assert_eq!(writes, WriteInclusions::UnknownValues);
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Constant(Constant::Null)));
        let (ret, writes) = extract_with_result(&f, 0, false, |_| None).unwrap();
        assert_eq!(
            ret,
            ReturnPaths {
                paths: Some(BTreeSet::new()),
                null: true
            }
        );
        assert_eq!(writes, WriteInclusions::UnknownValues);
    }

    #[test]
    fn composite_field_return_feeds_writes_without_promoting_coverage() {
        let g = getter("get", 2);
        let mut wrapper = getter("wrapper", 9);
        wrapper.blocks[0].instructions.push((
            ValueId(2),
            Instruction::CallStatic {
                symbol: "get".into(),
                args: vec![Operand::Val(ValueId(1))],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        ));
        wrapper.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(2))));
        let mut writer = link("writer", false);
        writer.blocks[0].instructions.insert(
            0,
            (
                ValueId(3),
                Instruction::CallStatic {
                    symbol: "wrapper".into(),
                    args: vec![Operand::Val(ValueId(0))],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ),
        );
        if let Instruction::SetField { object, .. } = &mut writer.blocks[0].instructions[1].1 {
            *object = Operand::Val(ValueId(3));
        }
        let mut m = Module::new();
        m.arc_memory = true;
        m.functions = vec![writer, wrapper, g];
        for order in [false, true] {
            if order {
                m.functions.reverse();
            }
            let r = super::super::calls::solve(&m, &HashSet::from(["writer"]), 8).unwrap();
            assert_eq!(
                r["wrapper"].1.return_paths,
                ReturnPaths {
                    paths: Some(BTreeSet::from([SummaryPath {
                        origin: PathOrigin::Parameter(0),
                        fields: vec![9, 2]
                    }])),
                    null: true,
                }
            );
            assert_eq!(
                r["writer"].1.write_inclusions,
                WriteInclusions::Known(BTreeSet::from([ParameterWrite {
                    object: SummaryPath {
                        origin: PathOrigin::Parameter(0),
                        fields: vec![9, 2]
                    },
                    index: Some(0),
                    value: SummaryPath::root(1),
                }]))
            );
            for (_, summary) in r.values() {
                assert_eq!(summary.fields(), ArcHeapFields::UnknownSet);
                assert_eq!(summary.publications(), ArcHeapPublications::UnknownValues);
            }
            // The stateless field API stays conservative.
            assert_eq!(r["wrapper"].1.ret(), &ArcHeapReturn::UnknownValue);
        }
    }

    #[test]
    fn recursive_field_return_grows_to_top_and_baseless_cycle_does_not_expose_bottom() {
        let mut f = getter("rec", 0);
        f.blocks[0].instructions.push((
            ValueId(2),
            Instruction::CallStatic {
                symbol: "rec".into(),
                args: vec![Operand::Val(ValueId(1))],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        ));
        f.blocks[0].instructions.push((
            ValueId(3),
            Instruction::Phi {
                incoming: vec![
                    (BlockId(1), Operand::Val(ValueId(1))),
                    (BlockId(2), Operand::Val(ValueId(2))),
                ],
                ty: Type::Ref,
            },
            Type::Ref,
        ));
        // Both exits feed the Phi with real SSA predecessors.
        let mut body = f.blocks.remove(0);
        let phi = body.instructions.pop().unwrap();
        body.terminator = Terminator::CondBranch {
            cond: Operand::Val(ValueId(4)),
            then_block: BlockId(1),
            else_block: BlockId(2),
        };
        f.params.push((ValueId(4), "cond".into(), Type::I1));
        if let Instruction::CallStatic { args, .. } = &mut body.instructions[1].1 {
            args.push(Operand::Val(ValueId(4)));
        }
        f.blocks = vec![
            body,
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
                instructions: vec![phi],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(3)))),
            },
        ];
        let mut without_base = getter("sem_base", 0);
        without_base.blocks[0].instructions = vec![(
            ValueId(1),
            Instruction::CallStatic {
                symbol: "sem_base".into(),
                args: vec![Operand::Val(ValueId(0))],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        )];
        let mut m = Module::new();
        m.arc_memory = true;
        m.functions = vec![f, without_base];
        for order in [false, true] {
            if order {
                m.functions.reverse();
            }
            let r = super::super::calls::solve(&m, &HashSet::from(["rec", "sem_base"]), 3).unwrap();
            assert_eq!(r["rec"].1.return_paths, ReturnPaths::unknown());
            assert_eq!(r["sem_base"].1.return_paths, ReturnPaths::unknown());
        }
    }

    fn link(symbol: &str, inverted: bool) -> Function {
        Function {
            symbol: symbol.into(),
            name: symbol.into(),
            debug_info: None,
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
                        object: Operand::Val(ValueId(u32::from(inverted))),
                        index: 0,
                        value: Operand::Val(ValueId(u32::from(!inverted))),
                    },
                    Type::Void,
                )],
                terminator: Terminator::Return(None),
            }],
        }
    }

    fn expected_set(pairs: &[(usize, usize)]) -> WriteInclusions {
        WriteInclusions::Known(
            pairs
                .iter()
                .map(|&(object, val_ref)| ParameterWrite {
                    object: SummaryPath::root(object),
                    index: Some(0),
                    value: SummaryPath::root(val_ref),
                })
                .collect(),
        )
    }

    #[test]
    fn chained_reads_and_wrapper_preserve_argument_paths() {
        let mut f = link("profundo", false);
        f.blocks[0].instructions = vec![
            (
                ValueId(2),
                Instruction::GetField {
                    object: Operand::Val(ValueId(0)),
                    index: 0,
                },
                Type::Ref,
            ),
            (
                ValueId(3),
                Instruction::GetField {
                    object: Operand::Val(ValueId(2)),
                    index: 1,
                },
                Type::Ref,
            ),
            (
                ValueId(4),
                Instruction::GetField {
                    object: Operand::Val(ValueId(1)),
                    index: 2,
                },
                Type::Ref,
            ),
            (
                ValueId(5),
                Instruction::SetField {
                    object: Operand::Val(ValueId(3)),
                    index: 4,
                    value: Operand::Val(ValueId(4)),
                },
                Type::Void,
            ),
        ];
        let expected = |prefix: Vec<usize>| {
            WriteInclusions::Known(BTreeSet::from([ParameterWrite {
                object: SummaryPath {
                    origin: PathOrigin::Parameter(0),
                    fields: prefix,
                },
                index: Some(4),
                value: SummaryPath {
                    origin: PathOrigin::Parameter(1),
                    fields: vec![2],
                },
            }]))
        };
        assert_eq!(extract(&f, 8, |_| None).unwrap(), expected(vec![0, 1]));
        assert_eq!(
            extract(&f, 1, |_| None).unwrap(),
            WriteInclusions::UnknownValues
        );
        let mut wrapper = link("wrapper", false);
        wrapper.blocks[0].instructions = vec![
            (
                ValueId(2),
                Instruction::GetField {
                    object: Operand::Val(ValueId(0)),
                    index: 9,
                },
                Type::Ref,
            ),
            (
                ValueId(3),
                Instruction::CallStatic {
                    symbol: "profundo".into(),
                    args: vec![Operand::Val(ValueId(2)), Operand::Val(ValueId(1))],
                    ret_ty: Type::Void,
                },
                Type::Void,
            ),
        ];
        let mut m = Module::new();
        m.arc_memory = true;
        m.functions = vec![wrapper, f];
        for order in [false, true] {
            if order {
                m.functions.reverse();
            }
            let r = super::super::calls::solve(&m, &HashSet::from(["wrapper"]), 8).unwrap();
            assert_eq!(r["wrapper"].1.write_inclusions, expected(vec![9, 0, 1]));
            assert_eq!(r["wrapper"].1.fields(), ArcHeapFields::UnknownSet);
            assert_eq!(
                r["wrapper"].1.publications(),
                ArcHeapPublications::UnknownValues
            );
        }
    }

    #[test]
    fn growing_paths_in_scc_and_phi_reach_top_without_truncating() {
        let mut f = link("recursivo", false);
        f.blocks[0].instructions.push((
            ValueId(3),
            Instruction::GetField {
                object: Operand::Val(ValueId(0)),
                index: 0,
            },
            Type::Ref,
        ));
        f.blocks[0].instructions.push((
            ValueId(4),
            Instruction::CallStatic {
                symbol: "recursivo".into(),
                args: vec![Operand::Val(ValueId(3)), Operand::Val(ValueId(1))],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
        let mut m = Module::new();
        m.arc_memory = true;
        m.functions.push(f);
        let r = super::super::calls::solve(&m, &HashSet::from(["recursivo"]), 4).unwrap();
        assert_eq!(r["recursivo"].1.write_inclusions, WriteInclusions::Exceeded);
        let mut f = link("phi_crescente", false);
        f.params.push((ValueId(5), "cond".into(), Type::I1));
        f.blocks = vec![
            BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(1)),
            },
            BasicBlock {
                id: BlockId(1),
                instructions: vec![
                    (
                        ValueId(2),
                        Instruction::Phi {
                            incoming: vec![
                                (BlockId(0), Operand::Val(ValueId(0))),
                                (BlockId(1), Operand::Val(ValueId(3))),
                            ],
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(3),
                        Instruction::GetField {
                            object: Operand::Val(ValueId(2)),
                            index: 0,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(4),
                        Instruction::SetField {
                            object: Operand::Val(ValueId(2)),
                            index: 0,
                            value: Operand::Val(ValueId(1)),
                        },
                        Type::Void,
                    ),
                ],
                terminator: Terminator::CondBranch {
                    cond: Operand::Val(ValueId(5)),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                },
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![],
                terminator: Terminator::Return(None),
            },
        ];
        assert_eq!(
            extract(&f, 3, |_| None).unwrap(),
            WriteInclusions::UnknownValues
        );
    }

    #[test]
    fn parametric_write_does_not_promote_coverage_and_null_creates_no_edge() {
        let mut f = link("ligar", false);
        assert_eq!(extract(&f, 8, |_| None).unwrap(), expected_set(&[(0, 1)]));
        let r = super::super::summaries::extract(&f, 8).unwrap();
        assert_eq!(r.fields(), ArcHeapFields::UnknownSet);
        assert_eq!(r.publications(), ArcHeapPublications::UnknownValues);
        let Instruction::SetField { value, .. } = &mut f.blocks[0].instructions[0].1 else {
            unreachable!()
        };
        *value = Operand::Constant(Constant::Null);
        assert_eq!(extract(&f, 8, |_| None).unwrap(), expected_set(&[]));
        assert_eq!(
            super::super::summaries::extract(&f, 8).unwrap().fields(),
            ArcHeapFields::UnknownSet
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
            extract(&f, 8, |_| None).unwrap(),
            WriteInclusions::UnknownValues
        );
    }

    #[test]
    fn copy_and_local_return_aliases_instantiate_arguments() {
        let mut f = link("ligar", false);
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
            debug_info: None,
            params: vec![(ValueId(0), "x".into(), Type::Ref)],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        };
        let mut m = Module::new();
        m.arc_memory = true;
        m.functions = vec![f, id];
        let roots = HashSet::from(["ligar"]);
        let r = super::super::calls::solve(&m, &roots, 8).unwrap();
        assert_eq!(r["ligar"].1.write_inclusions, expected_set(&[(0, 1)]));
        // A representation equal in bits does not authorize converting an I64
        // argument into a Ref to sustain the store summary.
        m.functions[0].params[1].2 = Type::I64;
        assert!(super::super::calls::solve(&m, &roots, 8).is_err());
        m.functions[0].params[1].2 = Type::Ref;
        let Instruction::CallStatic { args, .. } = &mut m.functions[0].blocks[0].instructions[1].1
        else {
            unreachable!()
        };
        args[0] = Operand::Constant(Constant::Int(7));
        let r = super::super::calls::solve(&m, &roots, 8).unwrap();
        assert_eq!(
            r["ligar"].1.write_inclusions,
            WriteInclusions::UnknownValues
        );
    }

    #[test]
    fn phi_keeps_alias_product_and_whole_summary_budget() {
        let mut f = link("phi", false);
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
            extract(&f, 4, |_| None).unwrap(),
            expected_set(&[(0, 0), (0, 1), (1, 0), (1, 1)])
        );
        assert_eq!(extract(&f, 3, |_| None).unwrap(), WriteInclusions::Exceeded);
        // A null path does not become an allocation or a fresh argument.
        let Instruction::Phi { incoming, .. } = &mut f.blocks[3].instructions[0].1 else {
            unreachable!()
        };
        incoming[1].1 = Operand::Constant(Constant::Null);
        assert_eq!(extract(&f, 4, |_| None).unwrap(), expected_set(&[(0, 0)]));
    }

    #[test]
    fn chain_of_thousand_writers_composes_independent_of_body_order() {
        let mut m = Module::new();
        m.arc_memory = true;
        for n in 0..1000 {
            let mut f = link(&format!("s{n:04}"), false);
            if n != 999 {
                f.blocks[0].instructions[0].1 = Instruction::CallStatic {
                    symbol: format!("s{:04}", n + 1),
                    args: vec![Operand::Val(ValueId(1)), Operand::Val(ValueId(0))],
                    ret_ty: Type::Void,
                };
            }
            m.functions.push(f);
        }
        for order in [false, true] {
            if order {
                m.functions.reverse();
            }
            let r = super::super::calls::solve(&m, &HashSet::from(["s0000"]), 8).unwrap();
            assert_eq!(r.len(), 1000);
            for n in 0..1000 {
                let expected = if (999 - n) % 2 == 0 { (0, 1) } else { (1, 0) };
                assert_eq!(
                    r[&format!("s{n:04}")].1.write_inclusions,
                    expected_set(&[expected])
                );
            }
        }
    }

    #[test]
    fn writer_scc_composes_permutation_and_raises_budget_to_top() {
        let mut f = link("f", false);
        f.blocks[0].instructions.push((
            ValueId(3),
            Instruction::CallStatic {
                symbol: "g".into(),
                args: vec![Operand::Val(ValueId(1)), Operand::Val(ValueId(0))],
                ret_ty: Type::Void,
            },
            Type::Void,
        ));
        let mut g = link("g", false);
        g.blocks[0].instructions[0].1 = Instruction::CallStatic {
            symbol: "f".into(),
            args: vec![Operand::Val(ValueId(0)), Operand::Val(ValueId(1))],
            ret_ty: Type::Void,
        };
        let mut m = Module::new();
        m.arc_memory = true;
        m.functions = vec![f, g];
        for order in [false, true] {
            if order {
                m.functions.reverse();
            }
            let r = super::super::calls::solve(&m, &HashSet::from(["f"]), 8).unwrap();
            for (_, summary) in r.values() {
                assert_eq!(summary.write_inclusions, expected_set(&[(0, 1), (1, 0)]));
                assert_eq!(summary.fields(), ArcHeapFields::UnknownSet);
                assert_eq!(summary.publications(), ArcHeapPublications::UnknownValues);
            }
            let r = super::super::calls::solve(&m, &HashSet::from(["f"]), 1).unwrap();
            assert!(
                r.values()
                    .all(|(_, r)| r.write_inclusions == WriteInclusions::Exceeded)
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
        let r = super::super::calls::solve(&m, &HashSet::from(["f"]), 8).unwrap();
        let WriteInclusions::Known(positive_set) = expected_set(&[(0, 1), (1, 0)]) else {
            unreachable!()
        };
        for (_, summary) in r.values() {
            assert_eq!(
                summary.write_inclusions,
                WriteInclusions::Partial(positive_set.clone())
            );
            assert_eq!(summary.fields(), ArcHeapFields::UnknownSet);
            assert_eq!(summary.publications(), ArcHeapPublications::UnknownValues);
        }
    }
}
