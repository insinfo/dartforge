//! Contracts of scalar locals private to the activation, without aliases or escape.
//! Fields, Ref locals and imported addresses require other producers.

use super::{DartFunctionPlan, Ownership, TokenEffect, TokenPlan};
use crate::hir::*;
use crate::optimize::{
    cfg::Cfg,
    operands::{operands, terminator_operands},
};
use std::collections::{HashMap, HashSet};

pub(super) fn produce(
    f: &Function,
    classes: &mut HashMap<ValueId, Ownership>,
    plan: &mut TokenPlan,
) -> Result<HashMap<ValueId, Type>, String> {
    let (locals, produced_set) = analyze(f, classes, plan)?;
    for v in produced_set {
        classes.insert(v, Ownership::Trivial);
        plan.instructions.insert(v, TokenEffect::default());
    }
    for p in plan.received_addresses.keys() {
        classes.insert(*p, Ownership::Trivial);
    }
    Ok(locals)
}

/// Handle-free representations of a private local: scalars and SIMD vectors.
fn scalar(t: &Type) -> bool {
    matches!(t, Type::I1 | Type::I8 | Type::I64 | Type::F64) || t.is_vector()
}

/// By-address capture of direct local functions (`lower/direct_functions.rs`):
/// the `Ptr` parameter of the direct function points to the scalar `alloca` of
/// its declarer, which exists while it runs. The type of each parameter comes from its
/// uses (read, write, forwarding to another direct function) and from the addresses
/// the callers yield, as a fixpoint. Writes into the plans the received addresses
/// and, per call, the yielded ones; `analyze` checks each body against them.
///
/// # Errors
/// Divergent types for the same parameter, or a call that passes to a
/// scalar parameter something that is not a scalar local of the same type.
pub(super) fn produce_addresses(
    m: &Module,
    plans: &mut HashMap<String, DartFunctionPlan>,
) -> Result<(), String> {
    let directs = &m.arc_direct_local_functions;
    if directs.is_empty() {
        return Ok(());
    }
    let by_symbol: HashMap<&str, &Function> =
        m.functions.iter().map(|f| (f.symbol.as_str(), f)).collect();
    // (direct function, position of the Ptr parameter) → type of the pointed local.
    let mut types: HashMap<(&str, usize), Type> = HashMap::new();
    let ptr_of = |sym_name: &str, i: usize| {
        directs.contains(sym_name)
            && by_symbol
                .get(sym_name)
                .is_some_and(|g| g.params.get(i).is_some_and(|p| p.2 == Type::Ptr))
    };
    // Addresses of scalar locals visible in `f`: its allocas and, in a
    // direct function, the already typed parameters.
    let addresses =
        |f: &Function, types: &HashMap<(&str, usize), Type>| -> HashMap<ValueId, Type> {
            let mut e: HashMap<ValueId, Type> = f
                .blocks
                .first()
                .into_iter()
                .flat_map(|b| &b.instructions)
                .filter_map(|(v, inst, ty)| match inst {
                    Instruction::Alloca(t) if scalar(t) && *ty == Type::Ptr => Some((*v, *t)),
                    _ => None,
                })
                .collect();
            for (i, (v, _, _)) in f.params.iter().enumerate() {
                if let Some(t) = types.get(&(f.symbol.as_str(), i)) {
                    e.insert(*v, *t);
                }
            }
            e
        };
    fn define<'a>(
        types: &mut HashMap<(&'a str, usize), Type>,
        key: (&'a str, usize),
        t: Type,
    ) -> Result<bool, String> {
        match types.get(&key) {
            Some(current) if *current != t => Err(format!(
                "capture by address in {}: parameter {} read as {current:?} and {t:?}",
                key.0, key.1
            )),
            Some(_) => Ok(false),
            None => {
                types.insert(key, t);
                Ok(true)
            }
        }
    }
    loop {
        let mut changed = false;
        for f in &m.functions {
            let value_types: HashMap<ValueId, Type> = f
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
            let visible_set = addresses(f, &types);
            let parameter = |v: &ValueId| {
                f.params
                    .iter()
                    .position(|(p, _, t)| p == v && *t == Type::Ptr)
            };
            for (_, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
                match inst {
                    Instruction::Load {
                        ptr: Operand::Val(p),
                        ty,
                    } if directs.contains(&f.symbol) => {
                        if let Some(i) = parameter(p) {
                            changed |= define(&mut types, (f.symbol.as_str(), i), *ty)?;
                        }
                    }
                    Instruction::Store {
                        ptr: Operand::Val(p),
                        val: Operand::Val(x),
                    } if directs.contains(&f.symbol) => {
                        if let (Some(i), Some(t)) = (parameter(p), value_types.get(x)) {
                            changed |= define(&mut types, (f.symbol.as_str(), i), *t)?;
                        }
                    }
                    Instruction::CallStatic { symbol, args, .. } if directs.contains(symbol) => {
                        for (j, a) in args.iter().enumerate() {
                            let Operand::Val(a) = a else { continue };
                            if !ptr_of(symbol, j) {
                                continue;
                            }
                            // The caller yields a local of known type; a
                            // forwarded parameter inherits the callee's type.
                            if let Some(t) = visible_set.get(a) {
                                changed |= define(&mut types, (symbol.as_str(), j), *t)?;
                            } else if let (Some(i), Some(t)) = (
                                parameter(a).filter(|_| directs.contains(&f.symbol)),
                                types.get(&(symbol.as_str(), j)).copied(),
                            ) {
                                changed |= define(&mut types, (f.symbol.as_str(), i), t)?;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        if !changed {
            break;
        }
    }
    for f in &m.functions {
        let visible_set = addresses(f, &types);
        let plan = plans
            .get_mut(&f.symbol)
            .ok_or_else(|| format!("Dart function without a plan: {}", f.symbol))?;
        for (i, (v, _, _)) in f.params.iter().enumerate() {
            if let Some(t) = types.get(&(f.symbol.as_str(), i)).filter(|t| scalar(t)) {
                plan.tokens.received_addresses.insert(*v, *t);
            }
        }
        for (v, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            let Instruction::CallStatic { symbol, args, .. } = inst else {
                continue;
            };
            let mut yielded = Vec::new();
            for (j, a) in args.iter().enumerate() {
                let Some(t) = types.get(&(symbol.as_str(), j)).filter(|t| scalar(t)) else {
                    continue;
                };
                if !matches!(a, Operand::Val(a) if visible_set.get(a) == Some(t)) {
                    return Err(format!(
                        "capture by address in {}: v{} passes to parameter {j} of {symbol} something that is not a local {t:?}",
                        f.symbol, v.0
                    ));
                }
                yielded.push((j, *t));
            }
            if !yielded.is_empty() {
                plan.tokens.yielded_addresses.insert(*v, yielded);
            }
        }
    }
    Ok(())
}

/// Rebuilds the contract on the current body without filling in missing metadata.
pub(super) fn verify(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    plan: &TokenPlan,
) -> Result<(), String> {
    let (locals, produced_set) = analyze(f, classes, plan)?;
    for v in produced_set {
        if classes.get(&v) != Some(&Ownership::Trivial)
            || plan.instructions.get(&v) != Some(&TokenEffect::default())
        {
            return Err(format!(
                "v{}: scalar local without a complete contract",
                v.0
            ));
        }
    }
    for p in plan.received_addresses.keys() {
        if classes.get(p) != Some(&Ownership::Trivial) {
            return Err(format!(
                "v{}: captured address without a Trivial contract",
                p.0
            ));
        }
    }
    verify_values(f, &locals, classes)
}

fn analyze(
    f: &Function,
    classes: &HashMap<ValueId, Ownership>,
    plan: &TokenPlan,
) -> Result<(HashMap<ValueId, Type>, HashSet<ValueId>), String> {
    let mut locals = HashMap::new();
    for (bi, b) in f.blocks.iter().enumerate() {
        for (v, inst, ty) in &b.instructions {
            if let Instruction::Alloca(t) = inst
                && scalar(t)
            {
                if bi != 0 || *ty != Type::Ptr {
                    return Err(format!(
                        "v{}: scalar local requires a Ptr alloca at the entry",
                        v.0
                    ));
                }
                locals.insert(*v, *t);
            }
        }
    }
    // Addresses received from the declarer of the direct function: scalars, already
    // initialized, with the same usage rules as an own alloca.
    let mut received = HashSet::new();
    for (p, t) in &plan.received_addresses {
        if !f.params.iter().any(|(v, _, ty)| v == p && *ty == Type::Ptr) || !scalar(t) {
            return Err(format!(
                "v{}: captured address requires a scalar Ptr parameter",
                p.0
            ));
        }
        locals.insert(*p, *t);
        received.insert(*p);
    }
    let mut yielded_seen = HashSet::new();
    if locals.is_empty() {
        if let Some(v) = plan.yielded_addresses.keys().min_by_key(|v| v.0) {
            return Err(format!(
                "v{}: call lends an address without a scalar local",
                v.0
            ));
        }
        return Ok((locals, HashSet::new()));
    }
    super::ssa::verify(f)?;
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
    let local = |op: &Operand| match op {
        Operand::Val(v) if locals.contains_key(v) => Some(*v),
        _ => None,
    };
    let mut produced_set = HashSet::new();
    for b in &f.blocks {
        for (v, inst, ty) in &b.instructions {
            let mut allowed = None;
            let mut yielded: Vec<ValueId> = Vec::new();
            let mut expected = None;
            match inst {
                Instruction::CallStatic { args, .. } if plan.yielded_addresses.contains_key(v) => {
                    yielded_seen.insert(*v);
                    for (i, t) in &plan.yielded_addresses[v] {
                        match args.get(*i).and_then(local) {
                            Some(p) if locals[&p] == *t && !yielded.contains(&p) => yielded.push(p),
                            _ => {
                                return Err(format!(
                                    "v{}: argument {i} does not lend a distinct scalar local {t:?}",
                                    v.0
                                ));
                            }
                        }
                    }
                }
                Instruction::Alloca(_) if locals.contains_key(v) => expected = Some(Type::Ptr),
                Instruction::Load { ptr, ty: read_done } => {
                    if let Some(p) = local(ptr) {
                        allowed = Some(p);
                        if *read_done != locals[&p] {
                            return Err(format!(
                                "v{}: read diverges from the type of the local",
                                v.0
                            ));
                        }
                        expected = Some(*read_done);
                    }
                }
                Instruction::Store { ptr, val } => {
                    if let Some(p) = local(ptr) {
                        allowed = Some(p);
                        let typ = match val {
                            Operand::Val(v) => types.get(v).copied(),
                            // The late state uses literal bytes (0/1/2). The
                            // type comes from the slot, with the range checked before the
                            // textual coercion; does not apply to SSA values.
                            Operand::Constant(Constant::Int(n))
                                if locals[&p] == Type::I8 && (0..=255).contains(n) =>
                            {
                                Some(Type::I8)
                            }
                            Operand::Constant(Constant::Int(_)) => Some(Type::I64),
                            Operand::Constant(Constant::Double(_)) => Some(Type::F64),
                            Operand::Constant(Constant::Bool(_)) => Some(Type::I1),
                            _ => None,
                        };
                        if typ != Some(locals[&p]) {
                            return Err(format!(
                                "v{}: write diverges from the type of the local",
                                v.0
                            ));
                        }
                        expected = Some(Type::Void);
                    }
                }
                _ => {}
            }
            // Only the ptr operand of Load/Store may use the address. A
            // second occurrence (e.g. Store as the value) is also an escape.
            // A call to a direct function may yield each local once.
            let mut uses = 0;
            let mut unexpected = false;
            operands(inst, &mut |op| {
                if let Some(p) = local(op) {
                    uses += 1;
                    unexpected |= allowed != Some(p) && !yielded.contains(&p);
                }
            });
            if unexpected || uses != usize::from(allowed.is_some()) + yielded.len() {
                return Err(format!(
                    "v{}: address of a scalar local escapes or has an alias",
                    v.0
                ));
            }
            if let Some(expected) = expected {
                if *ty != expected
                    || classes.get(v).is_some_and(|c| *c != Ownership::Trivial)
                    || plan
                        .instructions
                        .get(v)
                        .is_some_and(|e| *e != TokenEffect::default())
                {
                    return Err(format!(
                        "v{}: contract incompatible with a scalar local",
                        v.0
                    ));
                }
                produced_set.insert(*v);
            }
        }
        let mut escapes = false;
        terminator_operands(&b.terminator, &mut |op| {
            escapes |= local(op).is_some();
        });
        if escapes {
            return Err(format!(
                "b{}: address of a scalar local escapes in the terminator",
                b.id.0
            ));
        }
    }
    if let Some(v) = plan
        .yielded_addresses
        .keys()
        .filter(|v| !yielded_seen.contains(v))
        .min_by_key(|v| v.0)
    {
        return Err(format!("v{}: addresses lent outside a direct call", v.0));
    }
    let cfg = Cfg::new(f);
    let universe: HashSet<_> = locals.keys().copied().collect();
    // Must analysis: the intersection keeps only initializations present
    // in every predecessor. The top in loops decreases until the fixpoint.
    let mut entries = vec![universe.clone(); f.blocks.len()];
    let mut exits = entries.clone();
    let transfer = |bi: usize, mut state: HashSet<ValueId>| {
        for (v, inst, _) in &f.blocks[bi].instructions {
            match inst {
                Instruction::Alloca(_) if locals.contains_key(v) => {
                    state.remove(v);
                }
                Instruction::Store { ptr, .. } => {
                    if let Some(p) = local(ptr) {
                        state.insert(p);
                    }
                }
                _ => {}
            }
        }
        state
    };
    loop {
        let mut changed = false;
        for &bi in &cfg.rpo {
            let mut entry = if bi == 0 {
                received.clone()
            } else {
                universe.clone()
            };
            if bi != 0 {
                for &p in &cfg.predecessors[bi] {
                    if cfg.reachable(p) {
                        entry.retain(|v| exits[p].contains(v));
                    }
                }
            }
            let exit = transfer(bi, entry.clone());
            changed |= entries[bi] != entry || exits[bi] != exit;
            entries[bi] = entry;
            exits[bi] = exit;
        }
        if !changed {
            break;
        }
    }
    for (bi, b) in f.blocks.iter().enumerate() {
        let mut state = if cfg.reachable(bi) {
            entries[bi].clone()
        } else {
            HashSet::new()
        };
        for (v, inst, _) in &b.instructions {
            match inst {
                Instruction::Alloca(_) if locals.contains_key(v) => {
                    state.remove(v);
                }
                Instruction::Store { ptr, .. } => {
                    if let Some(p) = local(ptr) {
                        state.insert(p);
                    }
                }
                Instruction::Load { ptr, .. } => {
                    if let Some(p) = local(ptr)
                        && !state.contains(&p)
                    {
                        return Err(format!(
                            "v{}: scalar local not initialized on all paths",
                            v.0
                        ));
                    }
                }
                // The direct function may read the yielded local: like a read.
                Instruction::CallStatic { args, .. } if plan.yielded_addresses.contains_key(v) => {
                    for (i, _) in &plan.yielded_addresses[v] {
                        if let Some(p) = args.get(*i).and_then(local)
                            && !state.contains(&p)
                        {
                            return Err(format!(
                                "v{}: lent scalar local not initialized on all paths",
                                v.0
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok((locals, produced_set))
}

pub(super) fn verify_values(
    f: &Function,
    locals: &HashMap<ValueId, Type>,
    classes: &HashMap<ValueId, Ownership>,
) -> Result<(), String> {
    for (v, inst, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
        if let Instruction::Store {
            ptr: Operand::Val(p),
            val: Operand::Val(origin),
        } = inst
            && locals.contains_key(p)
            && classes.get(origin) != Some(&Ownership::Trivial)
        {
            return Err(format!(
                "v{}: write of a scalar local requires a Trivial origin",
                v.0
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optimize::arc::{
        OwnerOrigin, ScopePlan, produce_and_verify_tokens, produce_arc_contracts,
    };

    fn diamond() -> Function {
        Function {
            symbol: "local".into(),
            name: "local".into(),
            debug_info: None,
            params: vec![(ValueId(0), "cond".into(), Type::I1)],
            return_ty: Type::Void,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(ValueId(1), Instruction::Alloca(Type::I64), Type::Ptr)],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(0)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(3),
                    instructions: vec![
                        (
                            ValueId(4),
                            Instruction::Load {
                                ptr: Operand::Val(ValueId(1)),
                                ty: Type::I64,
                            },
                            Type::I64,
                        ),
                        (
                            ValueId(5),
                            Instruction::IntToDouble(Operand::Val(ValueId(4))),
                            Type::F64,
                        ),
                    ],
                    terminator: Terminator::Return(None),
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![(
                        ValueId(3),
                        Instruction::Store {
                            ptr: Operand::Val(ValueId(1)),
                            val: Operand::Constant(Constant::Int(9)),
                        },
                        Type::Void,
                    )],
                    terminator: Terminator::Branch(BlockId(3)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![(
                        ValueId(2),
                        Instruction::Store {
                            ptr: Operand::Val(ValueId(1)),
                            val: Operand::Constant(Constant::Int(7)),
                        },
                        Type::Void,
                    )],
                    terminator: Terminator::Branch(BlockId(3)),
                },
            ],
        }
    }

    #[test]
    fn scalar_local_requires_initialization_on_all_edges() {
        let mut f = diamond();
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        let mut plan = TokenPlan::default();
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        assert_eq!(classes.len(), 6);
        assert!(classes.values().all(|c| *c == Ownership::Trivial));
        f.blocks[2].instructions.clear();
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        let before = classes.clone();
        let mut plan = TokenPlan::default();
        assert!(
            produce_arc_contracts(&f, &mut classes, &mut plan)
                .unwrap_err()
                .contains("not initialized")
        );
        assert_eq!(classes, before);
        assert!(plan.instructions.is_empty());
    }

    #[test]
    fn loop_local_requires_write_before_first_iteration() {
        let mut f = Function {
            symbol: "laco".into(),
            name: "laco".into(),
            debug_info: None,
            params: vec![(ValueId(0), "cond".into(), Type::I1)],
            return_ty: Type::Void,
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(ValueId(1), Instruction::Alloca(Type::I64), Type::Ptr)],
                    terminator: Terminator::Branch(BlockId(1)),
                },
                BasicBlock {
                    id: BlockId(1),
                    instructions: vec![
                        (
                            ValueId(2),
                            Instruction::Load {
                                ptr: Operand::Val(ValueId(1)),
                                ty: Type::I64,
                            },
                            Type::I64,
                        ),
                        (
                            ValueId(3),
                            Instruction::Store {
                                ptr: Operand::Val(ValueId(1)),
                                val: Operand::Constant(Constant::Int(9)),
                            },
                            Type::Void,
                        ),
                    ],
                    terminator: Terminator::CondBranch {
                        cond: Operand::Val(ValueId(0)),
                        then_block: BlockId(1),
                        else_block: BlockId(2),
                    },
                },
                BasicBlock {
                    id: BlockId(2),
                    instructions: vec![],
                    terminator: Terminator::Return(None),
                },
            ],
        };
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        let mut plan = TokenPlan::default();
        assert!(
            produce_arc_contracts(&f, &mut classes, &mut plan)
                .unwrap_err()
                .contains("not initialized")
        );
        assert_eq!(classes.len(), 1);
        assert!(plan.instructions.is_empty());
        f.blocks[0].instructions.push((
            ValueId(4),
            Instruction::Store {
                ptr: Operand::Val(ValueId(1)),
                val: Operand::Constant(Constant::Int(7)),
            },
            Type::Void,
        ));
        produce_and_verify_tokens(
            &f,
            &mut classes,
            &mut plan,
            &FunctionTables::default(),
            &ScopePlan::default(),
        )
        .unwrap();
        assert_eq!(classes.len(), 5);
    }

    #[test]
    fn typed_locals_feed_producers_without_inferring_by_width() {
        for (ty, value) in [
            (Type::I64, Instruction::Const(Constant::Int(7))),
            (Type::F64, Instruction::Const(Constant::Double(-0.0))),
            (Type::I1, Instruction::Const(Constant::Bool(true))),
            (
                Type::I8,
                Instruction::ZExt {
                    op: Operand::Constant(Constant::Bool(true)),
                    from: Type::I1,
                    to: Type::I8,
                },
            ),
        ] {
            let f = Function {
                symbol: "tipo_local".into(),
                name: "tipo_local".into(),
                debug_info: None,
                params: vec![],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (ValueId(0), Instruction::Alloca(ty), Type::Ptr),
                        (ValueId(1), value, ty),
                        (
                            ValueId(2),
                            Instruction::Store {
                                ptr: Operand::Val(ValueId(0)),
                                val: Operand::Val(ValueId(1)),
                            },
                            Type::Void,
                        ),
                        (
                            ValueId(3),
                            Instruction::Load {
                                ptr: Operand::Val(ValueId(0)),
                                ty,
                            },
                            ty,
                        ),
                    ],
                    terminator: Terminator::Return(None),
                }],
            };
            let mut classes = HashMap::new();
            let mut plan = TokenPlan::default();
            produce_and_verify_tokens(
                &f,
                &mut classes,
                &mut plan,
                &FunctionTables::default(),
                &ScopePlan::default(),
            )
            .unwrap();
            assert_eq!(classes.len(), 4);
            assert!(classes.values().all(|c| *c == Ownership::Trivial));
            let mut module = Module {
                arc_memory: true,
                ..Module::default()
            };
            module.functions.push(f);
            let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
            assert!(ir.contains(&format!("load {}, ptr %v0", ty.llvm_ir())));
        }
    }

    #[test]
    fn module_links_scalar_read_to_owned_boxing_without_changing_tracing() {
        use crate::optimize::arc::{DartFunctionPlan, prepare_arc_dart_module};
        for arc in [false, true] {
            let mut f = diamond();
            f.blocks[1].instructions.push((
                ValueId(6),
                Instruction::Box {
                    op: Operand::Val(ValueId(4)),
                    from: Type::I64,
                },
                Type::Ref,
            ));
            let mut module = Module {
                arc_memory: arc,
                ..Module::default()
            };
            module.functions.push(f);
            let mut plans = HashMap::from([("local".into(), DartFunctionPlan::default())]);
            let before = format!("{module:?}/{plans:?}");
            assert_eq!(
                prepare_arc_dart_module(&mut module, &mut plans).unwrap(),
                if arc { (0, 1) } else { (0, 0) }
            );
            if arc {
                assert_eq!(plans["local"].classes[&ValueId(4)], Ownership::Trivial);
                assert_eq!(plans["local"].classes[&ValueId(6)], Ownership::Owned);
                assert!(matches!(
                    module.functions[0].blocks[1].instructions.last().unwrap().1,
                    Instruction::ArcDrop { .. }
                ));
                assert_eq!(
                    prepare_arc_dart_module(&mut module, &mut plans).unwrap(),
                    (0, 0)
                );
                module.functions[0].blocks[2].instructions.clear();
                let before = format!("{module:?}/{plans:?}");
                assert!(prepare_arc_dart_module(&mut module, &mut plans).is_err());
                assert_eq!(format!("{module:?}/{plans:?}"), before);
            } else {
                assert_eq!(format!("{module:?}/{plans:?}"), before);
            }
        }
    }

    #[test]
    fn verifier_rejects_stale_local_plan_after_body_change() {
        use crate::optimize::arc::verify_tokens;
        let base = diamond();
        let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
        let mut plan = TokenPlan::default();
        produce_arc_contracts(&base, &mut classes, &mut plan).unwrap();
        verify_tokens(&base, &classes, &FunctionTables::default(), &plan).unwrap();
        for case in 0..5 {
            let mut f = base.clone();
            let mut classes = classes.clone();
            let mut plan = plan.clone();
            match case {
                0 => {
                    f.blocks[2].instructions.clear();
                    classes.remove(&ValueId(3));
                    plan.instructions.remove(&ValueId(3));
                }
                1 => {
                    f.blocks[1].instructions.push((
                        ValueId(6),
                        Instruction::CallRuntime {
                            name: "extern_sem_ownership".into(),
                            args: vec![(Operand::Val(ValueId(1)), Type::Ptr)],
                            ret_ty: Type::Void,
                        },
                        Type::Void,
                    ));
                    classes.insert(ValueId(6), Ownership::Trivial);
                    plan.instructions.insert(ValueId(6), TokenEffect::default());
                }
                2 => {
                    f.blocks[1].instructions[0].1 = Instruction::Load {
                        ptr: Operand::Val(ValueId(1)),
                        ty: Type::F64,
                    }
                }
                3 => {
                    f.blocks[2].instructions[0].1 = Instruction::Store {
                        ptr: Operand::Val(ValueId(1)),
                        val: Operand::Constant(Constant::Null),
                    }
                }
                4 => {
                    plan.instructions.get_mut(&ValueId(4)).unwrap().may_fail = true;
                    plan.pendings.insert(ValueId(4), BlockId(2));
                }
                _ => unreachable!(),
            }
            let before = format!("{classes:?}/{plan:?}");
            assert!(
                verify_tokens(&f, &classes, &FunctionTables::default(), &plan).is_err(),
                "caso {case}"
            );
            assert_eq!(format!("{classes:?}/{plan:?}"), before);
        }
    }

    #[test]
    fn byte_literal_has_explicit_range_in_slot() {
        for n in [0, 1, 2, 255, -1, 256] {
            let f = Function {
                symbol: "byte".into(),
                name: "byte".into(),
                debug_info: None,
                params: vec![],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (ValueId(0), Instruction::Alloca(Type::I8), Type::Ptr),
                        (
                            ValueId(1),
                            Instruction::Store {
                                ptr: Operand::Val(ValueId(0)),
                                val: Operand::Constant(Constant::Int(n)),
                            },
                            Type::Void,
                        ),
                        (
                            ValueId(2),
                            Instruction::Load {
                                ptr: Operand::Val(ValueId(0)),
                                ty: Type::I8,
                            },
                            Type::I8,
                        ),
                    ],
                    terminator: Terminator::Return(None),
                }],
            };
            let mut classes = HashMap::new();
            let mut plan = TokenPlan::default();
            let result = produce_arc_contracts(&f, &mut classes, &mut plan);
            assert_eq!(result.is_ok(), (0..=255).contains(&n));
            if result.is_err() {
                assert!(classes.is_empty());
                assert!(plan.instructions.is_empty());
            }
        }
    }

    #[test]
    fn scalar_local_rejects_escape_and_managed_write_atomically() {
        let base = diamond();
        for case in 0..6 {
            let mut f = base.clone();
            match case {
                0 => f.blocks[1].instructions.push((
                    ValueId(6),
                    Instruction::CallRuntime {
                        name: "sem_contrato".into(),
                        args: vec![(Operand::Val(ValueId(1)), Type::Ptr)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                )),
                1 => f.blocks[1].terminator = Terminator::Return(Some(Operand::Val(ValueId(1)))),
                2 => {
                    f.blocks[1].instructions[0].1 = Instruction::Load {
                        ptr: Operand::Val(ValueId(1)),
                        ty: Type::Ref,
                    }
                }
                3 => {
                    f.blocks[2].instructions[0].1 = Instruction::Store {
                        ptr: Operand::Val(ValueId(1)),
                        val: Operand::Constant(Constant::Null),
                    }
                }
                4 => f.blocks[0].instructions[0].2 = Type::I64,
                5 => {
                    let a = f.blocks[0].instructions.remove(0);
                    f.blocks[3].instructions.insert(0, a);
                }
                _ => unreachable!(),
            }
            let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
            let before = classes.clone();
            let mut plan = TokenPlan::default();
            assert!(
                produce_arc_contracts(&f, &mut classes, &mut plan).is_err(),
                "caso {case}"
            );
            assert_eq!(classes, before);
            assert!(plan.instructions.is_empty());
        }
        let mut f = base;
        f.params.push((ValueId(6), "valor".into(), Type::I64));
        f.blocks[2].instructions[0].1 = Instruction::Store {
            ptr: Operand::Val(ValueId(1)),
            val: Operand::Val(ValueId(6)),
        };
        for class in [
            None,
            Some(Ownership::Owned),
            Some(Ownership::Borrowed {
                owner: OwnerOrigin::Caller,
                scope: 0,
            }),
        ] {
            let mut classes = HashMap::from([(ValueId(0), Ownership::Trivial)]);
            if let Some(c) = class {
                classes.insert(ValueId(6), c);
            }
            let before = classes.clone();
            let mut plan = TokenPlan::default();
            assert!(
                produce_arc_contracts(&f, &mut classes, &mut plan)
                    .unwrap_err()
                    .contains("Trivial origin")
            );
            assert_eq!(classes, before);
            assert!(plan.instructions.is_empty());
        }
    }

    #[test]
    fn by_address_capture_links_caller_local_to_direct_parameter() {
        use crate::optimize::arc::{DartFunctionPlan, prepare_arc_dart_module};
        let val = |n| Operand::Val(ValueId(n));
        for case in 0..4 {
            let mut m = Module::new();
            m.arc_memory = true;
            if case != 3 {
                m.arc_direct_local_functions.insert("direta".into());
            }
            let local = if case == 1 { Type::F64 } else { Type::I64 };
            let mut outside = vec![
                (ValueId(0), Instruction::Alloca(local), Type::Ptr),
                (
                    ValueId(1),
                    Instruction::Store {
                        ptr: val(0),
                        val: Operand::Constant(if case == 1 {
                            Constant::Double(1.0)
                        } else {
                            Constant::Int(1)
                        }),
                    },
                    Type::Void,
                ),
                (
                    ValueId(2),
                    Instruction::CallStatic {
                        symbol: "direta".into(),
                        args: vec![val(0)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                ),
            ];
            if case == 2 {
                // Yielded before the first write.
                outside.swap(1, 2);
            }
            m.functions.push(Function {
                symbol: "fora".into(),
                name: "fora".into(),
                debug_info: None,
                params: vec![],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: outside,
                    terminator: Terminator::Return(None),
                }],
            });
            m.functions.push(Function {
                symbol: "direta".into(),
                name: "direta".into(),
                debug_info: None,
                params: vec![(ValueId(0), "x".into(), Type::Ptr)],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![
                        (
                            ValueId(1),
                            Instruction::Load {
                                ptr: val(0),
                                ty: Type::I64,
                            },
                            Type::I64,
                        ),
                        (
                            ValueId(2),
                            Instruction::Add(val(1), Operand::Constant(Constant::Int(1))),
                            Type::I64,
                        ),
                        (
                            ValueId(3),
                            Instruction::Store {
                                ptr: val(0),
                                val: val(2),
                            },
                            Type::Void,
                        ),
                    ],
                    terminator: Terminator::Return(None),
                }],
            });
            let mut plans: HashMap<String, DartFunctionPlan> = ["fora", "direta"]
                .map(|s| (s.to_string(), DartFunctionPlan::default()))
                .into();
            let before = format!("{m:?}");
            let result = prepare_arc_dart_module(&mut m, &mut plans);
            if case != 0 {
                let error = result.unwrap_err();
                assert!(
                    match case {
                        1 => error.contains("read as"),
                        2 => error.contains("lent scalar local not initialized"),
                        _ => error.contains("escapes"),
                    },
                    "{case}: {error}"
                );
                assert_eq!(format!("{m:?}"), before);
                continue;
            }
            result.unwrap();
            assert_eq!(
                plans["direta"].tokens.received_addresses,
                HashMap::from([(ValueId(0), Type::I64)])
            );
            assert_eq!(plans["direta"].classes[&ValueId(0)], Ownership::Trivial);
            assert_eq!(
                plans["fora"].tokens.yielded_addresses,
                HashMap::from([(ValueId(2), vec![(0, Type::I64)])])
            );
        }
    }
}
