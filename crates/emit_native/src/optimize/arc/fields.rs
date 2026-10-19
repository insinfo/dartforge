//! Physical field accesses by static origin or by a layout guard of the image.
//! Complete HIR allocations are the anchors; copies/moves/Phis carry
//! sets of sites, without strong update per instance. Publication,
//! opaque use or containment invalidate the whole site in the global proof. IDs
//! freshly produced in the same block keep the proof until the first exposure.
//! Ref reads without a static origin use the check of the real heap map;
//! the token plan protects the borrow before invalidating the edge.
//! Does not replace Dart late/type guards, heap summaries or versions/pins.

use super::{DartFunctionPlan, edges::next_val};
use crate::{hir::*, optimize::operands::operands};
use std::collections::{HashMap, HashSet};

mod class_cases;

impl ArcExternalAncestry {
    pub(crate) fn from_summaries(summaries: &[&crate::pruning::Summary]) -> Self {
        let mut facts = Self::default();
        for summary in summaries {
            for &cid in &summary.nominal_classes {
                let Ok(cid) = u32::try_from(cid) else {
                    facts.ambiguous_classes.insert(0);
                    return facts;
                };
                if !facts.nominal_classes.insert(cid) && cid != 0 {
                    facts.ambiguous_classes.insert(cid);
                }
            }
            for &cid in &summary.ambiguous_classes {
                let Ok(cid) = u32::try_from(cid) else {
                    facts.ambiguous_classes.insert(0);
                    return facts;
                };
                facts.ambiguous_classes.insert(cid);
            }
            for &(child, parent) in &summary.parent_edges {
                let (Ok(child), Ok(parent)) = (u32::try_from(child), u32::try_from(parent)) else {
                    facts.ambiguous_classes.insert(0);
                    return facts;
                };
                facts.parent_edges.push((child, parent));
            }
        }
        facts.parent_edges.sort_unstable();
        facts.parent_edges.dedup();
        facts
    }
}

#[derive(Default)]
pub(super) struct PhysicalAncestry {
    complete: HashSet<u32>,
    parents: HashMap<u32, Vec<u32>>,
}

impl PhysicalAncestry {
    pub(super) fn from_module(module: &Module) -> Self {
        if !module.arc_fixed_ancestry { return Self::default(); }
        let external = &module.arc_external_ancestry;
        let mut counts = HashMap::new();
        for class in &module.classes { *counts.entry(class.id).or_insert(0) += 1; }
        if counts.values().any(|&count| count != 1)
            || external.ambiguous_classes.contains(&0)
            || external.nominal_classes.iter().chain(external.parent_edges.iter().map(|(child, _)| child))
                .any(|cid| *cid != 0 && (counts.contains_key(cid) || module.pruning_class_ids.contains(cid))) {
            return Self::default();
        }
        let mut parents: HashMap<u32, Vec<u32>> = HashMap::new();
        for &(child, parent) in module.subtyping_edges.iter().chain(&external.parent_edges) {
            if child == 0 && parent != 0 { return Self::default(); }
            let entry = parents.entry(child).or_default();
            if !entry.contains(&parent) { entry.push(parent); }
        }
        let complete = module.pruning_class_ids.iter().copied().filter(|cid|
            i32::try_from(*cid).is_ok() && !module.pruning_ambiguous_class_ids.contains(cid)
                && !external.ambiguous_classes.contains(cid)
                && (*cid == 0 || (counts.get(cid) == Some(&1)
                    && parents.get(cid).is_some_and(|parents| parents.contains(&0)))))
            .chain(external.nominal_classes.iter().copied().filter(|cid|
                i32::try_from(*cid).is_ok() && !external.ambiguous_classes.contains(cid)
                    && !module.pruning_ambiguous_class_ids.contains(cid)
                    && (*cid == 0 || parents.get(cid).is_some_and(|parents| parents.contains(&0)))))
            .collect();
        Self { complete, parents }
    }

    fn excludes(&self, cid: u32, target: u32) -> bool {
        if target == 0 || !self.complete.contains(&target) { return false; }
        let mut active = HashSet::new();
        let mut complete = HashSet::new();
        let mut pending = vec![(cid, false)];
        while let Some((cid, exit)) = pending.pop() {
            if cid == target { return false; }
            if exit { active.remove(&cid); complete.insert(cid); continue; }
            if complete.contains(&cid) { continue; }
            if !self.complete.contains(&cid) || !active.insert(cid) || active.len() + complete.len() > 512 { return false; }
            pending.push((cid, true));
            if let Some(parents) = self.parents.get(&cid) {
                pending.extend(parents.iter().filter(|&&parent| cid != 0 || parent != 0).map(|&parent| (parent, false)));
            }
        }
        true
    }
}

// A fixed-image negative subtype query is pure. Folding only complete negative
// facts lets the ordinary constant/CFG pipeline remove impossible field paths.
pub(super) fn fold_fixed_subtype_tests(module: &mut Module) -> bool {
    if module.functions.iter().any(|function| function.symbol == "dartforge_is_subclass") {
        return false;
    }
    let ancestry = PhysicalAncestry::from_module(module);
    if ancestry.complete.is_empty() { return false; }
    let mut changed = false;
    for function in &mut module.functions {
        for block in &mut function.blocks {
            for (_, instruction, ty) in &mut block.instructions {
                let Instruction::CallRuntime { name, args, ret_ty: Type::I8 } = instruction else { continue };
                if *ty != Type::I8 || name != "dartforge_is_subclass" { continue; }
                let [(Operand::Constant(Constant::Int(cid)), Type::I64),
                     (Operand::Constant(Constant::Int(target)), Type::I64)] = args.as_slice() else { continue };
                let (Ok(cid), Ok(target)) = (u32::try_from(*cid), u32::try_from(*target)) else { continue };
                if i32::try_from(cid).is_ok() && i32::try_from(target).is_ok() && ancestry.excludes(cid, target) {
                    *instruction = Instruction::Const(Constant::Int(0));
                    changed = true;
                }
            }
        }
    }
    changed
}

// Carries eligibility for the physical guard, without proving ownership/layout.
// Each SCC requires a Ref/Null anchor and every external dependency checked.
fn propagate_ref_entries(f: &Function, entries: &mut HashSet<ValueId>) {
    let nodes: Vec<_> = f.blocks.iter().flat_map(|b| &b.instructions).filter_map(|(v, i, t)| {
        if *t != Type::Ref { return None; }
        let ops: Vec<_> = match i {
            Instruction::ArcCopy { value } | Instruction::ArcMove { value } => vec![value],
            Instruction::Phi { incoming, .. } if !incoming.is_empty() => incoming.iter().map(|(_, op)| op).collect(),
            _ => return None,
        };
        Some((*v, ops))
    }).collect();
    let idx_value: HashMap<_, _> = nodes.iter().enumerate().map(|(i, (v, _))| (*v, i)).collect();
    let mut graph = vec![Vec::new(); nodes.len()];
    let mut anchors = vec![false; nodes.len()];
    let mut invalid_set = vec![false; nodes.len()];
    for (i, (_, ops)) in nodes.iter().enumerate() {
        for op in ops {
            match op {
                Operand::Constant(Constant::Null) => anchors[i] = true,
                Operand::Val(v) if entries.contains(v) => anchors[i] = true,
                Operand::Val(v) if idx_value.contains_key(v) => graph[i].push(idx_value[v]),
                _ => invalid_set[i] = true,
            }
        }
    }
    let sccs = super::super::scc::components(&graph);
    let mut component = vec![usize::MAX; nodes.len()];
    for (i, scc) in sccs.iter().enumerate() { for &v in scc { component[v] = i; } }
    let mut valid_set = vec![false; sccs.len()];
    for (i, scc) in sccs.iter().enumerate() {
        let mut anchor = false; let mut valid = true;
        for &v in scc {
            anchor |= anchors[v]; valid &= !invalid_set[v];
            for &dep in &graph[v] {
                if component[dep] != i { valid &= valid_set[component[dep]]; anchor |= valid_set[component[dep]]; }
            }
        }
        valid_set[i] = valid && anchor;
        if valid_set[i] { for &v in scc { entries.insert(nodes[v].0); } }
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
struct Origin {
    sites: HashSet<ValueId>,
    unknown: bool,
}

fn origin(op: &Operand, origins: &HashMap<ValueId, Origin>) -> Origin {
    match op {
        Operand::Val(v) => origins.get(v).cloned().unwrap_or(Origin {
            unknown: true,
            ..Default::default()
        }),
        _ => Origin {
            unknown: true,
            ..Default::default()
        },
    }
}

fn origins(f: &Function, layouts: &HashMap<u32, Vec<Type>>, receivers: Option<&HashMap<ValueId, u32>>) -> (HashMap<ValueId, Origin>, HashMap<ValueId, u32>) {
    let mut map = HashMap::new();
    let mut classes = HashMap::new();
    for (v, _, _) in &f.params {
        map.insert(
            *v,
            Origin {
                unknown: true,
                ..Default::default()
            },
        );
    }
    if let Some(receivers) = receivers {
        for (v, cid) in receivers {
            classes.insert(*v, *cid);
            map.insert(*v, Origin { sites: HashSet::from([*v]), unknown: false });
        }
    }
    for (v, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
        let o = match inst {
            Instruction::AllocObject { class_id, .. } => {
                classes.insert(*v, *class_id);
                Origin {
                    sites: HashSet::from([*v]),
                    unknown: false,
                }
            }
            Instruction::CallRuntime { name, args, ret_ty }
                if matches!(name.as_str(), "dartforge_object_new" | "dartforge_arc_object_owned_v1")
                    && *ret_ty == Type::Ref => {
                // The zeroed factory also fixes the physical class. Do not infer
                // size/layout from the width nor from non-constant parameters.
                let class = match args.as_slice() {
                    [(Operand::Constant(Constant::Int(cid)), Type::I64),
                     (Operand::Constant(Constant::Int(n)), Type::I64)] => {
                        u32::try_from(*cid).ok().filter(|cid| i32::try_from(*cid).is_ok())
                            .filter(|cid| usize::try_from(*n).ok().is_some_and(|n|
                                u16::try_from(n).is_ok() && layouts.get(cid).is_some_and(|l| l.len() == n)))
                    }
                    _ => None,
                };
                if let Some(cid) = class {
                    classes.insert(*v, cid);
                    Origin { sites: HashSet::from([*v]), unknown: false }
                } else {
                    Origin { unknown: true, ..Default::default() }
                }
            }
            Instruction::ArcCopy { .. } | Instruction::ArcMove { .. } | Instruction::Phi { .. } => {
                Origin::default()
            }
            _ => Origin {
                unknown: true,
                ..Default::default()
            },
        };
        map.insert(*v, o);
    }
    loop {
        let mut changed = false;
        for (v, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            let ops = match inst {
                Instruction::ArcCopy { value } | Instruction::ArcMove { value } => vec![value],
                Instruction::Phi { incoming, .. } => incoming.iter().map(|(_, o)| o).collect(),
                _ => continue,
            };
            let mut new = Origin::default();
            for op in ops {
                let o = origin(op, &map);
                new.sites.extend(o.sites);
                new.unknown |= o.unknown;
            }
            if map[v] != new {
                map.insert(*v, new);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    (map, classes)
}

fn representation(
    o: &Origin,
    index: usize,
    classes: &HashMap<ValueId, u32>,
    layouts: &HashMap<u32, Vec<Type>>,
    escape: &HashSet<ValueId>,
) -> Result<Type, String> {
    if o.unknown || o.sites.is_empty() || !o.sites.is_disjoint(escape) {
        return Err("receiver without an unpublished local origin".into());
    }
    known_representation(o, index, classes, layouts)
}

// The unknown part of the join requires a physical guard. The nominal part
// stays mandatory: do not hide a contradictory layout behind the guard.
fn known_representation(
    o: &Origin, index: usize, classes: &HashMap<ValueId, u32>,
    layouts: &HashMap<u32, Vec<Type>>,
) -> Result<Type, String> {
    let mut repr = None;
    for site in &o.sites {
        let t = layouts
            .get(&classes[site])
            .and_then(|l| l.get(index))
            .ok_or("uncertified class/layout/field index")?;
        if repr.is_some_and(|previous| previous != *t) {
            return Err("different representations at the join of receivers".into());
        }
        repr = Some(*t);
    }
    repr.ok_or_else(|| "receiver without a nominal origin".into())
}

const WRITE_SCALAR: &str = "dartforge_arc_write_field_scalar_checked_v1";
const READ_SCALAR: &str = "dartforge_arc_read_field_scalar_checked_v1";
const WRITE_CHECKED: &str = "dartforge_arc_write_field_ref_checked_v1";

fn field_layouts(layouts: &HashMap<u32, Vec<Type>>, idx_value: usize, repr: Type) -> Vec<i64> {
    let mut pairs: Vec<_> = layouts.iter().filter(|(cid, fields)|
        i32::try_from(**cid).is_ok() && u16::try_from(fields.len()).is_ok()
        && fields.get(idx_value) == Some(&repr)).collect();
    pairs.sort_by_key(|(cid, _)| **cid);
    pairs.into_iter().flat_map(|(cid, fields)| [i64::from(*cid), fields.len() as i64]).collect()
}

fn scalar_code(repr: Type) -> i64 {
    match repr { Type::I64 => 1, Type::F64 => 2, Type::I1 => 3, Type::I8 => 4,
        _ => unreachable!("unchecked scalar representation") }
}

// An imported Ptr, an alias or a computed size does not prove extent/immutability.
// The certified image also rechecks the vector against the current layouts.
pub(super) fn check_call(
    f: &Function, inst: &Instruction, layouts: Option<&HashMap<u32, Vec<Type>>>,
) -> Result<(), String> {
    let Instruction::CallRuntime { name, args, ret_ty } = inst else { return Ok(()); };
    if !matches!(name.as_str(), WRITE_CHECKED | WRITE_SCALAR | READ_SCALAR) { return Ok(()); }
    let error = || format!("checked field in {}: layouts vector without provenance/bounds", f.symbol);
    let (idx_value, vector, pairs, repr) = match (name.as_str(), args.as_slice(), ret_ty) {
        (WRITE_CHECKED, [(_, Type::Ref), (Operand::Constant(Constant::Int(i)), Type::I64),
            (_, Type::Ref), (Operand::Val(v), Type::Ptr), (Operand::Constant(Constant::Int(n)), Type::I64)], Type::Void) => (*i, *v, *n, Type::Ref),
        (WRITE_SCALAR, [(_, Type::Ref), (Operand::Constant(Constant::Int(i)), Type::I64),
            (_, Type::I64), (Operand::Val(v), Type::Ptr), (Operand::Constant(Constant::Int(n)), Type::I64),
            (Operand::Constant(Constant::Int(r)), Type::I64)], Type::Void)
        | (READ_SCALAR, [(_, Type::Ref), (Operand::Constant(Constant::Int(i)), Type::I64),
            (Operand::Val(v), Type::Ptr), (Operand::Constant(Constant::Int(n)), Type::I64),
            (Operand::Constant(Constant::Int(r)), Type::I64)], Type::I64) => {
                let repr = match r { 1 => Type::I64, 2 => Type::F64, 3 => Type::I1, 4 => Type::I8, _ => return Err(error()) };
                (*i, *v, *n, repr)
            }
        _ => return Err(error()),
    };
    let idx_value = usize::try_from(idx_value).map_err(|_| error())?;
    let Some((_, Instruction::ConstArray(values), Type::Ptr)) = f.blocks.iter()
        .flat_map(|b| &b.instructions).find(|(v, _, _)| *v == vector) else { return Err(error()); };
    if values.is_empty() || !values.len().is_multiple_of(2)
        || usize::try_from(pairs).ok() != Some(values.len() / 2)
        || values.as_chunks::<2>().0.iter().any(|p| i32::try_from(p[0]).is_err() || p[0] < 0
            || u16::try_from(p[1]).is_err() || usize::try_from(p[1]).ok().is_none_or(|n| idx_value >= n))
        || values.as_chunks::<2>().0.iter().map(|p| p[0]).collect::<Vec<_>>().windows(2).any(|p| p[0] >= p[1])
    { return Err(error()); }
    if let Some(layouts) = layouts && *values != field_layouts(layouts, idx_value, repr) {
        return Err(format!("checked field in {}: stale/divergent layouts", f.symbol));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn prepare(
    f: &mut Function,
    layouts: &HashMap<u32, Vec<Type>>,
    plan: &mut DartFunctionPlan,
    receivers: Option<&HashMap<ValueId, u32>>,
    preserve_fields: &HashSet<String>,
) -> Result<(), String> {
    prepare_with_class_cases(f, layouts, plan, receivers, preserve_fields, false)
}

#[cfg(test)]
pub(super) fn prepare_with_class_cases(
    f: &mut Function,
    layouts: &HashMap<u32, Vec<Type>>,
    plan: &mut DartFunctionPlan,
    receivers: Option<&HashMap<ValueId, u32>>,
    preserve_fields: &HashSet<String>,
    canonical_runtime: bool,
) -> Result<(), String> {
    prepare_with_ancestry(f, layouts, plan, receivers, preserve_fields, canonical_runtime, None)
}

pub(super) fn prepare_with_ancestry(
    f: &mut Function,
    layouts: &HashMap<u32, Vec<Type>>,
    plan: &mut DartFunctionPlan,
    receivers: Option<&HashMap<ValueId, u32>>,
    preserve_fields: &HashSet<String>,
    canonical_runtime: bool,
    ancestry: Option<&PhysicalAncestry>,
) -> Result<(), String> {
    // Const Null in SSA has no token. At a Phi entry, use the literal
    // shape for the nullable protocol, without confusing Const Int(0) or
    // Bool(false). The definition stays and its contract is still checked.
    let nulls: HashSet<_> = f.blocks.iter().flat_map(|b| &b.instructions)
        .filter_map(|(v, i, t)| (*t == Type::Ref && matches!(i, Instruction::Const(Constant::Null))).then_some(*v)).collect();
    for (_, i, _) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
        if let Instruction::Phi { incoming, ty: Type::Ref } = i {
            for (_, op) in incoming {
                if matches!(op, Operand::Val(v) if nulls.contains(v)) { *op = Operand::Constant(Constant::Null); }
            }
        }
    }
    for (_, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
        check_call(f, inst, Some(layouts))?;
    }
    if !f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .any(|(_, i, _)| {
            matches!(
                i,
                Instruction::GetField { .. } | Instruction::SetField { .. }
            )
        })
    {
        return Ok(());
    }
    let (origins, classes) = origins(f, layouts, receivers);
    let ancestry = ancestry.filter(|ancestry| !ancestry.complete.is_empty());
    let class_cases = class_cases::Cases::with_ancestry(f, canonical_runtime, ancestry);
    // Borrowed entries must already be valid instances by Dart
    // semantics. A local factory without a layout may still contain zeroed fields
    // without a Ref mark; do not confuse it with an open entry receiver.
    // These values may be checked receivers, without claiming a class or
    // freshness. Each read and call still has to go through the preparation
    // and the ownership contract; a failure discards the whole module.
    let mut ref_entries: HashSet<_> = f.params.iter()
        .filter(|(_, _, ty)| *ty == Type::Ref).map(|(v, _, _)| *v).collect();
    for (v, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
        if *ty != Type::Ref { continue; }
        // Null may accompany a valid Ref at a join, including on the
        // pending path. The access guard still rejects null;
        // this assigns no class nor turns scalar bits into a Ref.
        let derived = match inst {
            Instruction::ArcLoadStrong { .. } => true,
            Instruction::CallRuntime { name, ret_ty: Type::Ref, .. } if matches!(name.as_str(), super::vectors::READ_ARGUMENT | "dartforge_arc_list_ref_owned_v1") => true,
            Instruction::AllocObject { .. } => true,
            Instruction::CallRuntime { name, ret_ty: Type::Ref, .. }
                if matches!(name.as_str(), "dartforge_object_new" | "dartforge_arc_object_owned_v1") => classes.contains_key(v),
            // The catalog checks the Ref result, the arity and the declared types.
            // This only authorizes the physical guard; the transaction still rechecks
            // SSA types, native provenance, tokens and Dart boundaries. An
            // opaque extern or a scalar result does not become a reference.
            Instruction::CallRuntime { ret_ty: Type::Ref, .. } =>
                super::contracts::runtime_call_contract(inst).is_ok(),
            Instruction::Box { from: Type::I64 | Type::F64 | Type::I1 | Type::I8, .. }
            | Instruction::GetField { .. }
            | Instruction::EnvGet { .. }
            | Instruction::CellGet { .. }
            | Instruction::Const(Constant::Null)
            | Instruction::CallStatic { ret_ty: Type::Ref, .. }
            | Instruction::CallSelector { .. }
            | Instruction::CallInterface { ret_ty: Type::Ref, .. }
            | Instruction::CallDynamic { ret_ty: Type::Ref, .. }
            | Instruction::CallClosure { ret_ty: Type::Ref, .. } => true,
            Instruction::TypedCall { ret: Type::Ref, .. } => super::typed_set::check(f, inst, *ty)?,
            _ => false,
        };
        if derived { ref_entries.insert(*v); }
    }
    propagate_ref_entries(f, &mut ref_entries);
    let mut escape = HashSet::new();
    let mut exposures = HashMap::new();
    for (v, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
        let mut exposed = HashSet::new();
        let mut expose = |op: &Operand| {
            exposed.extend(origin(op, &origins).sites);
        };
        match inst {
            Instruction::GetField { .. }
            | Instruction::ArcCopy { .. }
            | Instruction::ArcMove { .. }
            | Instruction::ArcDrop { .. }
            | Instruction::Phi { .. } => {}
            Instruction::SetField { value, .. } => expose(value),
            Instruction::AllocObject { fields, .. } => fields.iter().for_each(&mut expose),
            Instruction::CallRuntime { name, args, .. } => {
                let safe = matches!(
                    name.as_str(),
                    "dartforge_arc_retain"
                        | "dartforge_object_new"
                        | "dartforge_arc_release"
                        | "dartforge_arc_object_owned_v1"
                        | "dartforge_arc_box_int_owned_v1"
                        | "dartforge_arc_box_double_owned_v1"
                        | "dartforge_exception_pending"
                        | "dartforge_rti_recipe"
                        | "dartforge_rti_eval"
                );
                let field = match name.as_str() {
                    "dartforge_arc_read_field_ref_v1" => Some(true),
                    "dartforge_arc_read_field_scalar_v1" => Some(false),
                    _ => None,
                };
                let mut safe_field = false;
                if let Some(refs) = field
                    && let [receiver, idx_value] = args.as_slice()
                    && let Operand::Constant(Constant::Int(n)) = idx_value.0
                    && let Ok(index) = usize::try_from(n)
                    && let Ok(repr) = representation(
                        &origin(&receiver.0, &origins),
                        index,
                        &classes,
                        layouts,
                        &HashSet::new(),
                    )
                {
                    safe_field = (repr == Type::Ref) == refs;
                }
                // Defining RTI may compact lists. Only the physical origin of
                // non-list instances closes the layout obligation here.
                let define_instance = name == "dartforge_rti_set" && args.len() == 2 && {
                    let o = origin(&args[0].0, &origins);
                    !o.unknown && !o.sites.is_empty() && o.sites.iter().all(|s|
                        i32::try_from(classes[s]).ok().is_some_and(|cid|
                            !dartforge_runtime::layout::cid::is_list(cid)))
                };
                if !safe && !safe_field && !define_instance {
                    // A native bits setter does not certify the logical
                    // representation (bool/byte). Only a typed SetField keeps the proof.
                    args.iter().for_each(|(o, _)| expose(o));
                    // The absence of Dart in the ownership catalog does not prove
                    // the absence of layout mutation, for instance on reload.
                    // Only the operations enumerated above have a closed contract here.
                    exposed.extend(classes.keys().copied());
                }
            }
            Instruction::CallStatic { symbol, args, .. } if preserve_fields.contains(symbol) && args.is_empty() => {}
            Instruction::CallStatic { .. }
            | Instruction::CallInterface { .. }
            | Instruction::CallClosure { .. }
            | Instruction::CallClosureForward { .. }
            | Instruction::TypedCall { .. } => exposed.extend(classes.keys().copied()),
            _ => operands(inst, &mut expose),
        }
        escape.extend(&exposed);
        exposures.insert(*v, exposed);
    }
    let types: HashMap<_, _> = f
        .params
        .iter()
        .map(|(v, _, t)| (*v, *t))
        .chain(
            f.blocks
                .iter()
                .flat_map(|b| b.instructions.iter().map(|(v, _, t)| (*v, *t))),
        )
        .collect();
    let typ = |op: &Operand| match op {
        Operand::Val(v) => types.get(v).copied(),
        Operand::Constant(Constant::Int(_)) => Some(Type::I64),
        Operand::Constant(Constant::Double(_)) => Some(Type::F64),
        Operand::Constant(Constant::Bool(_)) => Some(Type::I1),
        Operand::Constant(Constant::Null) => Some(Type::Ref),
        _ => None,
    };
    let parameters: HashSet<ValueId> = receivers.into_iter().flat_map(|m| m.keys().copied()).collect();
    let entry = f.blocks.first().map(|b| b.id);
    let entry_without_return = !f.blocks.iter().any(|b| {
        super::super::cfg::successors(&b.terminator).into_iter().any(|dest| Some(dest) == entry)
    });
    // For certified parameters, use only the entry prefix that
    // dominates the first call/publication. Does not assign freshness to the parameter,
    // nor reuse that proof in later blocks or in a loop iteration.
    let local_globals: HashSet<_> = escape.difference(&parameters).copied().collect();
    let mut next_item = next_val(f, plan);
    for b in &mut f.blocks {
        let mut invalid_set = if Some(b.id) == entry && entry_without_return {
            HashSet::new()
        } else { parameters.clone() };
        // IDs actually produced in this block, not abstract sites of
        // Phis that may represent objects of a previous iteration.
        let mut fresh_set: HashSet<ValueId> = HashSet::new();
        let mut instructions = Vec::new();
        for (v, inst, ty) in &b.instructions {
            let (object, index, value) = match inst {
                Instruction::GetField { object, index } => (object, *index, None),
                Instruction::SetField {
                    object,
                    index,
                    value,
                } => (object, *index, Some(value)),
                _ => {
                    fresh_set.retain(|id| origin(&Operand::Val(*id), &origins).sites.is_disjoint(&exposures[v]));
                    if classes.contains_key(v) && !parameters.contains(v) {
                        fresh_set.insert(*v);
                    } else if let Instruction::ArcCopy { value } | Instruction::ArcMove { value } = inst
                        && matches!(value, Operand::Val(of) if fresh_set.contains(of)) {
                        fresh_set.insert(*v);
                    }
                    invalid_set.extend(exposures[v].intersection(&parameters).copied());
                    instructions.push((*v, inst.clone(), *ty));
                    continue;
                }
            };
            let current_ones: HashSet<_> = if matches!(object, Operand::Val(id) if fresh_set.contains(id)) {
                // A future publication cannot affect the earlier write.
                // Do not revive the proof after an exposure that already ran.
                HashSet::new()
            } else { local_globals.union(&invalid_set).copied().collect() };
            let mut o = origin(object, &origins);
            class_cases.refine(&mut o, object, b.id, &classes);
            // Without a nominal anchor, the Ref accessor checks the receiver, the index and the
            // bitmap before borrowing. Do not reinterpret scalar bits,
            // nor ignore contradictions of a known origin. Open writes
            // require the additional check of the declared layouts of the image.
            let open_entry = o.unknown && o.sites.is_empty()
                && matches!(object, Operand::Val(v) if ref_entries.contains(v));
            // The exposure loses the static proof, not the owner. Checking the
            // nominal representation before opting for the physical guard avoids
            // accepting contradictions of class, size or type in the HIR itself.
            let exposed_nominal = !o.unknown && !o.sites.is_empty()
                && !o.sites.is_disjoint(&current_ones);
            let mixed_nominal = o.unknown && !o.sites.is_empty()
                && matches!(object, Operand::Val(v) if ref_entries.contains(v));
            let nominal = if exposed_nominal || mixed_nominal {
                Some(known_representation(&o, index, &classes, layouts)
                    .map_err(|e| format!("ARC field in {} v{}: {e}", f.symbol, v.0))?)
            } else { None };
            let checked_receiver = open_entry || exposed_nominal || mixed_nominal;
            let checked_ref_read = value.is_none() && *ty == Type::Ref && checked_receiver;
            let checked_ref_write = value.is_some_and(|val_ref| typ(val_ref) == Some(Type::Ref))
                && *ty == Type::Void && checked_receiver;
            let scalar = if let Some(value) = value { typ(value) } else { Some(*ty) };
            let checked_scalar = checked_receiver && matches!(scalar, Some(Type::I64 | Type::F64 | Type::I1 | Type::I8));
            let repr = if let Some(nominal) = nominal { nominal }
            else if checked_ref_read || checked_ref_write {
                Type::Ref
            } else if checked_scalar {
                scalar.unwrap()
            } else {
                representation(&o, index, &classes, layouts, &current_ones)
                    .map_err(|e| format!("ARC field in {} v{}: {e}", f.symbol, v.0))?
            };
            let idx_value = i64::try_from(index)
                .map_err(|_| format!("ARC field in {} v{}: index outside the ABI", f.symbol, v.0))?;
            if typ(object) != Some(Type::Ref)
                || (value.is_none() && *ty != repr)
                || value.is_some_and(|o| typ(o) != Some(repr) || *ty != Type::Void)
                || plan.tokens.pendings.contains_key(v)
                || plan.tables.invocations.contains_key(v)
            {
                return Err(format!(
                    "ARC field in {} v{}: incompatible type or guard",
                    f.symbol, v.0
                ));
            }
            fresh_set.retain(|id| origin(&Operand::Val(*id), &origins).sites.is_disjoint(&exposures[v]));
            invalid_set.extend(exposures[v].intersection(&parameters).copied());
            let narrow = !matches!(repr, Type::I64 | Type::Ref);
            let auxiliary = if narrow {
                let a = ValueId(
                    u32::try_from(next_item)
                        .map_err(|_| "SSA IDs exhausted while preparing ARC fields")?,
                );
                next_item += 1;
                if let Some(limits) = plan.scopes.before.remove(v) {
                    plan.scopes.before.insert(a, limits);
                }
                Some(a)
            } else {
                None
            };
            let mut args = vec![
                (object.clone(), Type::Ref),
                (Operand::Constant(Constant::Int(idx_value)), Type::I64),
            ];
            if let Some(value) = value {
                let bits = if let Some(a) = auxiliary {
                    let conversion = if repr == Type::F64 {
                        Instruction::Bitcast {
                            op: value.clone(),
                            to: Type::I64,
                        }
                    } else {
                        Instruction::ZExt {
                            op: value.clone(),
                            from: repr,
                            to: Type::I64,
                        }
                    };
                    instructions.push((a, conversion, Type::I64));
                    Operand::Val(a)
                } else {
                    value.clone()
                };
                args.push((
                    bits,
                    if repr == Type::Ref {
                        Type::Ref
                    } else {
                        Type::I64
                    },
                ));
                if checked_ref_write || checked_scalar {
                    let values = field_layouts(layouts, index, repr);
                    if values.is_empty() {
                        return Err(format!("ARC field in {} v{}: no matching layout declared", f.symbol, v.0));
                    }
                    let vector = ValueId(u32::try_from(next_item).map_err(|_| "IDs SSA esgotados nos layouts Ref")?);
                    next_item += 1;
                    args.push((Operand::Val(vector), Type::Ptr));
                    args.push((Operand::Constant(Constant::Int((values.len() / 2) as i64)), Type::I64));
                    if let Some(limits) = plan.scopes.before.remove(v) {
                        plan.scopes.before.insert(vector, limits);
                    }
                    instructions.push((vector, Instruction::ConstArray(values), Type::Ptr));
                    if checked_scalar {
                        args.push((Operand::Constant(Constant::Int(scalar_code(repr))), Type::I64));
                    }
                }
                instructions.push((
                    *v,
                    Instruction::CallRuntime {
                        name: if checked_scalar {
                            WRITE_SCALAR
                        } else if checked_ref_write {
                            WRITE_CHECKED
                        } else if repr == Type::Ref {
                            "dartforge_arc_write_field_ref_v1"
                        } else {
                            "dartforge_arc_write_field_scalar_v1"
                        }
                        .into(),
                        args,
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ));
            } else {
                if checked_scalar {
                    let values = field_layouts(layouts, index, repr);
                    if values.is_empty() { return Err(format!("ARC field in {} v{}: no matching layout declared", f.symbol, v.0)); }
                    let vector = ValueId(u32::try_from(next_item).map_err(|_| "IDs SSA esgotados nos layouts escalares")?);
                    next_item += 1;
                    args.push((Operand::Val(vector), Type::Ptr));
                    args.push((Operand::Constant(Constant::Int((values.len() / 2) as i64)), Type::I64));
                    args.push((Operand::Constant(Constant::Int(scalar_code(repr))), Type::I64));
                    instructions.push((vector, Instruction::ConstArray(values), Type::Ptr));
                }
                let ret_ty = if repr == Type::Ref {
                    Type::Ref
                } else {
                    Type::I64
                };
                instructions.push((
                    auxiliary.unwrap_or(*v),
                    Instruction::CallRuntime {
                        name: if checked_scalar {
                            READ_SCALAR
                        } else if repr == Type::Ref {
                            "dartforge_arc_read_field_ref_v1"
                        } else {
                            "dartforge_arc_read_field_scalar_v1"
                        }
                        .into(),
                        args,
                        ret_ty,
                    },
                    ret_ty,
                ));
                if let Some(a) = auxiliary {
                    let conversion = if repr == Type::F64 {
                        Instruction::Bitcast {
                            op: Operand::Val(a),
                            to: repr,
                        }
                    } else {
                        Instruction::Trunc {
                            op: Operand::Val(a),
                            from: Type::I64,
                            to: repr,
                        }
                    };
                    instructions.push((*v, conversion, repr));
                }
            }
        }
        b.instructions = instructions;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::optimize::arc::*;

    #[test]
    fn ref_entries_in_scc_require_anchor_and_all_valid_dependencies() {
        for case in 0..6 {
            let val = |n| Operand::Val(ValueId(n));
            let anchor = match case { 1 => Operand::Constant(Constant::Null), 2 => Operand::Constant(Constant::Int(0)),
                3 => val(99), _ => val(0) };
            let mut incoming = vec![(BlockId(0), val(4))];
            if case != 4 { incoming.push((BlockId(0), anchor)); }
            if case == 5 { incoming.push((BlockId(0), val(99))); }
            let f = Function { symbol: "grafo".into(), name: "grafo".into(), debug_info: None,
                params: vec![(ValueId(0), "ref".into(), Type::Ref)], return_ty: Type::Void,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(3), Instruction::Phi { ty: Type::Ref, incoming }, Type::Ref),
                    (ValueId(4), Instruction::Phi { ty: Type::Ref, incoming: vec![(BlockId(0), val(3))] }, Type::Ref),
                    (ValueId(5), Instruction::Phi { ty: Type::Ref, incoming: vec![(BlockId(0), val(6))] }, Type::Ref),
                    (ValueId(6), Instruction::Phi { ty: Type::Ref, incoming: vec![(BlockId(0), val(5)), (BlockId(0), val(3)), (BlockId(0), val(0))] }, Type::Ref),
                    (ValueId(7), Instruction::ArcCopy { value: val(6) }, Type::Ref),
                ], terminator: Terminator::Return(None) }] };
            let mut entries = HashSet::from([ValueId(0)]);
            super::propagate_ref_entries(&f, &mut entries);
            for id in 3..=7 { assert_eq!(entries.contains(&ValueId(id)), case < 2, "caso {case} v{id}"); }
        }
    }

    #[test]
    fn ref_phi_with_null_does_not_admit_scalar_zero_as_receiver() {
        for (constant, valid) in [(Constant::Null, true), (Constant::Int(0), false), (Constant::Bool(false), false)] {
            let (mut m, mut plans) = module(vec![Type::I1], vec![Operand::Constant(Constant::Bool(true))]);
            let f = &mut m.functions[0];
            f.params = vec![(ValueId(0), "receiver".into(), Type::Ref), (ValueId(7), "cond".into(), Type::I1)];
            f.return_ty = Type::I1;
            f.blocks = vec![
                BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::CondBranch {
                    cond: Operand::Val(ValueId(7)), then_block: BlockId(1), else_block: BlockId(2) } },
                BasicBlock { id: BlockId(1), instructions: vec![
                    (ValueId(8), Instruction::ArcCopy { value: Operand::Val(ValueId(0)) }, Type::Ref),
                ], terminator: Terminator::Branch(BlockId(3)) },
                BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Branch(BlockId(3)) },
                BasicBlock { id: BlockId(3), instructions: vec![
                    (ValueId(9), Instruction::Phi { ty: Type::Ref, incoming: vec![
                        (BlockId(1), Operand::Val(ValueId(8))), (BlockId(2), Operand::Constant(constant.clone())),
                    ] }, Type::Ref),
                    (ValueId(10), Instruction::GetField { object: Operand::Val(ValueId(9)), index: 0 }, Type::I1),
                ], terminator: Terminator::Return(Some(Operand::Val(ValueId(10)))) },
            ];
            // The SSA shape also appears in the lowering of operators and casts.
            f.blocks[2].instructions.push((ValueId(11), Instruction::Const(constant.clone()),
                if valid { Type::Ref } else { Type::I64 }));
            if let Instruction::Phi { incoming, .. } = &mut f.blocks[3].instructions[0].1 {
                incoming[1].1 = Operand::Val(ValueId(11));
            }
            let before = format!("{m:?}/{plans:?}");
            let result = prepare_arc_dart_module(&mut m, &mut plans);
            assert_eq!(result.is_ok(), valid, "{result:?}");
            if valid {
                assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
            } else { assert_eq!(before, format!("{m:?}/{plans:?}")); }
        }
    }

    fn module(
        layout: Vec<Type>,
        fields: Vec<Operand>,
    ) -> (Module, HashMap<String, DartFunctionPlan>) {
        let mut m = Module::new();
        m.arc_memory = true;
        m.classes.push(ClassDef {
            id: 123,
            name: "C".into(),
            field_count: fields.len(),
            vtable: vec![],
            to_string_symbol: None,
        });
        m.arc_field_layouts.insert(123, layout);
        m.functions.push(Function {
            symbol: "campos".into(),
            name: "campos".into(),
            debug_info: None,
            params: vec![],
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(0),
                    Instruction::AllocObject {
                        class_id: 123,
                        fields,
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(None),
            }],
        });
        (
            m,
            HashMap::from([("campos".into(), DartFunctionPlan::default())]),
        )
    }

    #[test]
    fn audited_runtime_ref_checks_fields_without_promoting_opaque_extern_or_layoutless_factory() {
        for case in 0..9 {
            let (mut m, mut plans) = module(vec![Type::I64], vec![Operand::Constant(Constant::Int(0))]);
            let f = &mut m.functions[0];
            f.params = vec![(ValueId(0), "closure".into(), Type::Ref)];
            let (name_str, args) = match case {
                0 => ("dartforge_arc_closure_context_v1", vec![(Operand::Val(ValueId(0)), Type::Ref)]),
                1 => ("dartforge_arc_read_field_ref_v1", vec![(Operand::Val(ValueId(0)), Type::Ref), (Operand::Constant(Constant::Int(0)), Type::I64)]),
                2 => ("opaca", vec![(Operand::Val(ValueId(0)), Type::Ref)]),
                3 => ("dartforge_arc_closure_context_v1", vec![(Operand::Val(ValueId(0)), Type::I64)]),
                4 => ("dartforge_arc_object_owned_v1", vec![(Operand::Constant(Constant::Int(999)), Type::I64), (Operand::Constant(Constant::Int(1)), Type::I64)]),
                5 => ("dartforge_arc_read_field_scalar_v1", vec![(Operand::Val(ValueId(0)), Type::Ref), (Operand::Constant(Constant::Int(0)), Type::I64)]),
                6 => ("dartforge_arc_exception_owned_v1", vec![]),
                8 => ("dartforge_arc_read_field_late_ref_v1", vec![(Operand::Val(ValueId(0)), Type::Ref), (Operand::Constant(Constant::Int(0)), Type::I64)]),
                _ => ("dartforge_arc_closure_context_v1", vec![(Operand::Val(ValueId(0)), Type::Ref)]),
            };
            if case == 7 { f.params[0].2 = Type::I64; }
            f.blocks[0].instructions = vec![
                (ValueId(1), Instruction::CallRuntime { name: name_str.into(), args, ret_ty: Type::Ref }, Type::Ref),
                (ValueId(2), Instruction::GetField { object: Operand::Val(ValueId(1)), index: 0 }, Type::I64),
            ];
            f.return_ty = Type::I64;
            f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(2))));
            let before = format!("{m:?}"); let plans_before = plans.clone();
            let r = prepare_arc_dart_module(&mut m, &mut plans);
            if case < 2 || case == 6 || case == 8 {
                r.unwrap();
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(ir.contains("call i64 @dartforge_arc_read_field_scalar_checked_v1"));
                assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
            } else {
                assert!(r.is_err(), "caso {case}");
                assert_eq!(format!("{m:?}"), before); assert_eq!(plans, plans_before);
            }
        }
    }

    #[test]
    fn getter_return_has_checked_layout_without_inventing_class() {
        for valid_dest in [false, true] {
            let (mut m, mut plans) = module(vec![Type::Ref], vec![Operand::Constant(Constant::Null)]);
            m.functions[0].params = vec![(ValueId(0), "this".into(), Type::Ref)];
            m.functions[0].blocks[0].instructions = vec![
                (ValueId(1), Instruction::CallStatic { symbol: "getter".into(),
                    args: vec![Operand::Val(ValueId(0))], ret_ty: Type::Ref }, Type::Ref),
                (ValueId(2), Instruction::SetField { object: Operand::Val(ValueId(1)), index: 0,
                    value: Operand::Constant(Constant::Null) }, Type::Void),
            ];
            if valid_dest {
                m.functions.push(Function { symbol: "getter".into(), name: "getter".into(), debug_info: None,
                    params: vec![(ValueId(0), "this".into(), Type::Ref)], return_ty: Type::Ref,
                    blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                        (ValueId(1), Instruction::GetField { object: Operand::Val(ValueId(0)), index: 0 }, Type::Ref),
                    ], terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }] });
                let mut p = DartFunctionPlan::default();
                p.tokens.ret = TokenReturn::Owned;
                plans.insert("getter".into(), p);
            }
            let before = format!("{m:?}/{plans:?}");
            let result = prepare_arc_dart_module(&mut m, &mut plans);
            if valid_dest {
                result.unwrap();
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(ir.contains("call void @dartforge_arc_write_field_ref_checked_v1"));
                assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
            } else {
                assert!(result.is_err());
                assert_eq!(before, format!("{m:?}/{plans:?}"));
            }
        }
    }

    #[test]
    fn block_allocation_proves_prefix_without_reviving_old_or_exposed_alias() {
        for case in 0..5 {
            let (mut m, mut plans) = module(vec![Type::I64], vec![Operand::Constant(Constant::Int(0))]);
            let opaque = (ValueId(7), Instruction::CallRuntime { name: "dartforge_arc_collect".into(),
                args: vec![], ret_ty: Type::Void }, Type::Void);
            let b = &mut m.functions[0].blocks[0];
            if case == 0 { b.instructions.insert(0, opaque.clone()); }
            if case == 2 { b.instructions.push(opaque.clone()); }
            if case == 3 {
                b.instructions.push((ValueId(8), Instruction::ArcCopy { value: Operand::Val(ValueId(0)) }, Type::Ref));
                b.instructions.push(opaque.clone());
            }
            b.instructions.push((ValueId(1), Instruction::GetField { object: Operand::Val(ValueId(if case == 3 { 8 } else { 0 })), index: 0 }, Type::I64));
            if case == 1 { b.instructions.push(opaque.clone()); }
            if case == 4 {
                let allocate = b.instructions[0].clone();
                b.instructions = vec![(ValueId(9), allocate.1.clone(), Type::Ref)];
                b.terminator = Terminator::Branch(BlockId(1));
                m.functions[0].blocks.push(BasicBlock { id: BlockId(1), instructions: vec![
                    (ValueId(8), Instruction::Phi { ty: Type::Ref, incoming: vec![
                        (BlockId(0), Operand::Val(ValueId(9))), (BlockId(1), Operand::Val(ValueId(0))),
                    ] }, Type::Ref),
                    opaque, allocate,
                    (ValueId(1), Instruction::GetField { object: Operand::Val(ValueId(8)), index: 0 }, Type::I64),
                ], terminator: Terminator::CondBranch { cond: Operand::Constant(Constant::Bool(true)),
                    then_block: BlockId(1), else_block: BlockId(2) } });
                m.functions[0].blocks.push(BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(None) });
            }
            if case == 4 {
                // The object of the previous iteration dies on the back edge, before
                // the Phi receives the new one: the preparation releases the token there, and
                // the read of the field stays checked.
                prepare_arc_dart_module(&mut m, &mut plans).unwrap();
                let insts: Vec<_> = m.functions[0].blocks.iter().flat_map(|b| &b.instructions).collect();
                assert!(insts.iter().any(|(_, i, _)| matches!(i, Instruction::CallRuntime { name, .. } if name == super::READ_SCALAR)));
                assert!(insts.iter().any(|(_, i, _)| matches!(i, Instruction::ArcDrop { .. })));
                continue;
            }
            let result = prepare_arc_dart_module(&mut m, &mut plans);
            result.unwrap();
            let checked = m.functions[0].blocks.iter().flat_map(|b| &b.instructions)
                .any(|(_, i, _)| matches!(i, Instruction::CallRuntime { name, .. } if name == super::READ_SCALAR));
            assert_eq!(checked, case >= 2, "case {case}: exposed access must check the layout");
            assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
        }
    }

    #[test]
    fn define_rti_preserves_instance_and_requires_guard_in_lists() {
        for list in [false, true] {
            let (mut m, mut plans) = module(vec![Type::Ref], vec![Operand::Constant(Constant::Null)]);
            if list {
                let cid = dartforge_runtime::layout::cid::LIST as u32;
                m.classes[0].id = cid;
                m.arc_field_layouts = HashMap::from([(cid, vec![Type::Ref])]);
                let Instruction::AllocObject { class_id, .. } = &mut m.functions[0].blocks[0].instructions[0].1 else { panic!(); };
                *class_id = cid;
            }
            m.functions[0].blocks[0].instructions.extend([
                (ValueId(1), Instruction::CallRuntime { name: "dartforge_rti_set".into(), args: vec![
                    (Operand::Val(ValueId(0)), Type::Ref), (Operand::Constant(Constant::Int(0)), Type::I64),
                ], ret_ty: Type::Void }, Type::Void),
                (ValueId(2), Instruction::SetField { object: Operand::Val(ValueId(0)), index: 0,
                    value: Operand::Constant(Constant::Null) }, Type::Void),
            ]);
            let result = prepare_arc_dart_module(&mut m, &mut plans);
            result.unwrap();
            assert_eq!(m.functions[0].blocks[0].instructions.iter().any(|(_, i, _)|
                matches!(i, Instruction::CallRuntime { name, .. } if name == super::WRITE_CHECKED)), list);
            assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
        }
    }

    #[test]
    fn open_scalar_fields_check_layout_without_confusing_representations() {
        for repr in [Type::I64, Type::F64, Type::I1, Type::I8] {
            let (mut m, mut plans) = module(vec![repr], vec![Operand::Constant(Constant::Null)]);
            let f = &mut m.functions[0];
            f.params = vec![(ValueId(0), "this".into(), Type::Ref)];
            f.blocks[0].instructions.clear();
            let val_ref = match repr {
                Type::I64 => Operand::Constant(Constant::Int(42)),
                Type::F64 => Operand::Constant(Constant::Double(-0.0)),
                Type::I1 => Operand::Constant(Constant::Bool(true)),
                Type::I8 => {
                    f.blocks[0].instructions.push((ValueId(1), Instruction::Trunc {
                        op: Operand::Constant(Constant::Int(255)), from: Type::I64, to: Type::I8,
                    }, Type::I8));
                    Operand::Val(ValueId(1))
                }
                _ => unreachable!(),
            };
            f.blocks[0].instructions.extend([
                (ValueId(2), Instruction::SetField { object: Operand::Val(ValueId(0)), index: 0, value: val_ref }, Type::Void),
                (ValueId(3), Instruction::GetField { object: Operand::Val(ValueId(0)), index: 0 }, repr),
            ]);
            prepare_arc_dart_module(&mut m, &mut plans).unwrap();
            let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
            assert!(ir.contains("call void @dartforge_arc_write_field_scalar_checked_v1"));
            assert!(ir.contains("call i64 @dartforge_arc_read_field_scalar_checked_v1"));
            assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
            certify_arc_aot_ports(&mut m, &plans).unwrap();
            m.arc_field_layouts.insert(123, vec![Type::Ref]);
            let before = format!("{m:?}/{plans:?}");
            assert!(prepare_arc_dart_module(&mut m, &mut plans).is_err());
            assert_eq!(before, format!("{m:?}/{plans:?}"));
            assert!(certify_arc_aot_ports(&mut m, &plans).is_err());
        }
    }

    #[test]
    fn open_ref_setter_checks_class_size_and_current_table() {
        let (mut m, mut plans) = module(vec![Type::Ref], vec![Operand::Constant(Constant::Null)]);
        m.arc_field_layouts.insert(124, vec![Type::Ref, Type::I64]);
        m.arc_field_layouts.insert(125, vec![Type::I64]);
        for (id, n) in [(124, 2), (125, 1)] {
            m.classes.push(ClassDef { id, name: format!("C{id}"), field_count: n,
                vtable: vec![], to_string_symbol: None });
        }
        m.functions[0].params = vec![(ValueId(0), "this".into(), Type::Ref),
            (ValueId(1), "valor".into(), Type::Ref)];
        m.functions[0].blocks[0].instructions = vec![(ValueId(2), Instruction::SetField {
            object: Operand::Val(ValueId(0)), index: 0, value: Operand::Val(ValueId(1)),
        }, Type::Void)];
        prepare_arc_dart_module(&mut m, &mut plans).unwrap();
        assert!(m.functions[0].blocks[0].instructions.iter().any(|(_, i, _)|
            matches!(i, Instruction::ConstArray(v) if v == &vec![123, 1, 124, 2])));
        let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
        assert!(ir.contains("call void @dartforge_arc_write_field_ref_checked_v1"));
        assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
        for case in 0..4 {
            let mut fs = m.functions.clone();
            let mut ps = plans.clone();
            if case == 0 {
                let (_, Instruction::ConstArray(v), _) = &mut fs[0].blocks[0].instructions[0] else { panic!(); };
                v[1] = 2;
            } else {
                let (_, Instruction::CallRuntime { args, .. }, _) = &mut fs[0].blocks[0].instructions[1] else { panic!(); };
                if case == 1 { args[4].0 = Operand::Constant(Constant::Int(1)); }
                if case == 2 { args[3] = (Operand::Val(ValueId(0)), Type::Ref); }
                if case == 3 { args[1].0 = Operand::Constant(Constant::Int(-1)); }
            }
            let before = format!("{fs:?}/{ps:?}");
            let result = super::prepare(&mut fs[0], &m.arc_field_layouts, ps.get_mut("campos").unwrap(), None, &HashSet::new());
            assert!(result.is_err(), "caso {case}: {result:?}");
            assert_eq!(before, format!("{fs:?}/{ps:?}"));
        }
        certify_arc_aot_ports(&mut m, &plans).unwrap();
        m.arc_field_layouts.insert(123, vec![Type::I64]);
        assert!(certify_arc_aot_ports(&mut m, &plans).is_err());
    }

    #[test]
    fn open_receiver_ref_getter_checks_map_and_returns_owned() {
        let (mut m, _) = module(vec![], vec![]);
        m.functions[0].params = vec![(ValueId(0), "this".into(), Type::Ref)];
        m.functions[0].return_ty = Type::Ref;
        m.functions[0].blocks[0].instructions = vec![(ValueId(1), Instruction::GetField {
            object: Operand::Val(ValueId(0)), index: 39,
        }, Type::Ref)];
        m.functions[0].blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
        let mut plan = DartFunctionPlan::default();
        plan.tokens.ret = TokenReturn::Owned;
        let mut plans = HashMap::from([("campos".into(), plan)]);
        assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (1, 0));
        let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
        let read = ir.find("call i64 @dartforge_arc_read_field_ref_v1").unwrap();
        let retain = ir.find("call void @df.arc_retain_ref(i64 %v1)").unwrap();
        assert!(read < retain);
        assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
    }

    #[test]
    fn open_receiver_requires_layout_for_write_or_scalar_and_limits() {
        for case in 0..3 {
            let (mut m, mut plans) = module(vec![], vec![]);
            m.functions[0].params = vec![(ValueId(0), "this".into(), Type::Ref)];
            m.functions[0].blocks[0].instructions = vec![(ValueId(1), if case == 0 {
                Instruction::SetField { object: Operand::Val(ValueId(0)), index: 0,
                    value: Operand::Constant(Constant::Null) }
            } else {
                Instruction::GetField { object: Operand::Val(ValueId(0)),
                    index: if case == 2 { usize::MAX } else { 0 } }
            }, if case == 0 { Type::Void } else if case == 1 { Type::I64 } else { Type::Ref })];
            let before = format!("{m:?}/{plans:?}");
            assert!(prepare_arc_dart_module(&mut m, &mut plans).is_err());
            assert_eq!(before, format!("{m:?}/{plans:?}"));
        }
    }

    #[test]
    fn zeroed_factory_fixes_origin_only_with_checked_class_and_size() {
        for case in 0..5 {
            let (mut m, mut plans) = module(vec![Type::Ref], vec![Operand::Constant(Constant::Null)]);
            m.functions[0].params.push((ValueId(7), "filho".into(), Type::Ref));
            let name_str = if case == 1 { "dartforge_arc_object_owned_v1" } else { "dartforge_object_new" };
            m.functions[0].blocks[0].instructions[0].1 = Instruction::CallRuntime {
                name: name_str.into(), args: vec![
                    (Operand::Constant(Constant::Int(if case == 2 { 124 } else { 123 })), Type::I64),
                    (Operand::Constant(Constant::Int(if case == 3 { 2 } else { 1 })), Type::I64),
                ], ret_ty: Type::Ref,
            };
            if case == 4 {
                m.functions[0].blocks[0].instructions.push((ValueId(8), Instruction::CallRuntime {
                    name: "dartforge_print_handle".into(), args: vec![(Operand::Val(ValueId(0)), Type::Ref)], ret_ty: Type::Void,
                }, Type::Void));
            }
            m.functions[0].blocks[0].instructions.push((ValueId(1), Instruction::SetField {
                object: Operand::Val(ValueId(0)), index: 0, value: Operand::Val(ValueId(7)),
            }, Type::Void));
            let before = format!("{m:?}/{plans:?}");
            let result = prepare_arc_dart_module(&mut m, &mut plans);
            if case >= 2 {
                assert!(result.is_err(), "caso {case}: {result:?}");
                assert_eq!(before, format!("{m:?}/{plans:?}"));
            } else {
                result.unwrap();
                assert!(m.functions[0].blocks[0].instructions.iter().any(|(_, i, _)|
                    matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_arc_write_field_ref_v1")));
                assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
            }
        }
    }

    #[test]
    fn ref_read_retained_before_edge_replacement() {
        let (mut m, mut plans) = module(vec![Type::Ref], vec![Operand::Val(ValueId(10))]);
        let f = &mut m.functions[0];
        f.return_ty = Type::Ref;
        f.blocks[0].instructions.insert(
            0,
            (
                ValueId(10),
                Instruction::Box {
                    op: Operand::Constant(Constant::Int(i64::MAX)),
                    from: Type::I64,
                },
                Type::Ref,
            ),
        );
        f.blocks[0].instructions.extend([
            (
                ValueId(1),
                Instruction::GetField {
                    object: Operand::Val(ValueId(0)),
                    index: 0,
                },
                Type::Ref,
            ),
            (
                ValueId(2),
                Instruction::SetField {
                    object: Operand::Val(ValueId(0)),
                    index: 0,
                    value: Operand::Constant(Constant::Null),
                },
                Type::Void,
            ),
        ]);
        f.blocks[0].terminator = Terminator::Return(Some(Operand::Val(ValueId(1))));
        plans.get_mut("campos").unwrap().tokens.ret = TokenReturn::Owned;
        assert_eq!(
            prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
            (1, 2)
        );
        let ops = &m.functions[0].blocks[0].instructions;
        let copy = ops
            .iter()
            .position(|(v, i, _)| *v == ValueId(1) && matches!(i, Instruction::ArcCopy { .. }))
            .unwrap();
        let set = ops.iter().position(|(v, _, _)| *v == ValueId(2)).unwrap();
        assert!(copy < set);
        assert!(!ops.iter().any(|(_, i, _)| matches!(
            i,
            Instruction::GetField { .. } | Instruction::SetField { .. }
        )));
        crate::llvm::LlvmEmitter::new(&m).emit_all();
        let before = format!("{m:?}/{plans:?}");
        assert_eq!(
            prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{m:?}/{plans:?}"), before);
    }

    #[test]
    fn narrow_fields_preserve_bits_and_limit_before_read() {
        let (mut m, mut plans) = module(
            vec![Type::F64, Type::I1, Type::I8],
            vec![
                Operand::Constant(Constant::Double(-0.0)),
                Operand::Constant(Constant::Bool(true)),
                Operand::Val(ValueId(10)),
            ],
        );
        let f = &mut m.functions[0];
        f.blocks[0].instructions.insert(
            0,
            (
                ValueId(10),
                Instruction::Trunc {
                    op: Operand::Constant(Constant::Int(255)),
                    from: Type::I64,
                    to: Type::I8,
                },
                Type::I8,
            ),
        );
        for (i, repr) in [Type::F64, Type::I1, Type::I8].into_iter().enumerate() {
            f.blocks[0].instructions.push((
                ValueId(20 + i as u32),
                Instruction::GetField {
                    object: Operand::Val(ValueId(0)),
                    index: i,
                },
                repr,
            ));
            f.blocks[0].instructions.push((
                ValueId(30 + i as u32),
                Instruction::SetField {
                    object: Operand::Val(ValueId(0)),
                    index: i,
                    value: Operand::Val(ValueId(20 + i as u32)),
                },
                Type::Void,
            ));
        }
        let p = plans.get_mut("campos").unwrap();
        p.scopes
            .before
            .insert(ValueId(20), vec![ScopeChange::Open(7)]);
        p.scopes
            .exits
            .insert(BlockId(0), vec![ScopeChange::Close(7)]);
        assert_eq!(
            prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
            (0, 1)
        );
        let ops = &m.functions[0].blocks[0].instructions;
        let pos = ops.iter().position(|(v, _, _)| *v == ValueId(20)).unwrap();
        assert!(matches!(
            ops[pos].1,
            Instruction::Bitcast { to: Type::F64, .. }
        ));
        assert_eq!(
            plans["campos"].scopes.before[&ops[pos - 1].0],
            vec![ScopeChange::Open(7)]
        );
        assert!(!plans["campos"].scopes.before.contains_key(&ValueId(20)));
        for v in [21, 22] {
            assert!(matches!(
                ops.iter().find(|(id, _, _)| id.0 == v).unwrap().1,
                Instruction::Trunc {
                    from: Type::I64,
                    ..
                }
            ));
        }
        crate::llvm::LlvmEmitter::new(&m).emit_all();
    }

    #[test]
    fn exposed_aliases_require_guard_and_contradictions_reject_atomically() {
        for case in 0..8 {
            let (mut m, mut plans) =
                module(vec![Type::I64], vec![Operand::Constant(Constant::Int(85))]);
            let f = &mut m.functions[0];
            let mut receiver = Operand::Val(ValueId(0));
            let mut index = 0;
            let mut ty = Type::I64;
            match case {
                0 => {
                    f.blocks[0].instructions.extend([
                        (
                            ValueId(1),
                            Instruction::ArcCopy {
                                value: receiver.clone(),
                            },
                            Type::Ref,
                        ),
                        (
                            ValueId(2),
                            Instruction::CallRuntime {
                                name: "dartforge_print_handle".into(),
                                args: vec![(Operand::Val(ValueId(1)), Type::Ref)],
                                ret_ty: Type::Void,
                            },
                            Type::Void,
                        ),
                    ]);
                }
                1 => {
                    f.params.push((ValueId(7), "C".into(), Type::Ref));
                    receiver = Operand::Val(ValueId(7));
                }
                2 => index = 1,
                3 => ty = Type::Ref,
                4 => {
                    f.blocks[0].instructions.push((
                        ValueId(1),
                        Instruction::CallRuntime {
                            name: "dartforge_arc_write_field_ref_v1".into(),
                            args: vec![
                                (receiver.clone(), Type::Ref),
                                (Operand::Constant(Constant::Int(0)), Type::I64),
                                (Operand::Constant(Constant::Null), Type::Ref),
                            ],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ));
                }
                5 => {
                    m.arc_field_layouts.insert(123, vec![Type::F64]);
                    ty = Type::F64;
                    plans
                        .get_mut("campos")
                        .unwrap()
                        .classes
                        .insert(ValueId(u32::MAX), Ownership::Trivial);
                }
                6 => {
                    f.blocks[0].instructions.push((
                        ValueId(8),
                        Instruction::SetField {
                            object: receiver.clone(),
                            index: 0,
                            value: Operand::Constant(Constant::Null),
                        },
                        Type::Void,
                    ));
                }
                _ => {
                    f.blocks[0].instructions.push((
                        ValueId(8),
                        Instruction::CallRuntime {
                            name: "extern_sem_resumo".into(),
                            args: vec![],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ));
                }
            }
            f.blocks[0].instructions.push((
                ValueId(5),
                Instruction::GetField {
                    object: receiver,
                    index,
                },
                ty,
            ));
            let before = format!("{m:?}/{plans:?}");
            let result = prepare_arc_dart_module(&mut m, &mut plans);
            if matches!(case, 1 | 4) {
                result.unwrap();
                assert!(m.functions[0].blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)|
                    matches!(i, Instruction::CallRuntime { name, .. } if name == super::READ_SCALAR)));
                assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
            } else {
                let error = result.unwrap_err();
                if case == 0 { assert!(error.contains("exceptional exits"), "caso {case}: {error}"); }
                if case == 7 { assert!(error.contains("extern_sem_resumo"), "caso {case}: {error}"); }
                assert_eq!(format!("{m:?}/{plans:?}"), before, "caso {case}");
            }
            m.arc_memory = false;
            let before = format!("{m:?}/{plans:?}");
            assert_eq!(
                prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                (0, 0)
            );
            assert_eq!(format!("{m:?}/{plans:?}"), before);
        }
    }

    #[test]
    fn layout_guard_with_mixed_phi_gets_edge_copy() {
        let (mut m, mut plans) =
            module(vec![Type::I64], vec![Operand::Constant(Constant::Int(85))]);
        let f = &mut m.functions[0];
        f.params = vec![
            (ValueId(7), "outro".into(), Type::Ref),
            (ValueId(8), "cond".into(), Type::I1),
        ];
        f.blocks[0].terminator = Terminator::CondBranch {
            cond: Operand::Val(ValueId(8)),
            then_block: BlockId(1),
            else_block: BlockId(2),
        };
        for b in [1, 2] {
            f.blocks.push(BasicBlock {
                id: BlockId(b),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(3)),
            });
        }
        f.blocks.push(BasicBlock {
            id: BlockId(3),
            instructions: vec![
                (
                    ValueId(9),
                    Instruction::Phi {
                        incoming: vec![
                            (BlockId(1), Operand::Val(ValueId(0))),
                            (BlockId(2), Operand::Val(ValueId(7))),
                        ],
                        ty: Type::Ref,
                    },
                    Type::Ref,
                ),
                (
                    ValueId(10),
                    Instruction::CallRuntime {
                        name: "dartforge_print_handle".into(),
                        args: vec![(Operand::Val(ValueId(9)), Type::Ref)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
                (
                    ValueId(11),
                    Instruction::GetField {
                        object: Operand::Val(ValueId(0)),
                        index: 0,
                    },
                    Type::I64,
                ),
            ],
            terminator: Terminator::Return(None),
        });
        // No fallible consumer in between: the borrow of the parameter gets the
        // copy on the edge, and the layout guard stays on `v11`.
        m.functions[0].blocks.last_mut().unwrap().instructions.remove(1);
        prepare_arc_dart_module(&mut m, &mut plans).unwrap();
        assert!(m.functions[0].blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)|
            matches!(i, Instruction::ArcCopy { value: Operand::Val(ValueId(7)) })));
    }

    #[test]
    fn phi_and_copy_keep_all_candidate_layouts() {
        for case in 0..3 {
            let (mut m, mut plans) =
                module(vec![Type::I64], vec![Operand::Constant(Constant::Int(85))]);
            m.classes.push(ClassDef {
                id: 124,
                name: "D".into(),
                field_count: 1,
                vtable: vec![],
                to_string_symbol: None,
            });
            let repr = if case == 1 { Type::Ref } else { Type::I64 };
            m.arc_field_layouts.insert(124, vec![repr]);
            let other = if case == 1 {
                Operand::Constant(Constant::Null)
            } else {
                Operand::Constant(Constant::Int(86))
            };
            let f = &mut m.functions[0];
            f.params = vec![(ValueId(0), "cond".into(), Type::I1)];
            f.return_ty = Type::I64;
            f.blocks = vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(0)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::AllocObject {
                            class_id: 123,
                            fields: vec![Operand::Constant(Constant::Int(85))],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![(
                        ValueId(2),
                        Instruction::AllocObject {
                            class_id: 124,
                            fields: vec![other],
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(3),
                    instructions: vec![
                        (
                            ValueId(3),
                            Instruction::Phi {
                                incoming: vec![
                                    (BlockId(1), Operand::Val(ValueId(1))),
                                    (
                                        BlockId(2),
                                        if case == 2 {
                                            Operand::Constant(Constant::Null)
                                        } else {
                                            Operand::Val(ValueId(2))
                                        },
                                    ),
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
                            Instruction::GetField {
                                object: Operand::Val(ValueId(4)),
                                index: 0,
                            },
                            Type::I64,
                        ),
                    ],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(5)))),
                },
            ];
            let before = format!("{m:?}/{plans:?}");
            if case == 1 {
                assert!(prepare_arc_dart_module(&mut m, &mut plans).is_err());
                assert_eq!(format!("{m:?}/{plans:?}"), before);
            } else {
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    if case == 2 { (0, 3) } else { (0, 2) }
                );
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                if case == 0 {
                    assert!(ir.contains("call i64 @dartforge_arc_read_field_scalar_v1(i64 %v4, i64 0)"));
                } else {
                    // The null path is rejected by the physical guard, without
                    // invalidating the known representation of the other path.
                    assert!(ir.contains("call i64 @dartforge_arc_read_field_scalar_checked_v1(i64 %v4, i64 0,"));
                }
            }
        }
    }

    #[test]
    fn mixed_phi_checks_nominal_origin_without_hiding_type_or_owner() {
        for case in 0..4 {
            let repr = if case == 1 { Type::Ref } else { Type::I64 };
            let field = if repr == Type::Ref { Operand::Constant(Constant::Null) }
                else { Operand::Constant(Constant::Int(85)) };
            let (mut m, mut plans) = module(vec![repr], vec![field.clone()]);
            let f = &mut m.functions[0];
            f.params = vec![(ValueId(7), "receiver".into(), Type::Ref),
                (ValueId(8), "cond".into(), Type::I1)];
            f.return_ty = Type::I64;
            f.blocks = vec![
                BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::CondBranch {
                    cond: Operand::Val(ValueId(8)), then_block: BlockId(1), else_block: BlockId(2) } },
                BasicBlock { id: BlockId(1), instructions: vec![
                    (ValueId(0), Instruction::AllocObject { class_id: 123, fields: vec![field] }, Type::Ref),
                ], terminator: Terminator::Branch(BlockId(3)) },
                BasicBlock { id: BlockId(2), instructions: vec![
                    (ValueId(9), if case == 2 { Instruction::Const(Constant::Int(0)) }
                        else { Instruction::ArcCopy { value: Operand::Val(ValueId(7)) } },
                        if case == 2 { Type::I64 } else { Type::Ref }),
                ], terminator: Terminator::Branch(BlockId(3)) },
                BasicBlock { id: BlockId(3), instructions: vec![
                    (ValueId(10), Instruction::Phi { ty: Type::Ref, incoming: vec![
                        (BlockId(1), Operand::Val(ValueId(0))),
                        (BlockId(2), Operand::Val(ValueId(if case == 3 { 7 } else { 9 }))),
                    ] }, Type::Ref),
                    (ValueId(11), Instruction::SetField { object: Operand::Val(ValueId(10)), index: 0,
                        value: Operand::Constant(Constant::Int(86)) }, Type::Void),
                    (ValueId(12), Instruction::GetField { object: Operand::Val(ValueId(10)), index: 0 }, Type::I64),
                ], terminator: Terminator::Return(Some(Operand::Val(ValueId(12)))) },
            ];
            let before = format!("{m:?}/{plans:?}");
            let result = prepare_arc_dart_module(&mut m, &mut plans);
            // Case 3: the borrowed parameter enters the Phi with an owner; the
            // preparation copies on the edge (`phis.rs`), like the explicit case 0.
            if case == 3 {
                result.unwrap();
                assert!(m.functions[0].blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)|
                    matches!(i, Instruction::ArcCopy { value: Operand::Val(ValueId(7)) })));
                continue;
            }
            if case == 0 {
                result.unwrap();
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                assert!(ir.contains(super::WRITE_SCALAR));
                assert!(ir.contains(super::READ_SCALAR));
                let prepared = format!("{m:?}/{plans:?}");
                assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
                assert_eq!(prepared, format!("{m:?}/{plans:?}"));
            } else {
                let error = result.unwrap_err();
                if case == 1 { assert!(error.contains("incompatible type or guard"), "{error}"); }
                assert_eq!(before, format!("{m:?}/{plans:?}"));
            }
        }
    }

    #[test]
    fn loop_anchored_phi_keeps_origin_without_assuming_singleton() {
        let (mut m, mut plans) =
            module(vec![Type::I64], vec![Operand::Constant(Constant::Int(85))]);
        let f = &mut m.functions[0];
        f.params = vec![(ValueId(1), "repetir".into(), Type::I1)];
        f.return_ty = Type::I64;
        f.blocks[0].terminator = Terminator::Branch(BlockId(1));
        f.blocks.extend([
            BasicBlock {
                id: BlockId(1),
                instructions: vec![
                    (
                        ValueId(3),
                        Instruction::Phi {
                            incoming: vec![
                                (BlockId(0), Operand::Val(ValueId(0))),
                                (BlockId(2), Operand::Val(ValueId(3))),
                            ],
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(5),
                        Instruction::GetField {
                            object: Operand::Val(ValueId(3)),
                            index: 0,
                        },
                        Type::I64,
                    ),
                ],
                terminator: Terminator::CondBranch {
                    cond: Operand::Val(ValueId(1)),
                    then_block: BlockId(2),
                    else_block: BlockId(3),
                },
            },
            BasicBlock {
                id: BlockId(2),
                instructions: vec![],
                terminator: Terminator::Branch(BlockId(1)),
            },
            BasicBlock {
                id: BlockId(3),
                instructions: vec![],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(5)))),
            },
        ]);
        assert_eq!(
            prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
            (0, 1)
        );
        crate::llvm::LlvmEmitter::new(&m).emit_all();
        let before = format!("{m:?}/{plans:?}");
        assert_eq!(
            prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
            (0, 0)
        );
        assert_eq!(format!("{m:?}/{plans:?}"), before);
    }
}
