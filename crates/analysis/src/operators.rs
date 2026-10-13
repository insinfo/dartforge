//! The arity of `operator` methods, as in the analyzer's
//! `ErrorVerifier._checkForWrongNumberOfParametersForOperator`, optional
//! parameters (`_checkForOptionalParameterInOperator`) and the return type of
//! `[]=` (`_checkForNonVoidReturnTypeForOperator`).

use crate::Unit;
use dartforge_diagnostics::codes::compile_time_error as c;
use dartforge_diagnostics::Diagnostic;
use dartforge_frontend::ast::{Ast, DeclKind, FunctionKind, MemberKind, ParameterKind, TypeId, TypeKind, TypedefKind};
use dartforge_intern::{Interner, SymbolId};

/// Checks the operator members of classes, mixins, enums, extensions and extension types.
pub fn arity(unit: Unit<'_>, names: &Interner) -> Vec<Diagnostic> {
    let ast = unit.ast;
    let void_aliases = void_aliases(unit);
    let mut out = Vec::new();
    for &id in &unit.unit.declarations {
        let members = match &ast.decl(id).kind {
            DeclKind::Class(d) => &d.members,
            DeclKind::Mixin(d) => &d.members,
            DeclKind::Enum(d) => &d.members,
            DeclKind::Extension(d) => &d.members,
            DeclKind::ExtensionType(d) => &d.members,
            _ => continue,
        };
        for &id in members.iter() {
            let MemberKind::Method(fid) = &ast.member(id).kind else {
                continue;
            };
            let function = ast.function(*fid);
            if function.kind != FunctionKind::Operator {
                continue;
            }
            let (Some(name), Some(params)) = (function.name, &function.parameters) else {
                continue;
            };
            let text = names.resolve(name.sym);
            let expected = match text {
                "[]=" => Some(2),
                "<" | ">" | "<=" | ">=" | "==" | "+" | "/" | "~/" | "*" | "%" | "|" | "^" | "&"
                | "<<" | ">>" | ">>>" | "[]" => Some(1),
                "~" => Some(0),
                _ => None,
            };
            let mut wrong_arity = false;
            if let Some(expected) = expected {
                if params.len() != expected {
                    wrong_arity = true;
                    out.push(Diagnostic::with_code(
                        c::WRONG_NUMBER_OF_PARAMETERS_FOR_OPERATOR,
                        name.span,
                        [
                            text.to_string(),
                            expected.to_string(),
                            params.len().to_string(),
                        ],
                    ));
                }
            } else if text == "-" && params.len() > 1 {
                wrong_arity = true;
                out.push(Diagnostic::with_code(
                    c::WRONG_NUMBER_OF_PARAMETERS_FOR_OPERATOR_MINUS,
                    name.span,
                    [params.len().to_string()],
                ));
            }
            // `_checkForOptionalParameterInOperator`, only without an arity
            // error (one error per operator); a `required` named one is not
            // optional.
            if !wrong_arity {
                let optional = params.iter().filter(|p| {
                    p.kind == ParameterKind::Optional || (p.kind == ParameterKind::Named && !p.required)
                });
                for p in optional {
                    out.push(Diagnostic::with_code(c::OPTIONAL_PARAMETER_IN_OPERATOR, p.span, [] as [&str; 0]));
                }
            }
            // `_checkForNonVoidReturnTypeForOperator`: `[]=` with a written
            // type that is not `void` (nor an alias of `void` declared in the
            // unit).
            if text == "[]="
                && let Some(t) = function.return_type
                && !is_void(ast, &void_aliases, t)
            {
                out.push(Diagnostic::with_code(c::NON_VOID_RETURN_FOR_OPERATOR, ast.ty(t).span, [] as [&str; 0]));
            }
        }
    }
    out
}

/// The unit's `typedef`s that are aliases of `void`.
fn void_aliases(unit: Unit<'_>) -> Vec<SymbolId> {
    let ast = unit.ast;
    unit
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

/// `type is VoidType` by the written form: `void` or a local alias of it.
fn is_void(ast: &Ast, aliases: &[SymbolId], t: TypeId) -> bool {
    match &ast.ty(t).kind {
        TypeKind::Void => true,
        TypeKind::Named { name, args } => args.is_empty() && name.len() == 1 && aliases.contains(&name[0].sym),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dartforge_frontend::parser::parse;

    #[test]
    fn real_operators_from_the_corpus() {
        for (source, code, operator, message) in [
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
            let mut names = Interner::new();
            let parsed = parse(source, &mut names);
            let found = arity(Unit { ast: &parsed.ast, unit: &parsed.unit, source }, &names);
            let start = source.find(&format!("operator {operator}")).unwrap() + "operator ".len();
            assert!(found.iter().any(|d|
                d.code == Some(code) && d.span.start == start && d.span.end == start + operator.len()
                    && d.message == message
            ), "{source}: {found:?}");
        }
    }

    #[test]
    fn minus_accepts_zero_or_one_parameter() {
        let source =
            "class A { operator -() => this; operator -(a) => this; operator -(a, b) => this; }";
        let mut names = Interner::new();
        let parsed = parse(source, &mut names);
        let found = arity(
            Unit {
                ast: &parsed.ast,
                unit: &parsed.unit,
                source,
            },
            &names,
        );
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(
            found[0].code,
            Some(c::WRONG_NUMBER_OF_PARAMETERS_FOR_OPERATOR_MINUS)
        );
        assert_eq!(
            found[0].message,
            "Operator '-' should declare 0 or 1 parameter, but 2 found."
        );
    }

    /// The language's `operator/invalid_operators_test.dart` (3.6.2 oracle):
    /// the whole optional parameter, and none when the arity is already wrong.
    #[test]
    fn optional_and_index_return() {
        let source = "typedef V = void;\nclass A {\n  operator ==([dynamic a]) => true;\n  operator <({a = 1}) => true;\n  operator >({required a}) => true;\n  operator +([a, b]) => 0;\n  int operator []=(a, b) => 0;\n  V operator []=(a, b) {}\n}";
        let mut names = Interner::new();
        let parsed = parse(source, &mut names);
        let found = arity(Unit { ast: &parsed.ast, unit: &parsed.unit, source }, &names);
        let v: Vec<(&str, &str)> = found
            .iter()
            .map(|d| (d.code.unwrap().info().name, &source[d.span.start..d.span.end]))
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
