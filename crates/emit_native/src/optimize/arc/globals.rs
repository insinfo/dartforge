//! Makes owners explicit in Ref globals after checking the declaration and every write.
//! The address stays private to the storage; the content is not a token of the code.

use super::*;

pub(super) fn prepare(m: &mut Module) -> Result<(), String> {
    if !m.arc_memory {
        return Ok(());
    }
    let mut declarations = HashMap::new();
    for (_, ty, s) in &m.globals {
        if declarations.insert(s.as_str(), *ty).is_some() {
            return Err(format!("ARC globals: repeated declaration {s}"));
        }
    }
    let refs: HashSet<_> = declarations
        .iter()
        .filter_map(|(s, ty)| (*ty == Type::Ref).then_some(*s))
        .collect();
    for f in &m.functions {
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
        let reference = |o: &Operand| match o {
            Operand::Val(v) => types.get(v) == Some(&Type::Ref),
            Operand::Constant(Constant::Null) => true,
            _ => false,
        };
        let mut affected = false;
        let mut published_address = false;
        let mut constant = |o: &Operand| {
            if let Operand::Constant(Constant::Function(s)) = o {
                published_address |= refs.contains(s.as_str());
            }
        };
        for b in &f.blocks {
            for (v, inst, ty) in &b.instructions {
                let ref_global = match inst {
                    Instruction::LoadGlobal {
                        symbol,
                        ty: Type::Ref,
                    }
                    | Instruction::StoreGlobal {
                        symbol,
                        ty: Type::Ref,
                        ..
                    }
                    | Instruction::ArcLoadStrong {
                        slot: StrongSlot::Global { symbol },
                    }
                    | Instruction::ArcStoreStrong {
                        slot: StrongSlot::Global { symbol },
                        ..
                    } => Some(symbol),
                    _ => None,
                };
                if ref_global.is_some_and(|s| !refs.contains(s.as_str())) {
                    return Err(format!(
                        "ARC globals in {} v{}: Ref without a declaration",
                        f.symbol, v.0
                    ));
                }
                if let Instruction::Const(Constant::Function(s)) = inst {
                    constant(&Operand::Constant(Constant::Function(s.clone())));
                }
                super::super::operands::operands(inst, &mut constant);
                let valid = match inst {
                    Instruction::LoadGlobal { symbol, ty: read_done }
                        if refs.contains(symbol.as_str()) =>
                    {
                        affected = true;
                        *read_done == Type::Ref && *ty == Type::Ref
                    }
                    Instruction::StoreGlobal {
                        symbol,
                        ty: stored,
                        root,
                        val,
                    } if refs.contains(symbol.as_str()) => {
                        affected = true;
                        *stored == Type::Ref
                            && *ty == Type::Void
                            && root.is_some()
                            && reference(val)
                    }
                    Instruction::ArcLoadStrong {
                        slot: StrongSlot::Global { symbol },
                    } if refs.contains(symbol.as_str()) => {
                        affected = true;
                        *ty == Type::Ref
                    }
                    Instruction::ArcStoreStrong {
                        slot: StrongSlot::Global { symbol },
                        value,
                        ..
                    } if refs.contains(symbol.as_str()) => {
                        affected = true;
                        *ty == Type::Void && reference(value)
                    }
                    _ => true,
                };
                if !valid {
                    return Err(format!(
                        "ARC globals in {} v{}: incompatible Ref access",
                        f.symbol, v.0
                    ));
                }
            }
            super::super::operands::terminator_operands(&b.terminator, &mut constant);
        }
        if published_address {
            return Err(format!(
                "ARC globals in {}: published Ref address",
                f.symbol
            ));
        }
        if affected {
            super::ssa::verify(f)?;
        }
    }
    let mut changed = false;
    for f in &mut m.functions {
        for (_, inst, _) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
            *inst = match inst {
                Instruction::LoadGlobal { symbol, .. } if refs.contains(symbol.as_str()) => {
                    Instruction::ArcLoadStrong {
                        slot: StrongSlot::Global {
                            symbol: symbol.clone(),
                        },
                    }
                }
                Instruction::StoreGlobal { symbol, val, .. }
                    if refs.contains(symbol.as_str()) =>
                {
                    Instruction::ArcStoreStrong {
                        slot: StrongSlot::Global {
                            symbol: symbol.clone(),
                        },
                        value: val.clone(),
                        mode: StrongStoreMode::Copy,
                    }
                }
                _ => continue,
            };
            changed = true;
        }
    }
    if changed {
        m.arc_aot_ports = None;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin() -> Module {
        let mut m = Module::new();
        m.arc_memory = true;
        m.globals.push((0, Type::Ref, "g".into()));
        m.functions.push(Function {
            symbol: "f".into(),
            name: "f".into(),
            params: vec![(ValueId(0), "objeto".into(), Type::Ref)],
            return_ty: Type::Void,
            debug_info: None,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(1),
                        Instruction::StoreGlobal {
                            symbol: "g".into(),
                            val: Operand::Val(ValueId(0)),
                            ty: Type::Ref,
                            root: Some(0),
                        },
                        Type::Void,
                    ),
                    (
                        ValueId(2),
                        Instruction::LoadGlobal {
                            symbol: "g".into(),
                            ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(3),
                        Instruction::StoreGlobal {
                            symbol: "g".into(),
                            val: Operand::Constant(Constant::Null),
                            ty: Type::Ref,
                            root: Some(0),
                        },
                        Type::Void,
                    ),
                    (
                        ValueId(4),
                        Instruction::GetField {
                            object: Operand::Val(ValueId(2)),
                            index: 0,
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(None),
            }],
        });
        m
    }

    #[test]
    fn module_produces_read_owner_before_clearing_global_and_reading_field() {
        let mut m = origin();
        let mut ps = HashMap::from([("f".into(), DartFunctionPlan::default())]);
        prepare_arc_dart_module(&mut m, &mut ps).unwrap();
        assert_eq!(ps["f"].classes[&ValueId(2)], Ownership::Owned);
        assert!(m.functions[0].blocks.iter().flat_map(|b| &b.instructions).any(|(_, i, _)|
            matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_arc_ler_campo_ref_v1")));
        verify_dart_function_contract(
            &m.functions[0],
            &ps["f"].classes,
            &ps["f"].tokens,
            &ps["f"].tables,
            &ps["f"].scopes,
        )
        .unwrap();
        let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
        let retention = ir.find("call void @dartforge_arc_retain(i64 %v2)").unwrap();
        let cleanup = ir[retention..].find("store i64 0, ptr %ga3").unwrap();
        assert!(cleanup > 0);
    }

    #[test]
    fn invalid_writes_and_exposed_addresses_reject_without_publication() {
        for case in 0..6 {
            let mut m = origin();
            match case {
                0 => {
                    if let Instruction::StoreGlobal { root, .. } =
                        &mut m.functions[0].blocks[0].instructions[0].1
                    {
                        *root = None;
                    }
                }
                1 => m.functions[0].params[0].2 = Type::I64,
                2 => m.globals.push((1, Type::Ref, "g".into())),
                3 => m.functions[0].blocks[0].instructions.push((
                    ValueId(5),
                    Instruction::Const(Constant::Function("g".into())),
                    Type::I64,
                )),
                4 => m.globals[0].1 = Type::I64,
                _ => {
                    if let Instruction::StoreGlobal { ty, .. } =
                        &mut m.functions[0].blocks[0].instructions[0].1
                    {
                        *ty = Type::I64;
                    }
                }
            }
            let before = format!("{m:?}");
            assert!(prepare(&mut m).is_err(), "caso {case}");
            assert_eq!(format!("{m:?}"), before, "caso {case}");
        }
    }

    #[test]
    fn tracing_preserves_operations_and_scalar_globals_do_not_become_ref_slots() {
        let mut m = origin();
        m.arc_memory = false;
        let before = format!("{m:?}");
        prepare(&mut m).unwrap();
        assert_eq!(format!("{m:?}"), before);
        let mut m = Module::new();
        m.arc_memory = true;
        m.globals.push((0, Type::I64, "contador".into()));
        let before = format!("{m:?}");
        prepare(&mut m).unwrap();
        assert_eq!(format!("{m:?}"), before);
    }

    #[test]
    fn areas_of_distinct_modules_restore_strong_slot_names() {
        let a = origin();
        let mut b = origin();
        b.functions[0].symbol = "segunda".into();
        let mut ms = [a, b];
        let mut ps = [
            HashMap::from([("f".into(), DartFunctionPlan::default())]),
            HashMap::from([("segunda".into(), DartFunctionPlan::default())]),
        ];
        prepare_arc_dart_modules(&mut ms, &mut ps).unwrap();
        for (m, ps) in ms.iter().zip(&ps) {
            for f in &m.functions {
                for (_, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
                    if let Instruction::ArcLoadStrong {
                        slot: StrongSlot::Global { symbol },
                    }
                    | Instruction::ArcStoreStrong {
                        slot: StrongSlot::Global { symbol },
                        ..
                    } = i
                    {
                        assert_eq!(symbol, "g");
                    }
                }
                let p = &ps[&f.symbol];
                verify_dart_function_contract(f, &p.classes, &p.tokens, &p.tables, &p.scopes)
                    .unwrap();
            }
            crate::llvm::LlvmEmitter::new(m).emit_all();
        }
    }
}

/// Classifies as Trivial the I64 reads of Dart globals of scalar type
/// (`Module::dart_scalar_globals`): the Dart type guarantees that the content is
/// an unboxed `int`. Their writes already require a Trivial value
/// (`pure_set::check_origins`). Rejects a Ref declaration, a read of another
/// width and a conflicting class; publishes nothing on error.
pub(super) fn produce_scalars(
    m: &Module,
    plans: &mut HashMap<String, DartFunctionPlan>,
) -> Result<(), String> {
    if !m.arc_memory || m.dart_scalar_globals.is_empty() {
        return Ok(());
    }
    let declarations: HashMap<&str, Type> = m.globals.iter().map(|(_, t, s)| (s.as_str(), *t)).collect();
    for s in &m.dart_scalar_globals {
        match declarations.get(s.as_str()) {
            Some(Type::I64 | Type::F64 | Type::I1 | Type::I8) | None => {}
            Some(t) => return Err(format!("global escalar {s} declarada {t:?}")),
        }
    }
    let mut new_set = plans.clone();
    for f in &m.functions {
        for (v, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
            let Instruction::LoadGlobal { symbol, ty: read_done } = inst else { continue };
            if !m.dart_scalar_globals.contains(symbol) || *read_done != Type::I64 {
                continue;
            }
            if *ty != Type::I64 || declarations.get(symbol.as_str()).is_some_and(|d| *d != Type::I64) {
                return Err(format!("scalar global {symbol} read in {} v{} with another width", f.symbol, v.0));
            }
            let Some(plan) = new_set.get_mut(&f.symbol) else { continue };
            if plan.classes.get(v).is_some_and(|c| *c != Ownership::Trivial)
                || plan.tokens.instructions.get(v).is_some_and(|e| *e != TokenEffect::default())
            {
                return Err(format!("scalar global {symbol} with a conflicting contract in {} v{}", f.symbol, v.0));
            }
            plan.classes.insert(*v, Ownership::Trivial);
            plan.tokens.instructions.insert(*v, TokenEffect::default());
        }
    }
    *plans = new_set;
    Ok(())
}
