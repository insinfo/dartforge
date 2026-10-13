//! Lifetime of frame/index pairs in a closed set of intermediate bodies.
//! Checks explicit CFG exits; implicit unwinding and tokens require another gate.

use super::super::operands::{operands, terminator_operands};
use super::{ArcRefSlotsDraft, audit_shared_ref_slots};
use crate::hir::*;
use std::collections::{HashMap, HashSet, VecDeque};

/// Verifies the origin, forwarding and explicit lifetime of the lowered shared slots.
///
/// Rebuilds the roles from the audited addresses of the original module.
/// Checks current signatures, capacity and activity of local frames, the
/// association of the imported index, absence of escape and every caller.
/// Forwarding cycles need a reachable local origin. The callee may
/// read/copy the slot, but not close, move or publish the caller's frame.
/// Changes neither the original nor the draft and publishes no emission certificate.
/// Does not verify Ref tokens nor foreign unwinding outside the CFG edges.
/// Tracing requires bodies copied without ARC transformations.
///
/// # Errors
/// Metadata incompatible with the origin, invalid signature/type, escape,
/// forwarding without origin, wrong index, closed frame or cleanup out of LIFO order.
///
/// ```
/// use dartforge_emit_native::{hir::Module, optimize::arc::*};
/// let mut m = Module::new(); m.arc_memory = true;
/// let r = lower_shared_ref_slots(&m)?;
/// verify_cfg_ref_slot_lives(&m, &r)?;
/// # Ok::<(), String>(())
/// ```
pub fn verify_cfg_ref_slot_lives(m: &Module, r: &ArcRefSlotsDraft) -> Result<(), String> {
    if !m.arc_memory {
        if !r.borrowed_set.is_empty()
            || !r.locals.is_empty()
            || !r.symbols.is_empty()
            || format!("{:?}", m.functions) != format!("{:?}", r.functions)
        {
            return Err("slots ARC: tracing não admite transformação de capturas".into());
        }
        return Ok(());
    }
    let origins = audit_shared_ref_slots(m)?;
    let mut new_ones = HashMap::new();
    for f in &r.functions {
        if new_ones.insert(f.symbol.as_str(), f).is_some() {
            return Err("slots ARC: símbolo novo repetido".into());
        }
    }
    let mut expected_set = HashSet::new();
    let mut imports = HashSet::new();
    let mut locals = HashSet::new();
    let mut renamed = HashSet::new();
    for f in &m.functions {
        let name_str = r.symbols.get(&f.symbol).unwrap_or(&f.symbol);
        if !expected_set.insert(name_str.as_str()) {
            return Err("slots ARC: colisão entre símbolos novos".into());
        }
        let new = new_ones
            .get(name_str.as_str())
            .ok_or_else(|| format!("slots ARC: corpo ausente {name_str}"))?;
        let ids = origins.get(&f.symbol);
        let captures: HashSet<_> = f
            .params
            .iter()
            .filter_map(|(v, _, _)| ids.is_some_and(|ss| ss.contains(v)).then_some(*v))
            .collect();
        let defs: HashSet<_> = f
            .params
            .iter()
            .map(|(v, _, _)| *v)
            .chain(
                f.blocks
                    .iter()
                    .flat_map(|b| &b.instructions)
                    .map(|(v, _, _)| *v),
            )
            .collect();
        let mut signature = Vec::new();
        if !captures.is_empty() {
            imports.insert(name_str.as_str());
            renamed.insert(f.symbol.as_str());
            if name_str == &f.symbol {
                return Err(format!("slots ARC: ABI nova reutiliza símbolo {name_str}"));
            }
            let pairs = r
                .borrowed_set
                .get(name_str)
                .ok_or_else(|| format!("slots ARC: pares ausentes de {name_str}"))?;
            if pairs.keys().copied().collect::<HashSet<_>>() != captures {
                return Err(format!(
                    "slots ARC: parâmetros não correspondem às capturas de {name_str}"
                ));
            }
            for (v, _, ty) in &f.params {
                if let Some(i) = pairs.get(v) {
                    if defs.contains(i) {
                        return Err(format!(
                            "slots ARC: índice de {name_str} reutiliza SSA da fonte"
                        ));
                    }
                    signature.extend([(*v, Type::I64), (*i, Type::I64)]);
                } else {
                    signature.push((*v, *ty));
                }
            }
        } else {
            signature.extend(f.params.iter().map(|(v, _, t)| (*v, *t)));
        }
        if signature
            != new
                .params
                .iter()
                .map(|(v, _, t)| (*v, *t))
                .collect::<Vec<_>>()
            || new.return_ty != f.return_ty
        {
            return Err(format!(
                "slots ARC: assinatura diferente da origem de {name_str}"
            ));
        }
        let quantity = f
            .blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .filter(|(v, i, _)| {
                ids.is_some_and(|ss| ss.contains(v)) && matches!(i, Instruction::Alloca(Type::Ref))
            })
            .count();
        if quantity != 0 {
            locals.insert(name_str.as_str());
            let &(q, n) = r
                .locals
                .get(name_str)
                .ok_or_else(|| format!("slots ARC: quadro local ausente de {name_str}"))?;
            if n != quantity
                || defs.contains(&q)
                || !new.blocks.first().is_some_and(|b| {
                    b.instructions.iter().any(|(v, i, t)| {
                        *v == q
                            && *t == Type::I64
                            && matches!(i, Instruction::CallRuntime { name, args, ret_ty }
                    if name == "dartforge_arc_quadro_abrir_v1" && *ret_ty == Type::I64
                    && args == &vec![(Operand::Constant(Constant::Int(n as i64)), Type::I64)])
                    })
                })
            {
                return Err(format!(
                    "slots ARC: fábrica/capacidade do quadro local inválida em {name_str}"
                ));
            }
        }
    }
    if new_ones.keys().copied().collect::<HashSet<_>>() != expected_set
        || r.borrowed_set
            .keys()
            .map(String::as_str)
            .collect::<HashSet<_>>()
            != imports
        || r.locals.keys().map(String::as_str).collect::<HashSet<_>>() != locals
        || r.symbols
            .keys()
            .map(String::as_str)
            .collect::<HashSet<_>>()
            != renamed
    {
        return Err("slots ARC: inventário de corpos/papéis incompatível com a origem".into());
    }
    let mut targets = HashMap::new();
    for (name_str, pairs) in &r.borrowed_set {
        let f = new_ones[name_str.as_str()];
        let positions: HashMap<_, _> = f
            .params
            .iter()
            .enumerate()
            .map(|(p, (v, _, _))| (*v, p))
            .collect();
        let mut order: Vec<_> = pairs
            .iter()
            .map(|(q, i)| (positions[q], positions[i]))
            .collect();
        order.sort_unstable();
        targets.insert(name_str.clone(), order);
    }
    let mut anchored = HashSet::<(String, ValueId)>::new();
    let mut edges = HashMap::<(String, ValueId), Vec<(String, ValueId)>>::new();
    for f in &r.functions {
        // A materialized address reopens the boundary even if the original
        // was closed. Also verify functions without local/imported slots.
        for b in &f.blocks {
            let mut open = false;
            let mut constant = |o: &Operand| {
                open |=
                    matches!(o, Operand::Constant(Constant::Function(s)) if targets.contains_key(s));
            };
            for (_, i, _) in &b.instructions {
                match i {
                    Instruction::Const(Constant::Function(s))
                    | Instruction::TearOff { code_symbol: s }
                    | Instruction::AllocClosure { code_symbol: s, .. } => {
                        if targets.contains_key(s) {
                            return Err("slots ARC: entrada emprestada materializada".into());
                        }
                    }
                    Instruction::AllocClosureTyped {
                        code_symbol,
                        typed,
                        ..
                    } if targets.contains_key(code_symbol) || targets.contains_key(typed) => {
                        return Err("slots ARC: entrada emprestada materializada".into());
                    }
                    Instruction::FunctionTable(ss)
                        if ss.iter().any(|s| targets.contains_key(s)) =>
                    {
                        return Err("slots ARC: entrada emprestada materializada".into());
                    }
                    _ => {}
                }
                operands(i, &mut constant);
            }
            terminator_operands(&b.terminator, &mut constant);
            if open {
                return Err("slots ARC: entrada emprestada materializada".into());
            }
        }
        if !r.borrowed_set.contains_key(&f.symbol) && !r.locals.contains_key(&f.symbol)
            && !f.blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i, Instruction::CallStatic { symbol, .. } if targets.contains_key(symbol))) { continue; }
        super::ssa::verify(f)?;
        let empty = HashMap::new();
        let pairs = r.borrowed_set.get(&f.symbol).unwrap_or(&empty);
        let entries = super::frames::analyze_shared(f, pairs, &targets)?;
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
        let typ = |op: &Operand| match op {
            Operand::Val(v) => types.get(v).copied(),
            Operand::Constant(Constant::Null | Constant::String(_)) => Some(Type::Ref),
            Operand::Constant(Constant::Int(_)) => Some(Type::I64),
            Operand::Constant(Constant::Double(_)) => Some(Type::F64),
            Operand::Constant(Constant::Bool(_)) => Some(Type::I1),
            _ => None,
        };
        let own_set: HashSet<_> = f.blocks.iter().flat_map(|b| &b.instructions).filter_map(|(v, i, _)|
            matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_arc_quadro_abrir_v1").then_some(*v)).collect();
        let natives: HashSet<_> = own_set
            .iter()
            .copied()
            .chain(pairs.keys().copied())
            .chain(pairs.values().copied())
            .collect();
        for b in &f.blocks {
            for (v, inst, ty) in &b.instructions {
                let mut allowed = Vec::new();
                let mut allow = |o: &Operand| {
                    if let Operand::Val(v) = o
                        && natives.contains(v)
                    {
                        allowed.push(*v);
                    }
                };
                match inst {
                    Instruction::CallRuntime { name, args, ret_ty }
                        if name.starts_with("dartforge_arc_quadro_") =>
                    {
                        super::contracts::runtime_call_contract(inst)?;
                        if *ty != *ret_ty || args.iter().any(|(a, t)| typ(a) != Some(*t)) {
                            return Err(format!(
                                "slots ARC: tipos reais da operação em {} v{}",
                                f.symbol, v.0
                            ));
                        }
                        if name != "dartforge_arc_quadro_abrir_v1" {
                            allow(&args[0].0);
                            if let Operand::Val(q) = args[0].0
                                && let Some(i) = pairs.get(&q)
                            {
                                if args.get(1).is_some_and(|a| a.0 == Operand::Val(*i)) {
                                    allow(&args[1].0);
                                }
                            }
                            if name == "dartforge_arc_quadro_mover_v1" {
                                allow(&args[2].0);
                            }
                        }
                    }
                    Instruction::ArcLoadStrong {
                        slot: StrongSlot::Frame { frame, .. },
                    }
                    | Instruction::ArcStoreStrong {
                        slot: StrongSlot::Frame { frame, .. },
                        ..
                    } => allow(frame),
                    Instruction::CallStatic {
                        symbol,
                        args,
                        ret_ty,
                    } if targets.contains_key(symbol) => {
                        if !entries.contains_key(&b.id) {
                            return Err("slots ARC: repasse em bloco sem origem alcançável".into());
                        }
                        let callee = new_ones[symbol.as_str()];
                        if args.len() != callee.params.len()
                            || *ty != *ret_ty
                            || *ret_ty != callee.return_ty
                            || args
                                .iter()
                                .zip(&callee.params)
                                .any(|(a, (_, _, t))| typ(a) != Some(*t))
                        {
                            return Err(format!(
                                "slots ARC: assinatura/tipos reais da chamada de {} para {symbol}",
                                f.symbol
                            ));
                        }
                        for &(qi, ii) in &targets[symbol] {
                            allow(&args[qi]);
                            allow(&args[ii]);
                            let dest = (symbol.clone(), callee.params[qi].0);
                            let Operand::Val(q) = args[qi] else {
                                return Err("slots ARC: quadro de chamada não SSA".into());
                            };
                            if pairs.contains_key(&q) {
                                edges
                                    .entry((f.symbol.clone(), q))
                                    .or_default()
                                    .push(dest);
                            } else if own_set.contains(&q) && entries.contains_key(&b.id) {
                                anchored.insert(dest);
                            } else {
                                return Err("slots ARC: repasse sem origem local alcançável".into());
                            }
                        }
                    }
                    _ => {}
                }
                let mut used_set = Vec::new();
                operands(inst, &mut |o| {
                    if let Operand::Val(v) = o
                        && natives.contains(v)
                    {
                        used_set.push(*v);
                    }
                });
                if used_set != allowed {
                    return Err(format!(
                        "slots ARC: identidade nativa escapa/ganha alias em {} v{}",
                        f.symbol, v.0
                    ));
                }
            }
            let mut escapes = false;
            terminator_operands(&b.terminator, &mut |o| {
                escapes |= matches!(o, Operand::Val(v) if natives.contains(v));
            });
            if escapes {
                return Err(format!(
                    "slots ARC: identidade nativa escapa no terminador de {}",
                    f.symbol
                ));
            }
        }
    }
    let mut queue: VecDeque<_> = anchored.iter().cloned().collect();
    while let Some(origin) = queue.pop_front() {
        for dest in edges.get(&origin).into_iter().flatten() {
            if anchored.insert(dest.clone()) {
                queue.push_back(dest.clone());
            }
        }
    }
    if r.borrowed_set
        .iter()
        .any(|(s, ps)| ps.keys().any(|q| !anchored.contains(&(s.clone(), *q))))
    {
        return Err("slots ARC: ciclo de quadros emprestados sem origem local alcançável".into());
    }
    Ok(())
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::lower_shared_ref_slots;
    use super::*;

    fn v(n: u32) -> Operand {
        Operand::Val(ValueId(n))
    }
    fn call(name_str: &str, args: Vec<Operand>) -> Instruction {
        Instruction::CallStatic {
            symbol: name_str.into(),
            args,
            ret_ty: Type::Void,
        }
    }
    fn f(name_str: &str, params: bool, instructions: Vec<(ValueId, Instruction, Type)>) -> Function {
        Function {
            symbol: name_str.into(),
            name: name_str.into(),
            debug_info: None,
            params: if params {
                vec![
                    (ValueId(0), "a".into(), Type::Ptr),
                    (ValueId(1), "b".into(), Type::Ptr),
                ]
            } else {
                vec![]
            },
            return_ty: Type::Void,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions,
                terminator: Terminator::Return(None),
            }],
        }
    }
    pub(in crate::optimize::arc) fn origin() -> Module {
        let mut m = Module::new();
        m.arc_memory = true;
        m.functions = vec![
            f(
                "raiz",
                false,
                vec![
                    (ValueId(0), Instruction::Alloca(Type::Ref), Type::Ptr),
                    (ValueId(1), Instruction::Alloca(Type::Ref), Type::Ptr),
                    (
                        ValueId(2),
                        Instruction::Store {
                            ptr: v(0),
                            val: Operand::Constant(Constant::Null),
                        },
                        Type::Void,
                    ),
                    (
                        ValueId(3),
                        Instruction::Store {
                            ptr: v(1),
                            val: Operand::Constant(Constant::Null),
                        },
                        Type::Void,
                    ),
                    (ValueId(4), call("a", vec![v(0), v(1)]), Type::Void),
                ],
            ),
            f(
                "a",
                true,
                vec![
                    (
                        ValueId(2),
                        Instruction::Load {
                            ptr: v(0),
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(3),
                        Instruction::Store {
                            ptr: v(1),
                            val: v(2),
                        },
                        Type::Void,
                    ),
                    (ValueId(4), call("b", vec![v(0), v(1)]), Type::Void),
                ],
            ),
            f(
                "b",
                true,
                vec![(ValueId(2), call("a", vec![v(0), v(1)]), Type::Void)],
            ),
        ];
        m
    }

    #[test]
    fn anchored_recursive_pairs_are_verified_without_publishing_ownership() {
        let m = origin();
        let mut r = lower_shared_ref_slots(&m).unwrap();
        let before = format!("{r:?}");
        verify_cfg_ref_slot_lives(&m, &r).unwrap();
        assert_eq!(format!("{r:?}"), before);
        r.functions.reverse();
        verify_cfg_ref_slot_lives(&m, &r).unwrap();
        // The cleanup stack of a callee holds no borrowed frames.
        let a = r
            .functions
            .iter()
            .find(|f| f.symbol == r.symbols["a"])
            .unwrap();
        let targets = HashMap::new();
        let stacks =
            super::super::frames::analyze_shared(a, &r.borrowed_set[&a.symbol], &targets)
                .unwrap();
        assert!(stacks.values().all(Vec::is_empty));
    }

    #[test]
    fn rejects_closed_frame_or_forged_index_even_with_correct_i64() {
        for case in 0..4 {
            let m = origin();
            let mut r = lower_shared_ref_slots(&m).unwrap();
            let b = &mut r.functions[0].blocks[0];
            if case == 0 {
                let close = b.instructions.pop().unwrap();
                let p = b
                    .instructions
                    .iter()
                    .position(|(_, i, _)| matches!(i, Instruction::CallStatic { .. }))
                    .unwrap();
                b.instructions.insert(p, close);
            } else if case == 3 {
                b.instructions.pop();
            } else {
                let Instruction::CallStatic { args, .. } = &mut b
                    .instructions
                    .iter_mut()
                    .find(|(_, i, _)| matches!(i, Instruction::CallStatic { .. }))
                    .unwrap()
                    .1
                else {
                    unreachable!()
                };
                if case == 1 {
                    args[0] = Operand::Constant(Constant::Int(123));
                } else {
                    args[1] = Operand::Constant(Constant::Int(2));
                }
            }
            let before = format!("{r:?}");
            assert!(
                verify_cfg_ref_slot_lives(&m, &r).is_err(),
                "caso {case}"
            );
            assert_eq!(format!("{r:?}"), before);
        }
    }

    #[test]
    fn callee_does_not_close_frame_or_mix_capture_indices() {
        for case in 0..3 {
            let m = origin();
            let mut r = lower_shared_ref_slots(&m).unwrap();
            let a = &mut r.functions[1];
            match case {
                0 => a.blocks[0].instructions.push((
                    ValueId(100),
                    Instruction::CallRuntime {
                        name: "dartforge_arc_quadro_fechar_v1".into(),
                        args: vec![(v(0), Type::I64)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                )),
                1 => {
                    let Instruction::CallStatic { args, .. } =
                        &mut a.blocks[0].instructions.last_mut().unwrap().1
                    else {
                        unreachable!()
                    };
                    args[1] = args[3].clone();
                }
                _ => {
                    let Instruction::CallRuntime { args, .. } = &mut a.blocks[0].instructions[0].1
                    else {
                        unreachable!()
                    };
                    args[1].0 = Operand::Constant(Constant::Int(0));
                }
            }
            assert!(
                verify_cfg_ref_slot_lives(&m, &r).is_err(),
                "caso {case}"
            );
        }
    }

    #[test]
    fn native_identity_does_not_become_dart_value_alias_or_published_address() {
        for case in 0..4 {
            let m = origin();
            let mut r = lower_shared_ref_slots(&m).unwrap();
            let dest = r.symbols["b"].clone();
            let a = &mut r.functions[1];
            match case {
                0 => a.blocks[0].instructions.push((
                    ValueId(100),
                    Instruction::Bitcast {
                        op: v(0),
                        to: Type::I64,
                    },
                    Type::I64,
                )),
                1 => a.blocks[0].terminator = Terminator::Return(Some(v(0))),
                2 => a.blocks[0].instructions.push((
                    ValueId(100),
                    Instruction::CallRuntime {
                        name: "opaco".into(),
                        args: vec![(v(0), Type::I64)],
                        ret_ty: Type::Void,
                    },
                    Type::Void,
                )),
                _ => a.blocks[0].instructions.push((
                    ValueId(100),
                    Instruction::Const(Constant::Function(dest)),
                    Type::Ptr,
                )),
            }
            assert!(
                verify_cfg_ref_slot_lives(&m, &r).is_err(),
                "caso {case}"
            );
        }
    }

    #[test]
    fn cycles_need_reachable_local_origin_in_current_body() {
        let m = origin();
        let mut r = lower_shared_ref_slots(&m).unwrap();
        r.functions[0].blocks[0]
            .instructions
            .retain(|(_, i, _)| !matches!(i, Instruction::CallStatic { .. }));
        assert!(
            verify_cfg_ref_slot_lives(&m, &r)
                .unwrap_err()
                .contains("sem origem local alcançável")
        );
    }

    #[test]
    fn roles_are_not_inferred_by_width_or_forged_metadata() {
        for case in 0..3 {
            let m = origin();
            let mut r = lower_shared_ref_slots(&m).unwrap();
            match case {
                0 => {
                    r.borrowed_set
                        .get_mut(&r.symbols["a"])
                        .unwrap()
                        .insert(ValueId(0), ValueId(1));
                }
                1 => r.locals.get_mut("raiz").unwrap().1 = 1,
                _ => {
                    r.borrowed_set
                        .get_mut(&r.symbols["a"])
                        .unwrap()
                        .remove(&ValueId(0));
                }
            }
            assert!(
                verify_cfg_ref_slot_lives(&m, &r).is_err(),
                "caso {case}"
            );
        }
        let mut m = origin();
        m.arc_memory = false;
        let mut r = lower_shared_ref_slots(&m).unwrap();
        verify_cfg_ref_slot_lives(&m, &r).unwrap();
        r.locals.insert("raiz".into(), (ValueId(0), 1));
        assert!(verify_cfg_ref_slot_lives(&m, &r).is_err());
    }
}
