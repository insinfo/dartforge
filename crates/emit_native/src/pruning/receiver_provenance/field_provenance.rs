//! Closed initialization facts for physically allocated, published constant objects.
//!
//! Declaration metadata selects a slot; it never supplies receiver identity. This
//! bounded slice accepts only straight initialization paths and guarded child calls.

use super::*;
use crate::hir::{FieldDeclaration, ICmpOp};

/// Immutable body witnesses and complete allocation/writer census for this audit.
pub(crate) struct Inventory<'a> {
    bodies: HashMap<String, &'a Function>,
    layouts: HashMap<u32, Vec<Option<FieldDeclaration>>>,
    initialized: HashMap<u32, HashMap<usize, Classes>>,
}

impl<'a> Inventory<'a> {
    /// Unknown births, aliases, raw writes and conflicting producer layouts invalidate facts.
    /// `complete_bodies` is an independent producer obligation: the supplied bodies
    /// must cover every linked source-library writer and allocation, including SDK
    /// bodies. Layout records, nominal registration and cached summaries alone do
    /// not discharge it. Partial callers must pass false.
    pub(crate) fn new(
        modules: &[&'a Module],
        proof: &Provenance<'a>,
        complete_bodies: bool,
    ) -> Self {
        let mut result = Self {
            bodies: HashMap::new(),
            layouts: HashMap::new(),
            initialized: HashMap::new(),
        };
        if !complete_bodies
            || modules.is_empty()
            || modules
                .iter()
                .any(|m| !m.arc_fixed_ancestry || m.arc_memory || m.arc_prepared)
        {
            return result;
        }
        let mut bad = HashSet::new();
        let mut references = HashSet::new();
        let mut stores: HashMap<String, Vec<(&Function, u32, Type)>> = HashMap::new();
        let mut roots = HashMap::new();
        let mut exposed = HashSet::new();
        let mut area_exposed = false;
        for module in modules {
            for (cid, layout) in &module.declared_field_layouts {
                if module.pruning_ambiguous_class_ids.contains(cid)
                    || *cid < dartforge_runtime::layout::FIRST_FREE_CID as u32
                    || !module.pruning_class_ids.contains(cid)
                    || module.arc_external_ancestry.ambiguous_classes.contains(cid)
                    || module.classes.iter().filter(|c| c.id == *cid).count() != 1
                    || module
                        .classes
                        .iter()
                        .find(|c| c.id == *cid)
                        .is_none_or(|c| c.field_count != layout.len())
                {
                    bad.insert(*cid);
                    continue;
                }
                if result.layouts.get(cid).is_some_and(|old| old != layout) {
                    bad.insert(*cid);
                }
                result.layouts.insert(*cid, layout.clone());
            }
            for (root, ty, symbol) in &module.globals {
                roots
                    .entry(symbol.clone())
                    .or_insert_with(Vec::new)
                    .push((*root, *ty));
            }
            for f in &module.functions {
                if result.bodies.insert(f.symbol.clone(), f).is_some() {
                    area_exposed = true;
                }
                for (_, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
                    match i {
                        I::CallStatic { symbol, .. } => {
                            references.insert(symbol.clone());
                            if matches!(
                                symbol.as_str(),
                                "dartforge_globals_area" | "dartforge_globals_area_id"
                            ) {
                                area_exposed = true;
                            }
                        }
                        I::ArcStoreStrong {
                            slot: crate::hir::StrongSlot::Global { symbol },
                            ..
                        } => {
                            exposed.insert(symbol.clone());
                        }
                        I::StoreGlobal {
                            symbol, ty, root, ..
                        } => {
                            stores.entry(symbol.clone()).or_default().push((
                                f,
                                root.unwrap_or(0),
                                *ty,
                            ));
                        }
                        I::TearOff { code_symbol } | I::AllocClosure { code_symbol, .. } => {
                            references.insert(code_symbol.clone());
                            exposed.insert(code_symbol.clone());
                        }
                        I::AllocClosureTyped {
                            code_symbol, typed, ..
                        } => {
                            references.insert(code_symbol.clone());
                            references.insert(typed.clone());
                            exposed.insert(code_symbol.clone());
                            exposed.insert(typed.clone());
                        }
                        I::Const(Constant::String(name))
                            if matches!(
                                name.as_str(),
                                "dartforge_globals_area" | "dartforge_globals_area_id"
                            ) =>
                        {
                            area_exposed = true
                        }
                        I::Const(Constant::Function(name)) => {
                            references.insert(name.clone());
                            exposed.insert(name.clone());
                        }
                        I::FunctionTable(names) => {
                            references.extend(names.iter().cloned());
                            exposed.extend(names.iter().cloned());
                        }
                        I::CallRuntime { name, .. } | I::DeclaredFieldCall { name, .. }
                            if matches!(
                                name.as_str(),
                                "dartforge_globals_area" | "dartforge_globals_area_id"
                            ) =>
                        {
                            area_exposed = true
                        }
                        _ => {}
                    }
                    operands(i, &mut |op| {
                        if let Operand::Constant(Constant::Function(name)) = op {
                            references.insert(name.clone());
                            exposed.insert(name.clone());
                        }
                    });
                }
                for b in &f.blocks {
                    terminator_operands(&b.terminator, &mut |op| {
                        if let Operand::Constant(Constant::Function(name)) = op {
                            references.insert(name.clone());
                            exposed.insert(name.clone());
                        }
                    });
                }
            }
            for (_, _, entries) in &module.method_tables {
                references.extend(entries.iter().map(|(_, f)| f.clone()));
            }
        }
        if area_exposed
            || [
                "dartforge_object_new",
                "dartforge_object_get",
                "dartforge_object_set",
                "dartforge_canonical_constant",
                "dartforge_mark_constant",
                "dartforge_exception_pending",
                "dartforge_globals_area",
                "dartforge_globals_area_id",
            ]
            .iter()
            .any(|s| result.bodies.contains_key(*s))
        {
            return result;
        }
        let mut accepted_sites = HashSet::new();
        let mut accepted_writes = HashSet::new();
        for f in result.bodies.values() {
            let Some(values) = proof.values_for(f) else {
                continue;
            };
            let Some(cache) = initialization(
                f,
                &result.layouts,
                values,
                &result.bodies,
                &roots,
                &stores,
                &exposed,
            ) else {
                continue;
            };
            accepted_sites.insert((f.symbol.clone(), cache.allocation));
            accepted_writes.extend(cache.writes.into_iter().map(|v| (f.symbol.clone(), v)));
            let fields = result.initialized.entry(cache.cid).or_default();
            for (slot, classes) in cache.fields {
                fields.entry(slot).or_default().extend(classes);
            }
        }
        // Inspect every supplied body, including unreachable blocks. An unused
        // constructor body is excluded only when no audited code/table/address
        // can invoke it; unknown entry parameters never receive class facts.
        for f in result.bodies.values() {
            let values = proof.values_for(f);
            let unused_void = f.return_ty == Type::Void
                && !references.contains(&f.symbol)
                && !matches!(f.name.as_str(), "main")
                && !f.params.is_empty();
            for (v, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
                match i {
                    I::AllocObject { class_id, .. } => {
                        if !accepted_sites.contains(&(f.symbol.clone(), *v)) {
                            bad.insert(*class_id);
                        }
                    }
                    I::CallRuntime { name, args, .. } | I::DeclaredFieldCall { name, args, .. }
                        if matches!(
                            name.as_str(),
                            "dartforge_object_new"
                                | "dartforge_object_new_t"
                                | "dartforge_arc_object_owned_v1"
                        ) =>
                    {
                        match args.first() {
                            Some((Operand::Constant(Constant::Int(cid)), Type::I64)) => {
                                if let Ok(cid) = u32::try_from(*cid) {
                                    if !accepted_sites.contains(&(f.symbol.clone(), *v)) {
                                        bad.insert(cid);
                                    }
                                }
                            }
                            _ => bad.extend(result.layouts.keys().copied()),
                        }
                    }
                    I::DeclaredFieldCall { name, args, .. } if name == "dartforge_object_set" => {
                        if unused_void || accepted_writes.contains(&(f.symbol.clone(), *v)) {
                            continue;
                        }
                        let known = args
                            .first()
                            .and_then(|(op, _)| values.and_then(|m| operand_classes(op, m)));
                        if let Some(classes) = known {
                            bad.extend(classes);
                        } else {
                            bad.extend(result.layouts.keys().copied());
                        }
                    }
                    I::CallRuntime { name, args, .. } if name == "dartforge_object_set" => {
                        if unused_void {
                            continue;
                        }
                        let known = args
                            .first()
                            .and_then(|(op, _)| values.and_then(|m| operand_classes(op, m)));
                        if let Some(classes) = known {
                            bad.extend(classes);
                        } else {
                            bad.extend(result.layouts.keys().copied());
                        }
                    }
                    I::SetField { object, .. } => {
                        if unused_void {
                            continue;
                        }
                        if let Some(classes) = values.and_then(|m| operand_classes(object, m)) {
                            bad.extend(classes);
                        } else {
                            bad.extend(result.layouts.keys().copied());
                        }
                    }
                    I::Bitcast {
                        op, to: Type::Ptr, ..
                    } => {
                        if let Some(classes) = values.and_then(|m| operand_classes(op, m)) {
                            bad.extend(classes);
                        } else {
                            bad.extend(result.layouts.keys().copied());
                        }
                    }
                    _ => {}
                }
            }
        }
        result.initialized.retain(|cid, _| !bad.contains(cid));
        result
    }

    /// Receiver classes must come from an independent physical witness at the call site.
    /// Unknown parameters and partial class sets cannot be recovered from field metadata.
    pub(crate) fn classes_for_read(
        &self,
        f: &Function,
        read: ValueId,
        receivers: &Classes,
    ) -> Option<Classes> {
        if receivers.is_empty()
            || !self
                .bodies
                .get(&f.symbol)
                .is_some_and(|body| std::ptr::eq(*body, f))
        {
            return None;
        }
        let shape = Shape::new(f).ok()?;
        let (_, Some((block, position))) = shape.definitions.get(&read)? else {
            return None;
        };
        let (_, i, tuple) = &f.blocks[*block].instructions[*position];
        let I::DeclaredFieldCall {
            declaration,
            name,
            args,
            ret_ty: Type::Ref,
        } = i
        else {
            return None;
        };
        if *tuple != Type::Ref
            || name != "dartforge_object_get"
            || !eligible(declaration)
            || !matches!(args.as_slice(), [(receiver, Type::Ref), (Operand::Constant(Constant::Int(slot)), Type::I64)]
                if shape.operand_type(receiver) == Some(Type::Ref) && usize::try_from(*slot).ok() == Some(declaration.layout_index))
        {
            return None;
        }
        let mut result = Classes::new();
        for cid in receivers {
            let layout = self.layouts.get(cid)?;
            if layout.get(declaration.layout_index)?.as_ref()? != declaration {
                return None;
            }
            result.extend(
                self.initialized
                    .get(cid)?
                    .get(&declaration.layout_index)?
                    .iter()
                    .copied(),
            );
        }
        (!result.is_empty()).then_some(result)
    }
}

struct Initialization {
    cid: u32,
    allocation: ValueId,
    writes: Vec<ValueId>,
    fields: HashMap<usize, Classes>,
}

/// Reconcile the declaration and the storage representation, without nominal inference.
fn eligible(d: &FieldDeclaration) -> bool {
    d.is_final
        && !d.is_late
        && d.storage_type == Type::Ref
        && !d.library_uri.is_empty()
        && !d.owner.is_empty()
        && !d.name.is_empty()
}

/// Accept the exact publication protocol and a single acyclic cold-success path.
fn initialization(
    f: &Function,
    layouts: &HashMap<u32, Vec<Option<FieldDeclaration>>>,
    values: &HashMap<ValueId, Classes>,
    functions: &HashMap<String, &Function>,
    roots: &HashMap<String, Vec<(u32, Type)>>,
    stores: &HashMap<String, Vec<(&Function, u32, Type)>>,
    exposed: &HashSet<String>,
) -> Option<Initialization> {
    if !f.params.is_empty() || f.return_ty != Type::Ref {
        return None;
    }
    let shape = Shape::new(f).ok()?;
    let entry = &f.blocks[0];
    let [(
        flag_v,
        I::LoadGlobal {
            symbol: flag,
            ty: Type::I8,
        },
        Type::I8,
    ), (
        condition,
        I::ICmp(ICmpOp::Ne, Operand::Val(read_flag), Operand::Constant(Constant::Int(0))),
        Type::I1,
    )] = entry.instructions.as_slice()
    else {
        return None;
    };
    if flag_v != read_flag {
        return None;
    }
    let Terminator::CondBranch {
        cond: Operand::Val(c),
        then_block,
        else_block,
    } = entry.terminator
    else {
        return None;
    };
    if c != *condition || then_block == else_block {
        return None;
    }
    let warm = &f.blocks[*shape.positions.get(&then_block)?];
    let [(
        warm_v,
        I::LoadGlobal {
            symbol: cache,
            ty: Type::Ref,
        },
        Type::Ref,
    )] = warm.instructions.as_slice()
    else {
        return None;
    };
    if flag != &format!("{cache}$ok")
        || !matches!(warm.terminator, Terminator::Return(Some(Operand::Val(v))) if v == *warm_v)
        || exposed.contains(cache)
        || exposed.contains(flag)
        || roots.contains_key(flag)
    {
        return None;
    }
    let mut bi = *shape.positions.get(&else_block)?;
    let mut path = HashSet::new();
    let mut flat = Vec::new();
    let mut guarded = HashSet::new();
    let mut failures = HashSet::new();
    let mut predecessors = vec![Vec::new(); f.blocks.len()];
    for (from, block) in f.blocks.iter().enumerate() {
        let successors: Vec<_> = match &block.terminator {
            Terminator::Branch(to) => vec![*to],
            Terminator::CondBranch {
                then_block,
                else_block,
                ..
            } => vec![*then_block, *else_block],
            Terminator::Switch { default, cases, .. } => std::iter::once(*default)
                .chain(cases.iter().map(|(_, to)| *to))
                .collect(),
            _ => vec![],
        };
        for to in successors {
            predecessors[*shape.positions.get(&to)?].push(from);
        }
    }
    let mut previous = 0;
    loop {
        if !path.insert(bi) || predecessors[bi] != [previous] {
            return None;
        }
        previous = bi;
        let block = &f.blocks[bi];
        flat.extend(block.instructions.iter());
        match &block.terminator {
            Terminator::Branch(to) => bi = *shape.positions.get(to)?,
            Terminator::CondBranch {
                cond: Operand::Val(c),
                then_block: fail,
                else_block: success,
            } => {
                let n = flat.len();
                if n < 3 {
                    return None;
                }
                let (call_value, call, Type::Ref) = flat[n - 3] else {
                    return None;
                };
                let I::CallStatic {
                    symbol,
                    args,
                    ret_ty: Type::Ref,
                } = call
                else {
                    return None;
                };
                if !args.is_empty()
                    || functions.get(symbol).is_none_or(|callee| {
                        !callee.params.is_empty() || callee.return_ty != Type::Ref
                    })
                {
                    return None;
                }
                let (
                    pending,
                    I::CallRuntime {
                        name,
                        args,
                        ret_ty: Type::I8,
                    },
                    Type::I8,
                ) = flat[n - 2]
                else {
                    return None;
                };
                let (
                    test,
                    I::ICmp(ICmpOp::Ne, Operand::Val(p), Operand::Constant(Constant::Int(0))),
                    Type::I1,
                ) = flat[n - 1]
                else {
                    return None;
                };
                if name != "dartforge_exception_pending"
                    || !args.is_empty()
                    || p != pending
                    || c != test
                    || fail == success
                {
                    return None;
                }
                let failure = &f.blocks[*shape.positions.get(fail)?];
                if !failure.instructions.is_empty()
                    || !matches!(
                        failure.terminator,
                        Terminator::Return(Some(Operand::Constant(Constant::Null)))
                    )
                {
                    return None;
                }
                if values.get(call_value).is_none() {
                    return None;
                }
                guarded.insert(*call_value);
                failures.insert(*shape.positions.get(fail)?);
                bi = *shape.positions.get(success)?;
            }
            Terminator::Return(Some(Operand::Val(_))) => break,
            _ => return None,
        }
    }
    let warm_index = *shape.positions.get(&then_block)?;
    if predecessors[warm_index] != [0]
        || shape
            .reachable
            .iter()
            .any(|b| *b != 0 && *b != warm_index && !path.contains(b) && !failures.contains(b))
    {
        return None;
    }
    let (
        object,
        I::CallRuntime {
            name,
            args,
            ret_ty: Type::Ref,
        },
        Type::Ref,
    ) = *flat.first()?
    else {
        return None;
    };
    if name != "dartforge_object_new" {
        return None;
    }
    let [(Operand::Constant(Constant::Int(cid)), Type::I64), (Operand::Constant(Constant::Int(count)), Type::I64)] =
        args.as_slice()
    else {
        return None;
    };
    let cid = u32::try_from(*cid).ok()?;
    let layout = layouts.get(&cid)?;
    if cid < dartforge_runtime::layout::FIRST_FREE_CID as u32
        || cid > i32::MAX as u32
        || usize::try_from(*count).ok()? != layout.len()
    {
        return None;
    }
    let n = flat.len();
    if n < 5 {
        return None;
    }
    let (
        canonical,
        I::CallRuntime {
            name,
            args,
            ret_ty: Type::Ref,
        },
        Type::Ref,
    ) = flat[n - 4]
    else {
        return None;
    };
    if name != "dartforge_canonical_constant"
        || !matches!(args.as_slice(), [(Operand::Val(v),Type::Ref)] if v == object)
    {
        return None;
    }
    let (
        _,
        I::StoreGlobal {
            symbol,
            val: Operand::Val(stored),
            ty: Type::Ref,
            root: Some(root),
        },
        Type::Void,
    ) = flat[n - 3]
    else {
        return None;
    };
    if symbol != cache
        || stored != canonical
        || roots.get(cache).map(Vec::as_slice) != Some(&[(*root, Type::Ref)][..])
        || roots
            .values()
            .flatten()
            .filter(|(r, t)| r == root && *t == Type::Ref)
            .count()
            != 1
    {
        return None;
    }
    let (
        _,
        I::CallRuntime {
            name,
            args,
            ret_ty: Type::Void,
        },
        Type::Void,
    ) = flat[n - 2]
    else {
        return None;
    };
    if name != "dartforge_mark_constant"
        || !matches!(args.as_slice(), [(Operand::Val(v), Type::Ref), (Operand::Constant(Constant::Function(s)), Type::I64)] if v == canonical && s == &f.symbol)
    {
        return None;
    }
    let (
        _,
        I::StoreGlobal {
            symbol,
            val: Operand::Constant(Constant::Int(1)),
            ty: Type::I8,
            root: None,
        },
        Type::Void,
    ) = flat[n - 1]
    else {
        return None;
    };
    if symbol != flag
        || !matches!(f.blocks[bi].terminator, Terminator::Return(Some(Operand::Val(v))) if v == *canonical)
    {
        return None;
    }
    for slot in [cache, flag] {
        let writers = stores.get(slot)?;
        if writers.len() != 1 || !std::ptr::eq(writers[0].0, f) {
            return None;
        }
    }
    let mut fields = HashMap::new();
    let mut writes = Vec::new();
    for (v, i, ty) in &flat[1..n - 4] {
        match (i, ty) {
            (I::Const(Constant::Int(_)), Type::I64) => {}
            (
                I::CallStatic {
                    symbol,
                    args,
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ) if args.is_empty() && guarded.contains(v) && functions.contains_key(symbol) => {}
            (
                I::CallRuntime {
                    name,
                    args,
                    ret_ty: Type::I8,
                },
                Type::I8,
            ) if name == "dartforge_exception_pending" && args.is_empty() => {}
            (I::ICmp(ICmpOp::Ne, _, Operand::Constant(Constant::Int(0))), Type::I1) => {}
            (
                I::DeclaredFieldCall {
                    declaration,
                    name,
                    args,
                    ret_ty: Type::Void,
                },
                Type::Void,
            ) if name == "dartforge_object_set" => {
                let [(Operand::Val(receiver), Type::Ref), (Operand::Constant(Constant::Int(slot)), Type::I64), (value, Type::I64), (Operand::Constant(Constant::Int(1)), Type::I8)] =
                    args.as_slice()
                else {
                    return None;
                };
                let slot = usize::try_from(*slot).ok()?;
                if receiver != object
                    || !eligible(declaration)
                    || declaration.layout_index != slot
                    || layout.get(slot)?.as_ref()? != declaration
                    || fields.contains_key(&slot)
                    || shape.operand_type(value) != Some(Type::Ref)
                {
                    return None;
                }
                let classes = operand_classes(value, values)?;
                if classes.is_empty() {
                    return None;
                }
                fields.insert(slot, classes);
                writes.push(*v);
            }
            _ => return None,
        }
    }
    if layout.iter().enumerate().any(|(slot, d)| {
        d.as_ref()
            .is_none_or(|d| !eligible(d) || !fields.contains_key(&slot))
    }) {
        return None;
    }
    Some(Initialization {
        cid,
        allocation: *object,
        writes,
        fields,
    })
}

#[cfg(test)]
mod tests;
