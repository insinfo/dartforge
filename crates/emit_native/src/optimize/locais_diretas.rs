//! Retira funções locais diretas que perderam todos os caminhos de entrada.
//! O produtor declara os candidatos; nomes e parâmetros Ptr não provam fechamento.

use crate::hir::*;
use std::collections::{HashMap, HashSet};

pub(super) fn podar(m: &mut Module) {
    if !m.memoria_arc
        || m.funcoes_locais_diretas_arc.is_empty()
        || (!m.tabelas.is_empty() && m.tabelas.len() != m.functions.len())
    {
        return;
    }
    let mut vivos: HashSet<String> = m
        .functions
        .iter()
        .filter(|f| !m.funcoes_locais_diretas_arc.contains(&f.symbol))
        .map(|f| f.symbol.clone())
        .collect();
    vivos.extend(m.entry_symbol.iter().cloned());
    vivos.extend(m.chamar_dart.iter().cloned());
    vivos.extend(m.iniciar_rti.iter().cloned());
    vivos.extend(m.registro.iter().cloned());
    vivos.extend(m.registros_do_sdk.iter().cloned());
    vivos.extend(m.raizes_da_fonte.iter().cloned());
    vivos.extend(m.ajudantes.iter().map(|(_, s)| s.clone()));
    vivos.extend(m.ffi_callbacks.iter().map(|c| c.corpo.clone()));
    vivos.extend(m.ffi_trampolins.iter().map(|(_, s)| s.clone()));
    vivos.extend(m.funcoes_de_tabela.values().cloned());
    vivos.extend(m.retornos_uniformes_arc.iter().cloned());
    vivos.extend(m.funcoes_compartilhadas_arc.iter().cloned());
    vivos.extend(m.tearoffs_do_rastro.iter().map(|(s, _)| s.clone()));
    for c in &m.classes {
        vivos.extend(c.vtable.iter().map(|(_, s)| s.clone()));
        vivos.extend(c.to_string_symbol.iter().cloned());
    }
    for (_, tabela, entradas) in &m.tabelas_de_metodos {
        vivos.insert(tabela.clone());
        vivos.extend(entradas.iter().map(|(_, s)| s.clone()));
    }
    let por_nome: HashMap<_, _> = m.functions.iter().map(|f| (f.symbol.as_str(), f)).collect();
    let mut fila: Vec<_> = vivos.iter().cloned().collect();
    while let Some(s) = fila.pop() {
        let Some(f) = por_nome.get(s.as_str()) else {
            continue;
        };
        let mut citar = |s: &String| {
            if vivos.insert(s.clone()) {
                fila.push(s.clone());
            }
        };
        for b in &f.blocks {
            for (_, i, _) in &b.instructions {
                match i {
                    Instruction::CallStatic { symbol, .. } => citar(symbol),
                    Instruction::Const(Constant::Funcao(symbol)) => citar(symbol),
                    Instruction::TearOff { code_symbol }
                    | Instruction::AllocClosure { code_symbol, .. } => citar(code_symbol),
                    Instruction::AllocClosureTipada {
                        code_symbol,
                        tipado,
                        ..
                    } => {
                        citar(code_symbol);
                        citar(tipado);
                    }
                    Instruction::TabelaDeFuncoes(ss) => {
                        for s in ss {
                            citar(s);
                        }
                    }
                    _ => {}
                }
                super::operandos::operandos(i, &mut |o| {
                    if let Operand::Constant(Constant::Funcao(s)) = o {
                        citar(s);
                    }
                });
            }
            super::operandos::operandos_do_terminador(&b.terminator, &mut |o| {
                if let Operand::Constant(Constant::Funcao(s)) = o {
                    citar(s);
                }
            });
        }
    }
    let mortos: HashSet<_> = m
        .functions
        .iter()
        .filter(|f| m.funcoes_locais_diretas_arc.contains(&f.symbol) && !vivos.contains(&f.symbol))
        .map(|f| f.symbol.clone())
        .collect();
    if mortos.is_empty() {
        return;
    }
    if !m.tabelas.is_empty() {
        m.tabelas = std::mem::take(&mut m.tabelas)
            .into_iter()
            .zip(&m.functions)
            .filter_map(|(t, f)| (!mortos.contains(&f.symbol)).then_some(t))
            .collect();
    }
    m.functions.retain(|f| !mortos.contains(&f.symbol));
    m.funcoes_locais_diretas_arc.retain(|s| !mortos.contains(s));
    m.parametros_escalares_dart
        .retain(|s, _| !mortos.contains(s));
    m.parametros_rti_dart.retain(|s, _| !mortos.contains(s));
    m.parametros_vetores_dart.retain(|s, _| !mortos.contains(s));
    m.retornos_ref_dart.retain(|s| !mortos.contains(s));
    m.sitios_arc.retain(|s, _| !mortos.contains(s));
    m.erros_da_fonte.retain(|(s, _)| !mortos.contains(s));
    m.recusados.retain(|(s, _)| !mortos.contains(s));
    m.portas_arc_aot = None;
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn fonte_registra_captura_direta_e_pipeline_arc_retira_corpo_sem_chamada() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            use crate::context::Context;
            use dartforge_intern::Interner;
            use dartforge_types::table::{CoreTypes, TypeTable};
            let dir = tempfile::tempdir().unwrap();
            let entrada = dir.path().join("main.dart");
            std::fs::write(&entrada,
                "void main() { var x = 1; void local() { x = x + 1; } if (false) { local(); } }\n").unwrap();
            let sdk = crate::sdk_modulo::carregar_sdk_nativo(crate::sdk_testes()).unwrap();
            let mut interner = Interner::new();
            let (programa, ds) = dartforge_elements::load::load_lenient(&entrada, &sdk, None, &mut interner);
            assert!(ds.is_empty(), "{ds:?}");
            let mut tipos = TypeTable::new();
            let core = CoreTypes::init(&mut tipos, &programa, &interner);
            let (mut outline, ds) = dartforge_types::resolve_outline(&programa, &interner, &mut tipos, &core);
            let (corpos, ds_corpos) = dartforge_types::infer_program_bodies(
                &programa, &interner, &mut tipos, &core, &mut outline);
            for d in ds.iter().chain(&ds_corpos) {
                assert!(!dartforge_types::codes::e_erro_de_compilacao(&d.message), "{d:?}");
            }
            let mut ctx = Context::new(&programa, &interner, &tipos, &core, &outline, &corpos);
            for arc in [false, true] {
                ctx.memoria_arc = arc;
                let mut m = crate::lower::lower_program(&ctx);
                assert!(m.erros.is_empty(), "{:?}", m.erros);
                let locais: Vec<_> = m.functions.iter().filter(|f| f.name == "local")
                    .map(|f| f.symbol.clone()).collect();
                assert_eq!(locais.len(), 1);
                assert_eq!(m.funcoes_locais_diretas_arc.contains(&locais[0]), arc);
                super::super::otimizar(&mut m);
                assert_eq!(m.functions.iter().any(|f| f.symbol == locais[0]), !arc);
                assert!(m.funcoes_locais_diretas_arc.is_empty());
            }
        }).unwrap().join().unwrap();
    }

    fn f(s: &str, alvo: Option<&str>) -> Function {
        Function {
            symbol: s.into(),
            name: s.into(),
            params: vec![],
            return_ty: Type::Void,
            depuracao: None,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: alvo
                    .into_iter()
                    .map(|a| {
                        (
                            ValueId(0),
                            Instruction::CallStatic {
                                symbol: a.into(),
                                args: vec![],
                                ret_ty: Type::Void,
                            },
                            Type::Void,
                        )
                    })
                    .collect(),
                terminator: Terminator::Return(None),
            }],
        }
    }

    #[test]
    fn ciclo_sem_entrada_some_mas_cadeia_chamada_e_corpos_publicos_ficam() {
        let mut m = Module::new();
        m.memoria_arc = true;
        m.functions = vec![
            f("publica", Some("a")),
            f("a", Some("b")),
            f("b", None),
            f("morta", Some("recursiva")),
            f("recursiva", Some("morta")),
            f("externa", None),
        ];
        m.funcoes_locais_diretas_arc = ["a", "b", "morta", "recursiva"].map(String::from).into();
        m.retornos_ref_dart.insert("morta".into());
        m.sitios_arc.insert("morta".into(), Default::default());
        m.tabelas = vec![Default::default(); m.functions.len()];
        m.tabelas[2].confere_pilha = true;
        podar(&mut m);
        assert_eq!(
            m.functions
                .iter()
                .map(|f| f.symbol.as_str())
                .collect::<Vec<_>>(),
            ["publica", "a", "b", "externa"]
        );
        assert_eq!(m.tabelas.len(), 4);
        assert!(m.tabelas[2].confere_pilha);
        assert!(m.retornos_ref_dart.is_empty());
        assert!(m.sitios_arc.is_empty());
        let antes = format!("{m:?}");
        podar(&mut m);
        assert_eq!(format!("{m:?}"), antes);
    }

    #[test]
    fn enderecos_em_operandos_e_terminadores_e_entradas_publicadas_sao_raizes() {
        for caso in 0..7 {
            let mut m = Module::new();
            m.memoria_arc = true;
            m.functions = vec![
                f("publica", None),
                f("local", Some("filha")),
                f("filha", None),
            ];
            m.funcoes_locais_diretas_arc = ["local", "filha"].map(String::from).into();
            match caso {
                0 => m.functions[0].blocks[0].instructions.push((
                    ValueId(0),
                    Instruction::Const(Constant::Funcao("local".into())),
                    Type::I64,
                )),
                1 => {
                    m.functions[0].blocks[0].terminator = Terminator::Return(Some(
                        Operand::Constant(Constant::Funcao("local".into())),
                    ))
                }
                2 => m.ajudantes.push(("nativo".into(), "local".into())),
                3 => {
                    m.funcoes_de_tabela.insert(1, "local".into());
                }
                4 => m.raizes_da_fonte.push("local".into()),
                5 => m.tabelas_de_metodos.push((
                    1,
                    "tabela".into(),
                    vec![("call".into(), "local".into())],
                )),
                _ => m.entry_symbol = Some("local".into()),
            }
            podar(&mut m);
            assert_eq!(m.functions.len(), 3, "caso {caso}");
        }
    }

    #[test]
    fn tracing_e_inventario_excepcional_incompleto_nao_sao_alterados() {
        for arc in [false, true] {
            let mut m = Module::new();
            m.memoria_arc = arc;
            m.functions = vec![f("publica", None), f("morta", None)];
            m.funcoes_locais_diretas_arc.insert("morta".into());
            if arc {
                m.tabelas.push(Default::default());
            }
            let antes = format!("{m:?}");
            podar(&mut m);
            assert_eq!(format!("{m:?}"), antes);
        }
    }
}
