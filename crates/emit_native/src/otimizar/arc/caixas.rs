//! Traduz boxing escalar explícito em fábricas Owned antes da produção ARC.
use crate::hir::*;

pub(super) fn preparar(f: &mut Function) -> Result<(), String> {
    for (id, inst, ty) in f.blocks.iter_mut().flat_map(|b| &mut b.instructions) {
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
