//! Coerência estrutural dos fatos declarados; não prova origem dos receivers.

use crate::hir::*;
use std::collections::HashMap;

pub(super) fn verify(module: &Module) -> Result<(), String> {
    let mut classes: HashMap<u32, Vec<&ClassDef>> = HashMap::new();
    for c in &module.classes {
        classes.entry(c.id).or_default().push(c);
    }
    let mut ids: Vec<_> = module.arc_field_layouts.keys().copied().collect();
    ids.sort_unstable();
    for id in ids {
        let fields = &module.arc_field_layouts[&id];
        let cs = classes
            .get(&id)
            .ok_or_else(|| format!("layout ARC de classe ausente {id}"))?;
        if cs.len() != 1 || cs[0].field_count != fields.len() {
            return Err(format!(
                "layout ARC da classe {id} ambíguo ou com quantidade incompatível"
            ));
        }
        if fields
            .iter()
            .any(|t| !matches!(t, Type::Ref | Type::I1 | Type::I8 | Type::I64 | Type::F64))
        {
            return Err(format!(
                "layout ARC da classe {id} tem representação de campo inválida"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optimize::arc::{DartFunctionPlan, prepare_arc_dart_module};

    #[test]
    fn invalid_layout_does_not_publish_module_and_tracing_does_not_apply_facts() {
        for case in 0..5 {
            let mut m = Module::default();
            m.arc_memory = true;
            m.classes.push(ClassDef {
                id: 7,
                name: "C".into(),
                field_count: 2,
                vtable: vec![],
                to_string_symbol: None,
            });
            m.arc_field_layouts.insert(7, vec![Type::I64, Type::Ref]);
            m.functions.push(Function {
                symbol: "f".into(),
                name: "f".into(),
                debug_info: None,
                params: vec![],
                return_ty: Type::Void,
                blocks: vec![BasicBlock {
                    id: BlockId(0),
                    instructions: vec![(
                        ValueId(0),
                        Instruction::Box {
                            op: Operand::Constant(Constant::Int(i64::MAX)),
                            from: Type::I64,
                        },
                        Type::Ref,
                    )],
                    terminator: Terminator::Return(None),
                }],
            });
            let mut plans = HashMap::from([("f".into(), DartFunctionPlan::default())]);
            match case {
                1 => {
                    m.arc_field_layouts.get_mut(&7).unwrap().pop();
                }
                2 => m.classes.clear(),
                3 => m.arc_field_layouts.get_mut(&7).unwrap()[0] = Type::V4F32,
                4 => m.classes.push(m.classes[0].clone()),
                _ => {}
            }
            let before = format!("{m:?}/{plans:?}");
            if case == 0 {
                assert_eq!(
                    prepare_arc_dart_module(&mut m, &mut plans).unwrap(),
                    (0, 1)
                );
            } else {
                assert!(prepare_arc_dart_module(&mut m, &mut plans).is_err());
                assert_eq!(format!("{m:?}/{plans:?}"), before);
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
}
