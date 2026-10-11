//! Prepara boxing Owned e unboxing escalar auditado antes da produção ARC.
use crate::hir::*;

pub(super) fn preparar(f: &mut Function) -> Result<(), String> {
    for (id, inst, ty) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
        if let Instruction::Unbox { op, to } = inst {
            if *ty != *to {
                return Err(format!(
                    "unboxing ARC em {} v{} exige resultado {to:?}",
                    f.symbol, id.0
                ));
            }
            let nome = match *to {
                Type::I64 => "dartforge_unbox_int",
                Type::F64 => "dartforge_unbox_double",
                _ => continue,
            };
            *inst = Instruction::CallRuntime {
                name: nome.into(),
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
                "boxing ARC em {} v{} exige resultado Ref",
                f.symbol, id.0
            ));
        }
        let nome = match *from {
            Type::I64 => "dartforge_arc_box_int_owned_v1",
            Type::F64 => "dartforge_arc_box_double_owned_v1",
            Type::I1 | Type::I8 => continue, // As duas caixas bool são estáticas.
            Type::V4F32 | Type::V4I32 | Type::V2F64 => continue,
            _ => {
                return Err(format!(
                    "boxing ARC em {} v{} não cobre {from:?}",
                    f.symbol, id.0
                ));
            }
        };
        *inst = Instruction::CallRuntime {
            name: nome.into(),
            args: vec![(op.clone(), *from)],
            ret_ty: Type::Ref,
        };
    }
    Ok(())
}

#[cfg(test)]
mod testes_simd {
    use crate::otimizar::arc::*;
    use std::collections::HashMap;

    #[test]
    fn caixas_vetoriais_produzem_owned_e_emissao_arc_usa_palavras_nativas() {
        for from in [Type::V4F32, Type::V4I32, Type::V2F64] {
            let mut m = Module::new(); m.memoria_arc = true;
            m.retornos_ref_dart.insert("caixa".into());
            m.functions.push(Function {
                symbol: "caixa".into(), name: "caixa".into(), depuracao: None,
                params: vec![(ValueId(0), "vetor".into(), from)], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(1), Instruction::Box { op: Operand::Val(ValueId(0)), from }, Type::Ref),
                ], terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }],
            });
            let mut ps = HashMap::from([("caixa".into(), PlanoFuncaoDart::default())]);
            assert_eq!(preparar_arc_modulo_dart(&mut m, &mut ps).unwrap(), (0, 0));
            assert_eq!(ps["caixa"].classes[&ValueId(1)], Ownership::Owned);
            assert_eq!(ps["caixa"].classes[&ValueId(0)], Ownership::Trivial);
            let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
            assert!(ir.contains("call i64 @dartforge_arc_box_simd_owned_v1(i64"));
            assert!(ir.contains("extractelement <2 x i64> %bx1, i32 0"));
            let antes = format!("{m:?}"); let planos = ps.clone();
            assert_eq!(preparar_arc_modulo_dart(&mut m, &mut ps).unwrap(), (0, 0));
            assert_eq!(format!("{m:?}"), antes); assert_eq!(ps, planos);
            m.memoria_arc = false;
            let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
            assert!(ir.contains("call i64 @df.simd_caixa("));
            assert!(!ir.contains("call i64 @dartforge_arc_box_simd_owned_v1"));
        }
    }

    #[test]
    fn origem_de_caixa_nao_pode_ser_ref_ou_escalar_de_outra_largura() {
        for from in [Type::V4F32, Type::V4I32, Type::V2F64] {
            let f = Function {
                symbol: "invalida".into(), name: "invalida".into(), depuracao: None,
                params: vec![(ValueId(0), "handle".into(), Type::Ref)], return_ty: Type::Ref,
                blocks: vec![BasicBlock { id: BlockId(0), instructions: vec![
                    (ValueId(1), Instruction::Box { op: Operand::Val(ValueId(0)), from }, Type::Ref),
                ], terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }],
            };
            assert!(produzir_contratos_arc(&f, &mut HashMap::new(), &mut PlanoTokens::default()).is_err());
        }
    }
}
