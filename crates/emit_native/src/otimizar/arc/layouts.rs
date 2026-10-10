//! Coerência estrutural dos fatos declarados; não prova origem dos receivers.

use crate::hir::*;
use std::collections::HashMap;

pub(super) fn verificar(modulo: &Module) -> Result<(), String> {
    let mut classes: HashMap<u32, Vec<&ClassDef>> = HashMap::new();
    for c in &modulo.classes {
        classes.entry(c.id).or_default().push(c);
    }
    let mut ids: Vec<_> = modulo.layouts_campos_arc.keys().copied().collect();
    ids.sort_unstable();
    for id in ids {
        let campos = &modulo.layouts_campos_arc[&id];
        let cs = classes
            .get(&id)
            .ok_or_else(|| format!("layout ARC de classe ausente {id}"))?;
        if cs.len() != 1 || cs[0].field_count != campos.len() {
            return Err(format!(
                "layout ARC da classe {id} ambíguo ou com quantidade incompatível"
            ));
        }
        if campos
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
mod testes {
    use super::*;
    use crate::otimizar::arc::{PlanoFuncaoDart, preparar_arc_modulo_dart};

    #[test]
    fn layout_invalido_nao_publica_modulo_e_tracing_nao_aplica_fatos() {
        for caso in 0..5 {
            let mut m = Module::default();
            m.memoria_arc = true;
            m.classes.push(ClassDef {
                id: 7,
                name: "C".into(),
                field_count: 2,
                vtable: vec![],
                to_string_symbol: None,
            });
            m.layouts_campos_arc.insert(7, vec![Type::I64, Type::Ref]);
            m.functions.push(Function {
                symbol: "f".into(),
                name: "f".into(),
                depuracao: None,
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
            let mut planos = HashMap::from([("f".into(), PlanoFuncaoDart::default())]);
            match caso {
                1 => {
                    m.layouts_campos_arc.get_mut(&7).unwrap().pop();
                }
                2 => m.classes.clear(),
                3 => m.layouts_campos_arc.get_mut(&7).unwrap()[0] = Type::V4F32,
                4 => m.classes.push(m.classes[0].clone()),
                _ => {}
            }
            let antes = format!("{m:?}/{planos:?}");
            if caso == 0 {
                assert_eq!(
                    preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                    (0, 1)
                );
            } else {
                assert!(preparar_arc_modulo_dart(&mut m, &mut planos).is_err());
                assert_eq!(format!("{m:?}/{planos:?}"), antes);
            }
            m.memoria_arc = false;
            let antes = format!("{m:?}/{planos:?}");
            assert_eq!(
                preparar_arc_modulo_dart(&mut m, &mut planos).unwrap(),
                (0, 0)
            );
            assert_eq!(format!("{m:?}/{planos:?}"), antes);
        }
    }
}
