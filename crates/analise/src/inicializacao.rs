//! `final_not_initialized` conforme `_checkForNotInitializedFieldDeclaration`
//! e a verificação de variáveis de topo do `ErrorVerifier` do analyzer.

use crate::Unidade;
use dartforge_diagnostics::codigos::compile_time_error as c;
use dartforge_diagnostics::Diagnostic;
use dartforge_frontend::ast::{DeclKind, MemberKind, VariableList};
use dartforge_intern::{Interner, SymbolId};
use std::collections::HashSet;

fn final_sem_inicializador(v: &VariableList, nomes: &Interner, out: &mut Vec<(usize, Diagnostic)>, unidade: usize) {
    if !v.final_ || v.const_ || v.late || v.external || v.abstract_ {
        return;
    }
    for var in v.variables.iter().filter(|var| var.initializer.is_none()) {
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
    let mut com_gerador = HashSet::<SymbolId>::new();
    for unidade in unidades {
        for &id in &unidade.unit.declarations {
            let (nome, membros) = match &unidade.ast.decl(id).kind {
                DeclKind::Class(x) => (x.name.sym, &x.members),
                DeclKind::Enum(x) => (x.name.sym, &x.members),
                _ => continue,
            };
            if membros.iter().any(|&m| {
                matches!(&unidade.ast.member(m).kind, MemberKind::Constructor(k) if !k.factory)
            }) {
                com_gerador.insert(nome);
            }
        }
    }

    let mut out = Vec::new();
    for (i, unidade) in unidades.iter().enumerate() {
        for &id in &unidade.unit.declarations {
            let membros = match &unidade.ast.decl(id).kind {
                DeclKind::Variables(v) => {
                    final_sem_inicializador(v, nomes, &mut out, i);
                    continue;
                }
                DeclKind::Class(x) => (&x.members, com_gerador.contains(&x.name.sym)),
                DeclKind::Enum(x) => (&x.members, com_gerador.contains(&x.name.sym)),
                DeclKind::Mixin(x) => (&x.members, false),
                DeclKind::Extension(x) => (&x.members, false),
                DeclKind::ExtensionType(x) => (&x.members, false),
                _ => continue,
            };
            for &m in membros.0 {
                if let MemberKind::Field(v) = &unidade.ast.member(m).kind {
                    if v.static_ || !membros.1 {
                        final_sem_inicializador(v, nomes, &mut out, i);
                    }
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
}
