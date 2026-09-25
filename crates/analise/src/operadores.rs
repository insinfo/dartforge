//! Aridade de métodos `operator`, conforme
//! `ErrorVerifier._checkForWrongNumberOfParametersForOperator` do analyzer.

use crate::Unidade;
use dartforge_diagnostics::codigos::compile_time_error as c;
use dartforge_diagnostics::Diagnostic;
use dartforge_frontend::ast::{DeclKind, FunctionKind, MemberKind};
use dartforge_intern::Interner;

/// Confere operadores membros de classes, mixins, enums, extensões e tipos de extensão.
pub fn aridade(unidade: Unidade<'_>, nomes: &Interner) -> Vec<Diagnostic> {
    let ast = unidade.ast;
    let mut out = Vec::new();
    for &id in &unidade.unit.declarations {
        let membros = match &ast.decl(id).kind {
            DeclKind::Class(d) => &d.members,
            DeclKind::Mixin(d) => &d.members,
            DeclKind::Enum(d) => &d.members,
            DeclKind::Extension(d) => &d.members,
            DeclKind::ExtensionType(d) => &d.members,
            _ => continue,
        };
        for &id in membros.iter() {
            let MemberKind::Method(fid) = &ast.member(id).kind else {
                continue;
            };
            let funcao = ast.function(*fid);
            if funcao.kind != FunctionKind::Operator {
                continue;
            }
            let (Some(nome), Some(params)) = (funcao.name, &funcao.parameters) else {
                continue;
            };
            let texto = nomes.resolve(nome.sym);
            let esperado = match texto {
                "[]=" => Some(2),
                "<" | ">" | "<=" | ">=" | "==" | "+" | "/" | "~/" | "*" | "%" | "|" | "^" | "&"
                | "<<" | ">>" | ">>>" | "[]" => Some(1),
                "~" => Some(0),
                _ => None,
            };
            if let Some(esperado) = esperado {
                if params.len() != esperado {
                    out.push(Diagnostic::com_codigo(
                        c::WRONG_NUMBER_OF_PARAMETERS_FOR_OPERATOR,
                        nome.span,
                        [
                            texto.to_string(),
                            esperado.to_string(),
                            params.len().to_string(),
                        ],
                    ));
                }
            } else if texto == "-" && params.len() > 1 {
                out.push(Diagnostic::com_codigo(
                    c::WRONG_NUMBER_OF_PARAMETERS_FOR_OPERATOR_MINUS,
                    nome.span,
                    [params.len().to_string()],
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;
    use dartforge_frontend::parser::parse;

    #[test]
    fn operadores_reais_do_corpus() {
        for (fonte, codigo, operador, mensagem) in [
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/wrong_number_of_parameters_for_operator/WrongNumberOfParametersForOperator__tri_89062d24.dart")),
                c::WRONG_NUMBER_OF_PARAMETERS_FOR_OPERATOR,
                ">>>",
                "Operator '>>>' should declare exactly 1 parameters, but 0 found.",
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/wrong_number_of_parameters_for_operator/WrongNumberOfParametersForOperator__tilde_rP_rP.dart")),
                c::WRONG_NUMBER_OF_PARAMETERS_FOR_OPERATOR,
                "~",
                "Operator '~' should declare exactly 0 parameters, but 2 found.",
            ),
            (
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/diagnosticos/analyzer/wrong_number_of_parameters_for_operator/WrongNumberOfParametersForOperator__star_rP_rP.dart")),
                c::WRONG_NUMBER_OF_PARAMETERS_FOR_OPERATOR,
                "*",
                "Operator '*' should declare exactly 1 parameters, but 2 found.",
            ),
        ] {
            let mut nomes = Interner::new();
            let parsed = parse(fonte, &mut nomes);
            let achados = aridade(Unidade { ast: &parsed.ast, unit: &parsed.unit, fonte }, &nomes);
            let inicio = fonte.find(&format!("operator {operador}")).unwrap() + "operator ".len();
            assert!(achados.iter().any(|d|
                d.code == Some(codigo) && d.span.start == inicio && d.span.end == inicio + operador.len()
                    && d.message == mensagem
            ), "{fonte}: {achados:?}");
        }
    }

    #[test]
    fn menos_aceita_zero_ou_um_parametro() {
        let fonte =
            "class A { operator -() => this; operator -(a) => this; operator -(a, b) => this; }";
        let mut nomes = Interner::new();
        let parsed = parse(fonte, &mut nomes);
        let achados = aridade(
            Unidade {
                ast: &parsed.ast,
                unit: &parsed.unit,
                fonte,
            },
            &nomes,
        );
        assert_eq!(achados.len(), 1, "{achados:?}");
        assert_eq!(
            achados[0].code,
            Some(c::WRONG_NUMBER_OF_PARAMETERS_FOR_OPERATOR_MINUS)
        );
        assert_eq!(
            achados[0].message,
            "Operator '-' should declare 0 or 1 parameter, but 2 found."
        );
    }
}
