//! Prepares Owned boxing and audited scalar unboxing before ARC production.
use crate::hir::*;

pub(super) fn prepare(f: &mut Function) -> Result<(), String> {
    for (id, inst, ty) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
        if let Instruction::Unbox { op, to } = inst {
            if *ty != *to {
                return Err(format!(
                    "ARC unboxing in {} v{} requires result {to:?}",
                    f.symbol, id.0
                ));
            }
            let name = match *to {
                Type::I64 => "dartforge_unbox_int",
                Type::F64 => "dartforge_unbox_double",
                _ => continue,
            };
            *inst = Instruction::CallRuntime {
                name: name.into(),
                args: vec![(op.clone(), Type::Ref)],
                ret_ty: *to,
            };
            continue;
        }
        let Instruction::Box { op, from } = inst else {
            continue;
        };
        if *ty != Type::Ref {
            return Err(format!(
                "ARC boxing in {} v{} requires a Ref result",
                f.symbol, id.0
            ));
        }
        let name = match *from {
            Type::I64 => "dartforge_arc_box_int_owned_v1",
            Type::F64 => "dartforge_arc_box_double_owned_v1",
            Type::I1 | Type::I8 => continue, // The two bool boxes are static.
            Type::V4F32 | Type::V4I32 | Type::V2F64 => continue,
            _ => {
                return Err(format!(
                    "ARC boxing in {} v{} does not cover {from:?}",
                    f.symbol, id.0
                ));
            }
        };
        *inst = Instruction::CallRuntime {
            name: name.into(),
            args: vec![(op.clone(), *from)],
            ret_ty: Type::Ref,
        };
    }
    Ok(())
}

#[cfg(test)]
mod simd_tests {
    use crate::optimize::arc::*;
    use std::collections::HashMap;

    #[test]
    fn vector_boxes_produce_owned_and_arc_emission_uses_native_words() {
        for from in [Type::V4F32, Type::V4I32, Type::V2F64] {
            let mut m = Module::new();
            m.arc_memory = true;
            m.dart_ref_returns.insert("caixa".into());
            m.functions.push(Function {
                symbol: "caixa".into(),
                name: "caixa".into(),
                debug_info: None,
                params: vec![(ValueId(0), "vetor".into(), from)],
                return_ty: Type::Ref,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::Box {
                            op: Operand::Val(ValueId(0)),
                            from,
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                }],
            });
            let mut ps = HashMap::from([("caixa".into(), DartFunctionPlan::default())]);
            assert_eq!(prepare_arc_dart_module(&mut m, &mut ps).unwrap(), (0, 0));
            assert_eq!(ps["caixa"].classes[&ValueId(1)], Ownership::Owned);
            assert_eq!(ps["caixa"].classes[&ValueId(0)], Ownership::Trivial);
            let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
            assert!(ir.contains("call i64 @dartforge_arc_box_simd_owned_v1(i64"));
            assert!(ir.contains("extractelement <2 x i64> %bx1, i32 0"));
            let before = format!("{m:?}");
            let plans = ps.clone();
            assert_eq!(prepare_arc_dart_module(&mut m, &mut ps).unwrap(), (0, 0));
            assert_eq!(format!("{m:?}"), before);
            assert_eq!(ps, plans);
            m.arc_memory = false;
            let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
            assert!(ir.contains("call i64 @df.simd_box("));
            assert!(!ir.contains("call i64 @dartforge_arc_box_simd_owned_v1"));
        }
    }

    #[test]
    fn box_origin_cannot_be_ref_or_other_width_scalar() {
        for from in [Type::V4F32, Type::V4I32, Type::V2F64] {
            let f = Function {
                symbol: "invalida".into(),
                name: "invalida".into(),
                debug_info: None,
                params: vec![(ValueId(0), "handle".into(), Type::Ref)],
                return_ty: Type::Ref,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(1),
                        Instruction::Box {
                            op: Operand::Val(ValueId(0)),
                            from,
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))),
                }],
            };
            assert!(
                produce_arc_contracts(&f, &mut HashMap::new(), &mut TokenPlan::default()).is_err()
            );
        }
    }
}
