//! Aridade de métodos `operator`, conforme
//! `ErrorVerifier._checkForWrongNumberOfParametersForOperator` do analyzer,
//! parâmetros opcionais (`_checkForOptionalParameterInOperator`) e o tipo de
//! retorno de `[]=` (`_checkForNonVoidReturnTypeForOperator`).

use crate::Unidade;
use dartforge_diagnostics::codigos::compile_time_error as c;
use dartforge_diagnostics::Diagnostic;
use dartforge_frontend::ast::{Ast, DeclKind, FunctionKind, MemberKind, ParameterKind, TypeId, TypeKind, TypedefKind};
use dartforge_intern::{Interner, SymbolId};

/// Confere operadores membros de classes, mixins, enums, extensões e tipos de extensão.
pub fn aridade(unidade: Unidade<'_>, nomes: &Interner) -> Vec<Diagnostic> {
    let ast = unidade.ast;
    let vazios = aliases_de_void(unidade);
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
            let mut aridade_errada = false;
            if let Some(esperado) = esperado {
                if params.len() != esperado {
                    aridade_errada = true;
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
                aridade_errada = true;
                out.push(Diagnostic::com_codigo(
                    c::WRONG_NUMBER_OF_PARAMETERS_FOR_OPERATOR_MINUS,
                    nome.span,
                    [params.len().to_string()],
                ));
            }
            // `_checkForOptionalParameterInOperator`, só sem erro de aridade
            // (um erro por operador); nomeado `required` não é opcional.
            if !aridade_errada {
                let opcionais = params.iter().filter(|p| {
                    p.kind == ParameterKind::Optional || (p.kind == ParameterKind::Named && !p.required)
                });
                for p in opcionais {
                    out.push(Diagnostic::com_codigo(c::OPTIONAL_PARAMETER_IN_OPERATOR, p.span, [] as [&str; 0]));
                }
            }
            // `_checkForNonVoidReturnTypeForOperator`: `[]=` com tipo escrito
            // que não é `void` (nem alias de `void` declarado na unidade).
            if texto == "[]="
                && let Some(t) = funcao.return_type
                && !e_void(ast, &vazios, t)
            {
                out.push(Diagnostic::com_codigo(c::NON_VOID_RETURN_FOR_OPERATOR, ast.ty(t).span, [] as [&str; 0]));
            }
        }
    }
    out
}

/// Os `typedef` da unidade que são alias de `void`.
fn aliases_de_void(unidade: Unidade<'_>) -> Vec<SymbolId> {
    let ast = unidade.ast;
    unidade
        .unit
        .declarations
        .iter()
        .filter_map(|&id| match &ast.decl(id).kind {
            DeclKind::Typedef(d) => match d.kind {
                TypedefKind::Alias(t) if matches!(ast.ty(t).kind, TypeKind::Void) => Some(d.name.sym),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// `type is VoidType` pela forma escrita: `void` ou um alias local dele.
fn e_void(ast: &Ast, aliases: &[SymbolId], t: TypeId) -> bool {
    match &ast.ty(t).kind {
        TypeKind::Void => true,
        TypeKind::Named { name, args } => args.is_empty() && name.len() == 1 && aliases.contains(&name[0].sym),
        _ => false,
    }
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

    /// `operator/invalid_operators_test.dart` da linguagem (oráculo 3.6.2):
    /// o parâmetro opcional inteiro, e nenhum quando a aridade já está errada.
    #[test]
    fn opcional_e_retorno_de_indice() {
        let fonte = "typedef V = void;\nclass A {\n  operator ==([dynamic a]) => true;\n  operator <({a = 1}) => true;\n  operator >({required a}) => true;\n  operator +([a, b]) => 0;\n  int operator []=(a, b) => 0;\n  V operator []=(a, b) {}\n}";
        let mut nomes = Interner::new();
        let parsed = parse(fonte, &mut nomes);
        let achados = aridade(Unidade { ast: &parsed.ast, unit: &parsed.unit, fonte }, &nomes);
        let v: Vec<(&str, &str)> = achados
            .iter()
            .map(|d| (d.code.unwrap().info().nome, &fonte[d.span.start..d.span.end]))
            .collect();
        assert_eq!(
            v,
            vec![
                ("optional_parameter_in_operator", "dynamic a"),
                ("optional_parameter_in_operator", "a = 1"),
                ("wrong_number_of_parameters_for_operator", "+"),
                ("non_void_return_for_operator", "int"),
            ]
        );
    }
}
