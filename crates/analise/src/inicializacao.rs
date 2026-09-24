//! `final_not_initialized` conforme `_checkForNotInitializedFieldDeclaration`
//! e a verificação de variáveis de topo do `ErrorVerifier` do analyzer.

use crate::Unidade;
use dartforge_diagnostics::codigos::compile_time_error as c;
use dartforge_diagnostics::Diagnostic;
use dartforge_frontend::ast::{DeclKind, Initializer, MemberKind, VariableList};
use dartforge_intern::{Interner, SymbolId};
use std::collections::{HashMap, HashSet};

fn final_sem_inicializador(v: &VariableList, nomes: &Interner, out: &mut Vec<(usize, Diagnostic)>, unidade: usize, enum_index: bool) {
    if !v.final_ || v.const_ || v.late || v.external || v.abstract_ {
        return;
    }
    for var in v.variables.iter().filter(|var| var.initializer.is_none()) {
        if enum_index && nomes.resolve(var.name.sym) == "index" { continue; }
        out.push((unidade, Diagnostic::com_codigo(
            c::FINAL_NOT_INITIALIZED,
            var.name.span,
            [nomes.resolve(var.name.sym)],
        )));
    }
}

/// Uma classe com construtor gerador explícito delega a verificação dos seus
/// campos de instância ao `ConstructorFieldsVerifier`; factories não a fazem.
pub fn finais_nao_inicializados(unidades: &[Unidade<'_>], nomes: &Interner) -> Vec<(usize, Diagnostic)> {
    // `unidades` contém uma biblioteca e suas partes. As declarações
    // aumentadas da mesma classe podem estar em unidades diferentes.
    let mut com_gerador = HashSet::<(bool, SymbolId)>::new();
    let mut campos = HashMap::<(bool, SymbolId), Vec<(SymbolId, bool)>>::new();
    for unidade in unidades {
        for &id in &unidade.unit.declarations {
            let (chave, membros) = match &unidade.ast.decl(id).kind {
                DeclKind::Class(x) => ((false, x.name.sym), &x.members),
                DeclKind::Enum(x) => ((true, x.name.sym), &x.members),
                _ => continue,
            };
            if membros.iter().any(|&m| {
                matches!(&unidade.ast.member(m).kind, MemberKind::Constructor(k) if !k.factory)
            }) {
                com_gerador.insert(chave);
            }
            for &m in membros {
                if let MemberKind::Field(v) = &unidade.ast.member(m).kind {
                    if v.static_ { continue; }
                    for var in &v.variables {
                        // O membro `Enum.index` implícito não participa da
                        // checagem de inicialização, mesmo que o usuário
                        // declare outro `index` (erro próprio de enum).
                        if chave.0 && nomes.resolve(var.name.sym) == "index" { continue; }
                        campos.entry(chave).or_default().push((
                            var.name.sym,
                            v.final_ && !v.const_ && !v.late && !v.external && !v.abstract_ && var.initializer.is_none(),
                        ));
                    }
                }
            }
        }
    }

    // O verificador oficial ignora nomes de campos duplicados; o diagnóstico
    // de duplicata já é emitido por outro verificador.
    let mut pendentes = HashMap::<(bool, SymbolId), Vec<SymbolId>>::new();
    for (chave, campos) in campos {
        let mut contagem = HashMap::<SymbolId, usize>::new();
        for (nome, _) in &campos { *contagem.entry(*nome).or_default() += 1; }
        pendentes.insert(chave, campos.into_iter().filter_map(|(nome, falta)| {
            (falta && contagem[&nome] == 1).then_some(nome)
        }).collect());
    }

    let mut out = Vec::new();
    for (i, unidade) in unidades.iter().enumerate() {
        for &id in &unidade.unit.declarations {
            let membros = match &unidade.ast.decl(id).kind {
                DeclKind::Variables(v) => {
                    final_sem_inicializador(v, nomes, &mut out, i, false);
                    continue;
                }
                DeclKind::Class(x) => (&x.members, com_gerador.contains(&(false, x.name.sym))),
                DeclKind::Enum(x) => (&x.members, com_gerador.contains(&(true, x.name.sym))),
                DeclKind::Mixin(x) => (&x.members, false),
                DeclKind::Extension(x) => (&x.members, false),
                DeclKind::ExtensionType(x) => (&x.members, false),
                _ => continue,
            };
            for &m in membros.0 {
                match &unidade.ast.member(m).kind {
                    MemberKind::Field(v) => {
                        if v.static_ || !membros.1 {
                            let enum_index = !v.static_ && matches!(&unidade.ast.decl(id).kind, DeclKind::Enum(_));
                            final_sem_inicializador(v, nomes, &mut out, i, enum_index);
                        }
                    }
                    MemberKind::Constructor(k) if !k.factory && !k.external && k.redirect.is_none() => {
                        let (chave, primario) = match &unidade.ast.decl(id).kind {
                            DeclKind::Class(x) => ((false, x.name.sym), x.primary_constructor),
                            DeclKind::Enum(x) => ((true, x.name.sym), x.primary_constructor),
                            _ => continue,
                        };
                        // O corpus `analyzer` usa SDK 3.6.2: nele a sintaxe
                        // de construtor primário é recuperada como erro de
                        // parser, sem diagnóstico semântico de inicialização.
                        if primario == Some(m) { continue; }
                        if k.initializers.iter().any(|x| matches!(x, Initializer::Redirect { .. })) { continue; }
                        let mut faltantes = pendentes.get(&chave).cloned().unwrap_or_default();
                        faltantes.retain(|nome| {
                            !k.parameters.iter().any(|p| p.this_ && p.name.is_some_and(|n| n.sym == *nome))
                                && !k.initializers.iter().any(|x| matches!(x, Initializer::Field { name, .. } if name.sym == *nome))
                        });
                        if faltantes.is_empty() { continue; }
                        let mut nomes_faltantes: Vec<_> = faltantes.iter().map(|s| nomes.resolve(*s).to_string()).collect();
                        nomes_faltantes.sort();
                        let codigo = match nomes_faltantes.len() {
                            1 => c::FINAL_NOT_INITIALIZED_CONSTRUCTOR_1,
                            2 => c::FINAL_NOT_INITIALIZED_CONSTRUCTOR_2,
                            _ => c::FINAL_NOT_INITIALIZED_CONSTRUCTOR_3_PLUS,
                        };
                        let argumentos = match nomes_faltantes.len() {
                            1 | 2 => nomes_faltantes,
                            n => vec![nomes_faltantes[0].clone(), nomes_faltantes[1].clone(), (n - 2).to_string()],
                        };
                        // O oráculo 3.6.2 marca só o nome da classe, inclusive
                        // em `A.named()`; o analyzer main estende até `.named`.
                        out.push((i, Diagnostic::com_codigo(codigo, k.class_name.span, argumentos)));
                    }
                    _ => {}
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;
    use dartforge_frontend::parser::parse;

    fn testar(fonte: &str) -> Vec<(String, String)> {
        let mut nomes = Interner::new();
        let parsed = parse(fonte, &mut nomes);
        finais_nao_inicializados(&[Unidade { ast: &parsed.ast, unit: &parsed.unit, fonte }], &nomes)
            .into_iter()
            .map(|(_, d)| {
                let nome = fonte[d.span.start as usize..d.span.end as usize].to_string();
                (nome, d.message)
            })
            .collect()
    }

    #[test]
    fn topo_estatico_instancia_e_excecoes_do_oraculo() {
        let fonte = "final int topo; class A { static final int estatico; final int instancia; factory A() => throw 0; } class B { final int delegado; B(); } abstract class C { abstract final int abstrato; external final int externo; late final int tardio; }";
        let mut encontrados = testar(fonte);
        encontrados.sort();
        assert_eq!(encontrados, vec![
            ("B".into(), "All final variables must be initialized, but 'delegado' isn't.".into()),
            ("estatico".into(), "The final variable 'estatico' must be initialized.".into()),
            ("instancia".into(), "The final variable 'instancia' must be initialized.".into()),
            ("topo".into(), "The final variable 'topo' must be initialized.".into()),
        ]);
    }

    #[test]
    fn lista_parcial_e_nullable_tem_mesma_regra() {
        let encontrados = testar("final Object? v; class A { static final int a = 0, b, c = 0; }");
        assert_eq!(encontrados, vec![
            ("v".into(), "The final variable 'v' must be initialized.".into()),
            ("b".into(), "The final variable 'b' must be initialized.".into()),
        ]);
    }

    #[test]
    fn tres_fontes_do_corpus_oficial() {
        for (fonte, nome) in [
            (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__topLevelVariabl_dd6c7267.dart")), "v"),
            (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__class_staticFie_99366e40.dart")), "v2"),
            (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__class_instanceF_5d207b49.dart")), "v"),
        ] {
            assert_eq!(testar(fonte), vec![(
                nome.into(),
                format!("The final variable '{nome}' must be initialized."),
            )]);
        }
    }

    #[test]
    fn construtores_inicializam_campos_por_parametro_ou_lista() {
        let mut encontrados = testar("class A { final int x; final int y; A(this.x); A.named() : y = 0; } class B { final int v; B() : this.named(); B.named() : v = 0; }");
        encontrados.sort();
        assert_eq!(encontrados, vec![
            ("A".into(), "All final variables must be initialized, but 'x' isn't.".into()),
            ("A".into(), "All final variables must be initialized, but 'y' isn't.".into()),
        ]);
    }

    #[test]
    fn construtor_com_tres_campos_usa_variante_3_plus() {
        let encontrados = testar("class A { final int z; final int x; final int y; A(); }");
        assert_eq!(encontrados, vec![(
            "A".into(),
            "All final variables must be initialized, but 'x', 'y', and 1 others aren't.".into(),
        )]);
    }

    #[test]
    fn construtor_primario_nao_gera_codigo_semantico_no_corpus_36() {
        let fonte = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__class_instanceF_01f4790b.dart"));
        assert!(testar(fonte).is_empty());
    }

    #[test]
    fn construtores_secundarios_iguais_ao_oraculo_36() {
        for (fonte, offset, codigo, mensagem) in [
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__class_instanceF_0811e46c.dart")),
                60,
                c::FINAL_NOT_INITIALIZED_CONSTRUCTOR_3_PLUS,
                "All final variables must be initialized, but 'v1', 'v2', and 1 others aren't.",
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__class_instanceF_568a572a.dart")),
                45,
                c::FINAL_NOT_INITIALIZED_CONSTRUCTOR_1,
                "All final variables must be initialized, but 'v' isn't.",
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/variable_not_initialized/VariableNotInitialized__enum_instanceFi_5eb8f155.dart")),
                70,
                c::FINAL_NOT_INITIALIZED_CONSTRUCTOR_1,
                "All final variables must be initialized, but 'v' isn't.",
            ),
        ] {
            let mut nomes = Interner::new();
            let parsed = parse(fonte, &mut nomes);
            let achados = finais_nao_inicializados(&[Unidade { ast: &parsed.ast, unit: &parsed.unit, fonte }], &nomes);
            assert_eq!(achados.len(), 1, "{achados:?}");
            let d = &achados[0].1;
            assert_eq!(d.code, Some(codigo), "{achados:?}");
            assert_eq!(d.span.start, offset, "{achados:?}");
            assert_eq!(d.span.end, offset + 1, "{achados:?}");
            assert_eq!(d.message, mensagem, "{achados:?}");
        }
    }

    #[test]
    fn index_de_enum_usa_diagnostico_proprio() {
        let fonte = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/illegal_concrete_enum_member_declaration/IllegalConcreteEnumMemberDeclarationEnu_9000f3c7.dart"));
        assert!(testar(fonte).is_empty());
        assert!(testar("enum E { v; final int index; }").is_empty());
        assert_eq!(testar("enum E { v; static final int index; }"), vec![(
            "index".into(), "The final variable 'index' must be initialized.".into(),
        )]);
    }
}
