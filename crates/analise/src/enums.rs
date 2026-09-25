//! `enum_without_constants` conforme o `ErrorVerifier` do analyzer 3.6.2:
//! a declaração principal usa os valores do enum aumentado inteiro.

use crate::Unidade;
use dartforge_diagnostics::codigos::compile_time_error as c;
use dartforge_diagnostics::Diagnostic;
use dartforge_frontend::ast::DeclKind;
use dartforge_intern::SymbolId;
use std::collections::HashSet;

/// Retorna o índice da unidade junto de cada diagnóstico.
pub fn sem_constantes(unidades: &[Unidade<'_>]) -> Vec<(usize, Diagnostic)> {
    let mut com_constantes = HashSet::<SymbolId>::new();
    for unidade in unidades {
        for &id in &unidade.unit.declarations {
            if let DeclKind::Enum(e) = &unidade.ast.decl(id).kind {
                if !e.constants.is_empty() {
                    com_constantes.insert(e.name.sym);
                }
            }
        }
    }

    let mut out = Vec::new();
    for (i, unidade) in unidades.iter().enumerate() {
        for &id in &unidade.unit.declarations {
            let decl = unidade.ast.decl(id);
            if let DeclKind::Enum(e) = &decl.kind {
                if !decl.augment && !com_constantes.contains(&e.name.sym) {
                    out.push((
                        i,
                        Diagnostic::com_codigo(
                            c::ENUM_WITHOUT_CONSTANTS,
                            e.name.span,
                            std::iter::empty::<&str>(),
                        ),
                    ));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;
    use dartforge_frontend::features::{Feature, LanguageVersion, LibraryFeatures};
    use dartforge_frontend::parser::{parse, parse_com};
    use dartforge_intern::Interner;

    #[test]
    fn enum_vazio_igual_ao_oraculo_36() {
        let fonte = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/enum_without_constants/EnumWithoutConstants__noConstants.dart"));
        let mut nomes = Interner::new();
        let parsed = parse(fonte, &mut nomes);
        let achados = sem_constantes(&[Unidade {
            ast: &parsed.ast,
            unit: &parsed.unit,
            fonte,
        }]);
        assert_eq!(achados.len(), 1, "{achados:?}");
        let d = &achados[0].1;
        assert_eq!(d.code, Some(c::ENUM_WITHOUT_CONSTANTS));
        assert_eq!((d.span.start, d.span.end), (5, 6));
        assert_eq!(d.message, "The enum must have at least one constant.");
        assert_eq!(d.correcao().as_deref(), Some("Try declaring a constant."));
    }

    #[test]
    fn constante_em_augmentacao_completa_enum_base() {
        let fontes = ["enum E {}", "augment enum E { v }", "augment enum F {}"];
        let mut nomes = Interner::new();
        let features = LibraryFeatures::new(LanguageVersion::PISO, &[Feature::Augmentations]);
        let parsed: Vec<_> = fontes
            .iter()
            .map(|fonte| parse_com(fonte, &mut nomes, features))
            .collect();
        let unidades: Vec<_> = fontes
            .iter()
            .zip(&parsed)
            .map(|(fonte, parsed)| Unidade {
                ast: &parsed.ast,
                unit: &parsed.unit,
                fonte,
            })
            .collect();
        assert!(sem_constantes(&unidades).is_empty());

        let fonte = "enum E {} augment enum E {}";
        let parsed = parse_com(fonte, &mut nomes, features);
        let achados = sem_constantes(&[Unidade {
            ast: &parsed.ast,
            unit: &parsed.unit,
            fonte,
        }]);
        assert_eq!(achados.len(), 1, "{achados:?}");
        assert_eq!(achados[0].1.span.start, 5);
    }
}
