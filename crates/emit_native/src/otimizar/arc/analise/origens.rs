//! Origem nominal atribuída antes dos passes que renumeram/clonam SSA.
//! A identidade vale para a versão do corpo; não certifica pins/recarga,
//! contexto especializado ou singleton. Operações opacas continuam sem sítio.

use super::modelo::SitioArc;
use crate::hir::*;

pub(crate) fn alocacao(i: &Instruction) -> bool {
    matches!(
        i,
        Instruction::AllocObject { .. }
            | Instruction::AllocList { .. }
            | Instruction::AllocRecord { .. }
            | Instruction::AllocCell { .. }
            | Instruction::AllocEnv { .. }
            | Instruction::AllocClosure { .. }
            | Instruction::AllocClosureTipada { .. }
            | Instruction::Box { .. }
            | Instruction::JuntarTextos { .. }
    ) || matches!(i, Instruction::CallRuntime { name, .. }
            if matches!(name.as_str(), "dartforge_object_new" | "dartforge_object_new_t"
                | "dartforge_arc_objeto_owned_v1" | "dartforge_arc_objeto_owned_t_v1"))
}

/// Registra a origem dos corpos recém-produzidos, somente no lowering ARC.
/// O ordinal identifica a alocação original, não seu futuro ValueId SSA.
pub(crate) fn registrar(m: &mut Module, inicio: usize, memoria_arc: bool) {
    if !memoria_arc {
        return;
    }
    for f in &m.functions[inicio..] {
        let sitios = m.sitios_arc.entry(f.symbol.clone()).or_default();
        let mut proximo = sitios.values().map(|s| s.origem).max().map_or(0, |v| v + 1);
        for (v, i, _) in f.blocks.iter().flat_map(|b| &b.instructions) {
            if alocacao(i) && !sitios.contains_key(v) {
                sitios.insert(
                    *v,
                    SitioArc {
                        funcao: f.symbol.clone(),
                        origem: proximo,
                        especializacao: String::new(),
                    },
                );
                proximo += 1;
            }
        }
    }
}

/// Remove fatos de instruções eliminadas, sem fabricar origens para novas operações.
pub(crate) fn podar(m: &mut Module) {
    let vivos: std::collections::HashMap<_, _> = m
        .functions
        .iter()
        .map(|f| {
            let ids: std::collections::HashSet<_> = f
                .blocks
                .iter()
                .flat_map(|b| &b.instructions)
                .filter(|(_, i, _)| alocacao(i))
                .map(|(v, _, _)| *v)
                .collect();
            (f.symbol.clone(), ids)
        })
        .collect();
    m.sitios_arc.retain(|s, sitios| {
        let Some(ids) = vivos.get(s) else {
            return false;
        };
        sitios.retain(|v, _| ids.contains(v));
        !sitios.is_empty()
    });
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn inliner_remapeia_ssa_mas_conserva_sitio_e_poda_nao_fabrica_fatos() {
        let mut m = Module::new();
        m.memoria_arc = true;
        let fabrica = Function {
            symbol: "criar".into(),
            name: "criar".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(57),
                    Instruction::AllocObject {
                        class_id: 1,
                        fields: vec![],
                    },
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(57)))),
            }],
        };
        let mut caller = Function {
            symbol: "chamar".into(),
            name: "chamar".into(),
            depuracao: None,
            params: vec![],
            return_ty: Type::Ref,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![
                    (
                        ValueId(5),
                        Instruction::CallStatic {
                            symbol: "criar".into(),
                            args: vec![],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                    (
                        ValueId(6),
                        Instruction::CallStatic {
                            symbol: "criar".into(),
                            args: vec![],
                            ret_ty: Type::Ref,
                        },
                        Type::Ref,
                    ),
                ],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(6)))),
            }],
        };
        m.functions.push(fabrica.clone());
        registrar(&mut m, 0, true);
        let sitio = m.sitios_arc["criar"][&ValueId(57)].clone();
        assert_eq!(sitio.origem, 0); // Não é o ValueId 57 do corpo original.
        let mut locais = std::collections::HashMap::new();
        let copias = std::collections::HashMap::from([("criar".into(), (fabrica, false))]);
        assert!(crate::otimizar::inline::inlining_com_origens(
            &mut caller,
            &copias,
            &m.sitios_arc,
            &mut locais
        ));
        assert_eq!(locais.len(), 2);
        assert!(locais.values().all(|s| s == &sitio));
        assert!(!locais.contains_key(&ValueId(57)));
        m.sitios_arc.insert(caller.symbol.clone(), locais);
        m.functions.push(caller);
        let antes = m.sitios_arc.clone();
        podar(&mut m);
        assert_eq!(m.sitios_arc, antes);
        crate::otimizar::otimizar(&mut m);
        assert!(m.sitios_arc["chamar"].values().all(|s| s == &sitio));
        let otimizado = m.sitios_arc.clone();
        crate::otimizar::otimizar(&mut m);
        assert_eq!(m.sitios_arc, otimizado);
        m.functions[1]
            .blocks
            .iter_mut()
            .for_each(|b| b.instructions.retain(|(_, i, _)| !alocacao(i)));
        podar(&mut m);
        assert!(!m.sitios_arc.contains_key("chamar"));
        let mut tracing = Module::new();
        tracing.functions.push(m.functions[0].clone());
        registrar(&mut tracing, 0, false);
        assert!(tracing.sitios_arc.is_empty());
    }
}
