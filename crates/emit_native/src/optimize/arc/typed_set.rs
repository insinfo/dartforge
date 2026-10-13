//! Provenance of the typed PC and context of the same closure, without word coercion.

use super::*;

const TARGET: &str = "dartforge_arc_alvo_tipada_v1";
const CONTEXTO: &str = "dartforge_arc_contexto_closure_v1";

fn definition<'a>(f: &'a Function, op: &Operand) -> Option<&'a Instruction> {
    let Operand::Val(v) = op else { return None };
    f.blocks.iter().flat_map(|b| &b.instructions).find(|(id, _, _)| id == v).map(|(_, i, _)| i)
}

fn context(f: &Function, op: &Operand) -> Option<Operand> {
    let mut op = op.clone();
    let mut seen_set = HashSet::new();
    loop {
        let Operand::Val(v) = &op else { return None };
        if !seen_set.insert(*v) { return None; }
        match definition(f, &op)? {
            Instruction::ArcCopy { value } | Instruction::ArcMove { value } => op = value.clone(),
            Instruction::CallRuntime { name, args, ret_ty: Type::Ref } if name == CONTEXTO => {
                let [(closure, Type::Ref)] = args.as_slice() else { return None };
                return Some(closure.clone());
            }
            _ => return None,
        }
    }
}

fn signature(args: &[(Operand, Type)], ret: Type) -> Option<i64> {
    if !matches!(ret, Type::Ref | Type::I64 | Type::F64 | Type::I1) { return None; }
    super::objects::typed_abi(&args.iter().map(|(_, t)| *t).collect::<Vec<_>>(), ret)
}

pub(super) fn check(f: &Function, inst: &Instruction, ty: Type) -> Result<bool, String> {
    let Instruction::TypedCall { alvo, args, ret } = inst else { return Ok(false) };
    let error = || format!("ARC typed call in {}: PC, context, signature or return without verified provenance", f.symbol);
    let abi = signature(args, *ret).ok_or_else(error)?;
    if ty != *ret { return Err(error()); }
    let Some(Instruction::CallRuntime { name, args: origin, ret_ty: Type::I64 }) = definition(f, alvo)
        else { return Err(error()) };
    let [(closure, Type::Ref), (Operand::Constant(Constant::Int(code)), Type::I64)] = origin.as_slice()
        else { return Err(error()) };
    if name != TARGET || *code != abi || context(f, &args[0].0).as_ref() != Some(closure) {
        return Err(error());
    }
    let types: HashMap<_, _> = f.params.iter().map(|(v, _, t)| (*v, *t))
        .chain(f.blocks.iter().flat_map(|b| &b.instructions).map(|(v, _, t)| (*v, *t))).collect();
    for (op, t) in args {
        let compatible = match op {
            Operand::Val(v) => types.get(v) == Some(t),
            Operand::Constant(Constant::Null) => *t == Type::Ref,
            Operand::Constant(Constant::Int(_)) => *t == Type::I64,
            Operand::Constant(Constant::Double(_)) => *t == Type::F64,
            Operand::Constant(Constant::Bool(_)) => *t == Type::I1,
            _ => false,
        };
        if !compatible { return Err(error()); }
    }
    Ok(true)
}

// The runtime registration checks class, physical map, ABI and PC before the call.
// The descriptor read stays scalar and gets checked bounds/map.
// Operates only on the private copy of the ARC preparation, before fields/contracts.
pub(super) fn prepare(f: &mut Function) -> Result<(), String> {
    let mut targets = HashMap::new();
    let mut closures = Vec::new();
    for (_, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
        let Instruction::TypedCall { alvo, args, ret } = inst else { continue };
        if check(f, inst, *ty).unwrap_or(false) {
            let closure = context(f, &args[0].0).unwrap();
            if !closures.contains(&closure) { closures.push(closure); }
            continue;
        }
        let error = || format!("ARC typed call in {}: legacy read without covered PC/context/ABI", f.symbol);
        let abi = signature(args, *ret).ok_or_else(error)?;
        let Operand::Val(v) = alvo else { return Err(error()) };
        let Some(Instruction::CallRuntime { name, args: origin, ret_ty: Type::I64 }) = definition(f, alvo)
            else { return Err(error()) };
        let [(closure, Type::Ref), (Operand::Constant(Constant::Int(2)), Type::I64)] = origin.as_slice()
            else { return Err(error()) };
        if name != "dartforge_object_get" || context(f, &args[0].0).as_ref() != Some(closure) {
            return Err(error());
        }
        if targets.insert(*v, (closure.clone(), abi)).is_some_and(|old| old != (closure.clone(), abi)) {
            return Err(error());
        }
        if !closures.contains(closure) { closures.push(closure.clone()); }
    }
    for (v, inst, _) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
        if let Some((closure, abi)) = targets.get(v) {
            *inst = Instruction::CallRuntime { name: TARGET.into(), args: vec![
                (closure.clone(), Type::Ref), (Operand::Constant(Constant::Int(*abi)), Type::I64),
            ], ret_ty: Type::I64 };
        } else if let Instruction::CallRuntime { name, args, ret_ty: Type::I64 } = inst
            && name == "dartforge_object_get"
            && let [(closure, Type::Ref), (Operand::Constant(Constant::Int(3)), Type::I64)] = args.as_slice()
            && closures.contains(closure)
        {
            *name = "dartforge_arc_ler_campo_escalar_v1".into();
        }
    }
    for (_, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
        check(f, inst, *ty)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_returns_require_abi_and_trivial_body_plan() {
        for (ty, value, abi) in [(Type::I64, Constant::Int(i64::MIN), 32),
            (Type::F64, Constant::Double(-0.0), 33), (Type::I1, Constant::Bool(true), 34)] {
            let (mut m, mut p) = module();
            m.dart_ref_returns.remove("corpo"); m.dart_ref_returns.remove("invocar");
            m.functions[1].return_ty = ty;
            m.functions[1].blocks[0].terminator = Terminator::Return(Some(Operand::Constant(value.clone())));
            m.functions[0].return_ty = ty;
            m.functions[0].blocks[1].terminator = Terminator::Return(Some(Operand::Constant(value)));
            m.functions[0].blocks[0].instructions[3].2 = ty;
            if let Instruction::TypedCall { ret, .. } = &mut m.functions[0].blocks[0].instructions[3].1 { *ret = ty; }
            if let Instruction::AllocClosureTyped { abi: code, .. } = &mut m.functions[3].blocks[0].instructions[0].1 { *code = abi; }
            prepare_arc_dart_module(&mut m, &mut p).unwrap();
            assert_eq!(p["invocar"].classes[&ValueId(4)], Ownership::Trivial);
            assert_eq!(p["corpo"].tokens.ret, TokenReturn::Trivial);
            certify_arc_aot_ports(&mut m, &p).unwrap();
            let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
            assert!(ir.contains(&format!("call {} %ct4(", ty.llvm_ir())));
            let ready = p.clone();
            assert_eq!(prepare_arc_dart_module(&mut m, &mut p).unwrap(), (0, 0)); assert_eq!(p, ready);
            p.get_mut("corpo").unwrap().tokens.ret = TokenReturn::Owned;
            assert!(certify_arc_aot_ports(&mut m, &p).is_err());
        }
    }

    fn module() -> (Module, HashMap<String, DartFunctionPlan>) {
        let mut m = Module::new(); m.arc_memory = true;
        m.functions = vec![
            Function { symbol: "invocar".into(), name: "invocar".into(), debug_info: None,
                params: vec![(ValueId(0), "closure".into(), Type::Ref), (ValueId(1), "x".into(), Type::Ref),
                    (ValueId(8), "outra".into(), Type::Ref)], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(10), Instruction::CallRuntime { name: "dartforge_object_get".into(), args: vec![
                        (Operand::Val(ValueId(0)), Type::Ref), (Operand::Constant(Constant::Int(3)), Type::I64),
                    ], ret_ty: Type::I64 }, Type::I64),
                    (ValueId(2), Instruction::CallRuntime { name: "dartforge_object_get".into(), args: vec![
                        (Operand::Val(ValueId(0)), Type::Ref), (Operand::Constant(Constant::Int(2)), Type::I64),
                    ], ret_ty: Type::I64 }, Type::I64),
                    (ValueId(3), Instruction::CallRuntime { name: CONTEXTO.into(), args: vec![
                        (Operand::Val(ValueId(0)), Type::Ref),
                    ], ret_ty: Type::Ref }, Type::Ref),
                    (ValueId(4), Instruction::TypedCall { alvo: Operand::Val(ValueId(2)), args: vec![
                        (Operand::Val(ValueId(3)), Type::Ref), (Operand::Val(ValueId(1)), Type::Ref),
                    ], ret: Type::Ref }, Type::Ref),
                    (ValueId(5), Instruction::CallRuntime { name: "dartforge_exception_pending".into(), args: vec![], ret_ty: Type::I8 }, Type::I8),
                    (ValueId(6), Instruction::ICmp(ICmpOp::Ne, Operand::Val(ValueId(5)), Operand::Constant(Constant::Int(0))), Type::I1),
                ], terminator: Terminator::CondBranch { cond: Operand::Val(ValueId(6)), then_block: BlockId(1), else_block: BlockId(2) } },
                    BasicBlock { id: BlockId(1), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) },
                    BasicBlock { id: BlockId(2), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(4)))) },
                ] },
            Function { symbol: "corpo".into(), name: "corpo".into(), debug_info: None,
                params: vec![(ValueId(0), "env".into(), Type::Ref), (ValueId(1), "x".into(), Type::Ref)], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }] },
            Function { symbol: "entrada".into(), name: "entrada".into(), debug_info: None,
                params: vec![(ValueId(0), "closure".into(), Type::Ref), (ValueId(1), "args".into(), Type::Ptr), (ValueId(2), "desc".into(), Type::Ptr)],
                return_ty: Type::Ref, blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![], terminator: Terminator::Return(Some(Operand::Constant(Constant::Null))) }] },
            Function { symbol: "capturar".into(), name: "capturar".into(), debug_info: None, params: vec![], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(0), Instruction::AllocClosureTyped { code_symbol: "entrada".into(), env: Operand::Constant(Constant::Null), typed: "corpo".into(), abi: 31, direct: false }, Type::Ref),
                ], terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))) }] },
        ];
        m.dart_ref_returns.extend(["invocar".into(), "corpo".into(), "capturar".into()]);
        m.arc_uniform_returns.insert("entrada".into());
        m.dart_vector_parameters.insert("entrada".into(), HashMap::from([
            (ValueId(1), DartEntryVector::Arguments), (ValueId(2), DartEntryVector::Descriptor),
        ]));
        let p = m.functions.iter().map(|f| (f.symbol.clone(), DartFunctionPlan::default())).collect();
        (m, p)
    }

    #[test]
    fn legacy_becomes_checked_pc_with_retained_context_and_ownerless_pending_on_error() {
        for case in 0..17 {
            let (mut m, mut p) = module();
            match case {
                0 => {},
                1 | 6 => m.functions[0].blocks[0].instructions[1].1 = Instruction::CallRuntime {
                    name: TARGET.into(), args: vec![(Operand::Val(ValueId(0)), Type::Ref),
                        (Operand::Constant(Constant::Int(if case == 1 { 31 } else { 41 })), Type::I64)], ret_ty: Type::I64 },
                2 => if let Instruction::TypedCall { alvo, .. } = &mut m.functions[0].blocks[0].instructions[3].1 { *alvo = Operand::Constant(Constant::Int(123)); },
                3 => if let Instruction::CallRuntime { args, .. } = &mut m.functions[0].blocks[0].instructions[2].1 { args[0].0 = Operand::Val(ValueId(8)); },
                4 => if let Instruction::TypedCall { ret, .. } = &mut m.functions[0].blocks[0].instructions[3].1 { *ret = Type::I64; },
                5 => { m.functions[0].blocks[0].instructions.truncate(4); m.functions[0].blocks[0].terminator = Terminator::Branch(BlockId(2)); },
                7 => m.functions[0].blocks[1].terminator = Terminator::Return(Some(Operand::Val(ValueId(4)))),
                8 => m.arc_memory = false,
                9..=16 => {
                    m.functions[0].blocks[0].instructions.drain(0..3);
                    m.functions[0].blocks[0].instructions[0].1 = Instruction::CallClosure {
                        closure: Operand::Val(ValueId(0)), args: vec![Operand::Val(ValueId(1))],
                        names: vec![], tuple_types: Operand::Constant(Constant::Int(0)), ret_ty: Type::Ref,
                    };
                    if let Instruction::CallClosure { args, names, tuple_types, .. } = &mut m.functions[0].blocks[0].instructions[0].1 {
                        match case {
                            10 => args[0] = Operand::Constant(Constant::Int(123)),
                            11 => *tuple_types = Operand::Constant(Constant::Int(1)),
                            12 => { args.push(Operand::Val(ValueId(8))); *names = vec!["a".into(), "a".into()]; },
                            _ => {},
                        }
                    }
                    if case == 13 {
                        let mut t = FunctionTables::default(); t.invocations.insert(ValueId(4), BlockId(1));
                        m.tables.push(t);
                    }
                    if case >= 14 {
                        let inst = if case == 15 { Instruction::Const(Constant::Int(1)) } else {
                            Instruction::CallRuntime { name: "dartforge_rti_avaliar".into(), args: vec![
                                (Operand::Constant(Constant::Int(0)), Type::I64),
                                (Operand::Constant(Constant::Null), if case == 16 { Type::I64 } else { Type::Ref }),
                                (Operand::Constant(Constant::Int(0)), Type::I64), (Operand::Constant(Constant::Int(0)), Type::I64),
                            ], ret_ty: Type::I64 }
                        };
                        m.functions[0].blocks[0].instructions.insert(0, (ValueId(9), inst, Type::I64));
                        if let Instruction::CallClosure { tuple_types, .. } = &mut m.functions[0].blocks[0].instructions[1].1 {
                            *tuple_types = Operand::Val(ValueId(9));
                        }
                    }
                },
                _ => unreachable!(),
            }
            let before = format!("{m:?}/{p:?}");
            let result = prepare_arc_dart_module(&mut m, &mut p);
            if matches!(case, 0 | 1 | 9 | 14) {
                result.unwrap();
                assert_eq!(p["invocar"].classes[&ValueId(4)], Ownership::Owned);
                assert_eq!(p["invocar"].tokens.pendings[&ValueId(4)], BlockId(1));
                if case < 9 { assert_eq!(p["invocar"].classes[&ValueId(3)], Ownership::Owned); }
                assert!(!m.functions[0].blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i,
                    Instruction::CallRuntime { name, .. } if name == "dartforge_object_get")));
                let prepared = format!("{m:?}"); let prepared_plans = p.clone();
                assert_eq!(prepare_arc_dart_module(&mut m, &mut p).unwrap(), (0, 0));
                assert_eq!(prepared, format!("{m:?}")); assert_eq!(prepared_plans, p);
                certify_arc_aot_ports(&mut m, &p).unwrap();
                let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
                if case >= 9 {
                    assert!(ir.contains("call i64 @dartforge_arc_invocar_closure_uniforme_v1(i64 %v0"));
                    assert!(!ir.contains("call i64 %cf4("));
                } else {
                    assert!(ir.contains("@dartforge_arc_alvo_tipada_v1(i64 %v0, i64 31)"));
                    assert!(ir.contains("call i64 %ct4("));
                    let (mut without_registration, _) = module();
                    without_registration.functions = m.functions.clone();
                    without_registration.functions[3].blocks[0].instructions.clear();
                    without_registration.functions[3].blocks[0].terminator = Terminator::Return(Some(Operand::Constant(Constant::Null)));
                    without_registration.arc_aot_ports = None;
                    let mut new_set = HashMap::from_iter(without_registration.functions.iter().map(|f| (f.symbol.clone(), DartFunctionPlan::default())));
                    // The prepared bodies keep their own effects; reuse
                    // their plans, except for the factory that now returns null.
                    for s in ["invocar", "corpo", "entrada"] { new_set.insert(s.into(), p[s].clone()); }
                    prepare_arc_dart_module(&mut without_registration, &mut new_set).unwrap();
                    assert!(certify_arc_aot_ports(&mut without_registration, &new_set).unwrap_err().contains("without a body registered"));
                }
            } else {
                assert_eq!(result.is_ok(), case == 8, "caso {case}: {result:?}");
                assert_eq!(before, format!("{m:?}/{p:?}"));
            }
        }
    }
}
