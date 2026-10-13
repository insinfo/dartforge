//! Temporary bridge: gives an analyzer code to the diagnostics that today's
//! phases emit as text only.
//!
//! The A2 contract is the `Diagnostic` with `code`, `severity` and `args`
//! ([`dartforge_diagnostics`]). The `frontend` parser and the `types`
//! inference do not use it yet: the parser writes messages in Portuguese, and
//! `types` writes `"{template}: <rest>"` with the templates of `types::codes`.
//! Until request T1 (codes with arguments, emitted by `types`) and the
//! lexer/parser line land, this bridge **recognizes** those texts and
//! converts them. It does not decide semantics: the code is what the emitter
//! itself meant (the `name` of the `DiagnosticCode`), and the arguments are
//! the quoted fragments it wrote. Where that does not match the oracle, the
//! scoreboard shows it — that is what it is for. When `types` emits `code`,
//! the bridge is no longer consulted for that diagnostic (see
//! [`encode_types`]).
//!
//! The Portuguese strings matched below are the texts the other crates emit
//! today; they must change together with them.

use dartforge_diagnostics::{Codigo, Diagnostic, codigos};

/// The single-quoted fragments of `s`, in order.
fn quoted(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(i) = rest.find('\'') {
        let after = &rest[i + 1..];
        match after.find('\'') {
            Some(j) => {
                out.push(after[..j].to_string());
                rest = &after[j + 1..];
            }
            None => break,
        }
    }
    out
}

/// Names in `types::codes` that are not analyzer 3.6 code names, and the
/// official code that describes the same rule.
const ALIASES: &[(&str, &str)] = &[
    // Reading a non-nullable local before definite assignment.
    ("definitely_unassigned_variable", "not_assigned_potentially_non_nullable_local_variable"),
];

/// Reported names shared by several `uniqueName`s: which variant the bridge
/// uses when the emitter does not say the context.
const VARIANTS: &[(&str, &str)] = &[
    ("return_of_invalid_type", "CompileTimeErrorCode.RETURN_OF_INVALID_TYPE_FROM_FUNCTION"),
    ("class_instantiation_access_to_instance_member", "CompileTimeErrorCode.CLASS_INSTANTIATION_ACCESS_TO_INSTANCE_MEMBER"),
    ("class_instantiation_access_to_static_member", "CompileTimeErrorCode.CLASS_INSTANTIATION_ACCESS_TO_STATIC_MEMBER"),
    ("class_instantiation_access_to_unknown_member", "CompileTimeErrorCode.CLASS_INSTANTIATION_ACCESS_TO_UNKNOWN_MEMBER"),
];

fn by_name(name: &str) -> Option<Codigo> {
    let name = ALIASES.iter().find(|(a, _)| *a == name).map(|(_, b)| *b).unwrap_or(name);
    if let Some((_, u)) = VARIANTS.iter().find(|(n, _)| *n == name) {
        return Codigo::por_unico(u);
    }
    Codigo::por_nome(name)
}

/// Codes whose last argument is additional information that, outside of
/// records, is empty (`ARGUMENT_TYPE_NOT_ASSIGNABLE` `{2}`).
const LAST_OPTIONAL: &[&str] = &["argument_type_not_assignable"];

/// The templates of `types::codes` that inference uses today.
fn types_templates() -> Vec<dartforge_types::DiagnosticCode> {
    use dartforge_types::codes as c;
    vec![
        c::ARGUMENT_TYPE_NOT_ASSIGNABLE,
        c::RETURN_OF_INVALID_TYPE,
        c::INVALID_ASSIGNMENT,
        c::ASSIGNMENT_TO_FINAL_LOCAL,
        c::ASSIGNMENT_TO_FINAL,
        c::ASSIGNMENT_TO_FINAL_NO_SETTER,
        c::ASSIGNMENT_TO_METHOD,
        c::UNDEFINED_EXTENSION_SETTER,
        c::UNDEFINED_EXTENSION_GETTER,
        c::UNDEFINED_EXTENSION_METHOD,
        c::STATIC_ACCESS_TO_INSTANCE_MEMBER,
        c::CLASS_INSTANTIATION_ACCESS_TO_INSTANCE_MEMBER,
        c::CLASS_INSTANTIATION_ACCESS_TO_STATIC_MEMBER,
        c::CLASS_INSTANTIATION_ACCESS_TO_UNKNOWN_MEMBER,
        c::NEW_WITH_UNDEFINED_CONSTRUCTOR,
        c::NEW_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT,
        c::INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR,
        c::INVOCATION_OF_EXTENSION_WITHOUT_CALL,
        c::UNDEFINED_EXTENSION_OPERATOR,
        c::EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER,
        c::ASSIGNMENT_TO_CONST,
        c::ASSIGNMENT_TO_TYPE,
        c::ASSIGNMENT_TO_FUNCTION,
        c::NOT_INITIALIZED_NON_NULLABLE_VARIABLE,
        c::DEFINITELY_UNASSIGNED_VARIABLE,
        c::UNDEFINED_IDENTIFIER,
        c::REFERENCED_BEFORE_DECLARATION,
        c::UNDEFINED_GETTER,
        c::UNDEFINED_SETTER,
        c::UNDEFINED_METHOD,
        c::UNDEFINED_OPERATOR,
        c::AMBIGUOUS_EXTENSION_MEMBER_ACCESS,
        c::AWAIT_IN_WRONG_CONTEXT,
        c::YIELD_EACH_IN_NON_GENERATOR,
        c::NON_BOOL_CONDITION,
        c::NON_BOOL_NEGATION_EXPRESSION,
        c::EXTRA_POSITIONAL_ARGUMENTS,
        c::NOT_ENOUGH_POSITIONAL_ARGUMENTS,
        c::MISSING_REQUIRED_ARGUMENT,
        c::UNDEFINED_NAMED_PARAMETER,
        c::TYPE_ARGUMENT_NOT_MATCHING_BOUNDS,
        c::CONST_WITH_NON_CONSTANT_ARGUMENT,
        c::CONST_EVAL_THROWS_EXCEPTION,
        c::EQUAL_ELEMENTS_IN_CONST_SET,
        c::EQUAL_KEYS_IN_CONST_MAP,
        c::CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE,
        c::INVALID_NULL_AWARE_OPERATOR,
        c::UNNECESSARY_CAST,
        c::UNNECESSARY_TYPE_CHECK_TRUE,
        c::DEAD_CODE,
    ]
}

/// A `types` diagnostic (body or outline) → a diagnostic with a code.
/// `span_text` is the source fragment the diagnostic covers (it serves as the
/// argument when the emitter did not write the name).
pub fn encode_types(d: &Diagnostic, span_text: &str) -> Diagnostic {
    if d.code.is_some() {
        return d.clone();
    }
    for m in types_templates() {
        if let Some(rest) = d.message.strip_prefix(m.template) {
            let Some(code) = by_name(m.name) else { break };
            let mut args = quoted(rest);
            if LAST_OPTIONAL.contains(&code.info().nome) && args.len() == 2 {
                args.push(String::new());
            }
            return Diagnostic::com_codigo(code, d.span, args);
        }
    }
    // Old fixed messages of the outline (today `types` reports type names
    // already with the analyzer code); the name is the text in the span.
    let name = span_text.split('<').next().unwrap_or("").trim_end_matches('?').trim();
    let code = match d.message.as_str() {
        "Tipo não encontrado no escopo da biblioteca" => Some((codigos::compile_time_error::UNDEFINED_CLASS, vec![name])),
        "O símbolo encontrado não é um tipo" | "O elemento prefixado não é um tipo" => {
            Some((codigos::compile_time_error::NOT_A_TYPE, vec![name]))
        }
        "Tipo prefixado não encontrado" => Some((codigos::compile_time_error::UNDEFINED_CLASS, vec![name])),
        "Parâmetro de tipo não aceita argumentos de tipo" => {
            Some((codigos::parser::TYPE_ARGUMENTS_ON_TYPE_VARIABLE, vec![name]))
        }
        "Referência ambígua de tipo" => Some((codigos::compile_time_error::AMBIGUOUS_IMPORT, vec![name, ""])),
        _ => None,
    };
    match code {
        Some((c, args)) => Diagnostic::com_codigo(c, d.span, args),
        None => d.clone(),
    }
}

/// A syntactic diagnostic of `frontend` (lexer and parser) → a
/// `ParserErrorCode`/`ScannerErrorCode` code. Syntax is always published; the
/// code only serves parity (a separate metric, not blocking).
pub fn encode_syntax(d: &Diagnostic) -> Diagnostic {
    use codigos::{parser as p, scanner as s};
    if d.code.is_some() {
        return d.clone();
    }
    // The parser appends the token found: `esperava ';', encontrou '}'`.
    let m = d.message.split(", encontrou ").next().unwrap_or("");
    let (code, args): (Option<Codigo>, Vec<String>) = if let Some(c) = m.strip_prefix("caractere inesperado '") {
        let ch = c.chars().next().map(|c| (c as u32).to_string()).unwrap_or_default();
        (Some(s::ILLEGAL_CHARACTER), vec![ch])
    } else {
        match m {
            "comentário de bloco não terminado" => (Some(s::UNTERMINATED_MULTI_LINE_COMMENT), vec![]),
            "string não terminada" | "quebra de linha em string de aspas simples" => {
                (Some(s::UNTERMINATED_STRING_LITERAL), vec![])
            }
            "'$' precisa de identificador ou '{' em string" | "esperava um identificador após '$'" => {
                (Some(s::MISSING_IDENTIFIER), vec![])
            }
            "esperava um identificador" | "esperava o nome do parâmetro" | "esperava um nome de membro" | "esperava uma expressão" => {
                (Some(p::MISSING_IDENTIFIER), vec![])
            }
            "esperava um tipo" => (Some(p::EXPECTED_TYPE_NAME), vec![]),
            "esperava o corpo da função ('{', '=>' ou ';')" => (Some(p::MISSING_FUNCTION_BODY), vec![]),
            "esperava um statement" => (Some(p::MISSING_STATEMENT), vec![]),
            "esperava uma string" => (Some(p::EXPECTED_STRING_LITERAL), vec![]),
            _ => {
                // `esperava 'x'`, `esperava '}' fechando o bloco`, ...
                let a = quoted(m);
                if m.starts_with("esperava '") && a.len() == 1 && !m.contains(" ou ") && !m.contains(" após ") {
                    (Some(p::EXPECTED_TOKEN), a)
                } else {
                    (None, vec![])
                }
            }
        }
    };
    match code {
        Some(c) => Diagnostic::com_codigo(c, d.span, args),
        None => d.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dartforge_diagnostics::Span;

    #[test]
    fn types_by_template() {
        let t = dartforge_types::codes::UNDEFINED_IDENTIFIER.template;
        let d = Diagnostic::new(format!("{t}: 'h'"), Span { start: 3, end: 4 });
        let c = encode_types(&d, "h");
        assert_eq!(c.code.unwrap().info().nome, "undefined_identifier");
        assert_eq!(c.message, "Undefined name 'h'.");
    }

    #[test]
    fn assigning_const_and_top_level_final_have_their_own_codes() {
        let span = Span { start: 2, end: 3 };
        let constant = Diagnostic::new(dartforge_types::codes::ASSIGNMENT_TO_CONST.template, span);
        let final_ = Diagnostic::new(
            format!("{}: 'x'", dartforge_types::codes::ASSIGNMENT_TO_FINAL.template),
            span,
        );
        let constant = encode_types(&constant, "x");
        assert_eq!(constant.code, Some(codigos::compile_time_error::ASSIGNMENT_TO_CONST));
        assert_eq!(constant.message, "Constant variables can't be assigned a value.");
        let final_ = encode_types(&final_, "x");
        assert_eq!(final_.code, Some(codigos::compile_time_error::ASSIGNMENT_TO_FINAL));
        assert_eq!(final_.message, "'x' can't be used as a setter because it's final.");
    }

    #[test]
    fn assignment_to_type_and_function_gives_official_messages() {
        let span = Span { start: 4, end: 5 };
        for (template, code, message) in [
            (dartforge_types::codes::ASSIGNMENT_TO_TYPE.template, codigos::compile_time_error::ASSIGNMENT_TO_TYPE, "Types can't be assigned a value."),
            (dartforge_types::codes::ASSIGNMENT_TO_FUNCTION.template, codigos::compile_time_error::ASSIGNMENT_TO_FUNCTION, "Functions can't be assigned a value."),
        ] {
            let c = encode_types(&Diagnostic::new(template, span), "C");
            assert_eq!(c.code, Some(code));
            assert_eq!(c.message, message);
        }
    }

    #[test]
    fn class_getter_without_setter_keeps_arguments() {
        let d = Diagnostic::new(
            format!("{}: 'x' na classe 'A'", dartforge_types::codes::ASSIGNMENT_TO_FINAL_NO_SETTER.template),
            Span { start: 4, end: 5 },
        );
        let c = encode_types(&d, "x");
        assert_eq!(c.code, Some(codigos::compile_time_error::ASSIGNMENT_TO_FINAL_NO_SETTER));
        assert_eq!(c.message, "There isn't a setter named 'x' in class 'A'.");
    }

    #[test]
    fn write_to_method_has_official_code() {
        let d = Diagnostic::new(
            dartforge_types::codes::ASSIGNMENT_TO_METHOD.template,
            Span { start: 2, end: 5 },
        );
        let c = encode_types(&d, "foo");
        assert_eq!(c.code, Some(codigos::compile_time_error::ASSIGNMENT_TO_METHOD));
        assert_eq!(c.message, "Methods can't be assigned a value.");
    }

    #[test]
    fn missing_new_constructor_in_explicit_instantiation() {
        let d = Diagnostic::new(
            format!("{}: 'NoUnnamed', 'new'", dartforge_types::codes::NEW_WITH_UNDEFINED_CONSTRUCTOR.template),
            Span { start: 12, end: 15 },
        );
        let c = encode_types(&d, "new");
        assert_eq!(c.code, Some(codigos::compile_time_error::NEW_WITH_UNDEFINED_CONSTRUCTOR));
        assert_eq!(c.message, "The class 'NoUnnamed' doesn't have a constructor named 'new'.");
    }

    #[test]
    fn reference_to_generative_enum_constructor_has_own_code() {
        let d = Diagnostic::new(
            dartforge_types::codes::INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR.template,
            Span { start: 2, end: 7 },
        );
        let c = encode_types(&d, "E.new");
        assert_eq!(c.code, Some(codigos::compile_time_error::INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR));
        assert_eq!(c.message, "Generative enum constructors can only be used as targets of redirection.");
    }

    #[test]
    fn missing_setter_in_extension_override() {
        let d = Diagnostic::new(
            format!("{}: 'foo' em 'E'", dartforge_types::codes::UNDEFINED_EXTENSION_SETTER.template),
            Span { start: 7, end: 10 },
        );
        let c = encode_types(&d, "foo");
        assert_eq!(c.code, Some(codigos::compile_time_error::UNDEFINED_EXTENSION_SETTER));
        assert_eq!(c.message, "The setter 'foo' isn't defined for the extension 'E'.");
    }

    #[test]
    fn missing_getter_in_extension_override() {
        let d = Diagnostic::new(
            format!("{}: 'foo' em 'E'", dartforge_types::codes::UNDEFINED_EXTENSION_GETTER.template),
            Span { start: 7, end: 10 },
        );
        let c = encode_types(&d, "foo");
        assert_eq!(c.code, Some(codigos::compile_time_error::UNDEFINED_EXTENSION_GETTER));
        assert_eq!(c.message, "The getter 'foo' isn't defined for the extension 'E'.");
    }

    #[test]
    fn missing_method_in_extension_override() {
        let d = Diagnostic::new(
            format!("{}: 'm' em 'E'", dartforge_types::codes::UNDEFINED_EXTENSION_METHOD.template),
            Span { start: 7, end: 8 },
        );
        let c = encode_types(&d, "m");
        assert_eq!(c.code, Some(codigos::compile_time_error::UNDEFINED_EXTENSION_METHOD));
        assert_eq!(c.message, "The method 'm' isn't defined for the extension 'E'.");
    }

    #[test]
    fn override_without_call() {
        let d = Diagnostic::new(
            format!("{}: 'E'", dartforge_types::codes::INVOCATION_OF_EXTENSION_WITHOUT_CALL.template),
            Span { start: 7, end: 11 },
        );
        let c = encode_types(&d, "E(0)");
        assert_eq!(c.code, Some(codigos::compile_time_error::INVOCATION_OF_EXTENSION_WITHOUT_CALL));
        assert_eq!(c.message, "The extension 'E' doesn't define a 'call' method so the override can't be used in an invocation.");
    }

    #[test]
    fn missing_operator_in_override() {
        let d = Diagnostic::new(
            format!("{}: 'unary-' em 'E'", dartforge_types::codes::UNDEFINED_EXTENSION_OPERATOR.template),
            Span { start: 33, end: 34 },
        );
        let c = encode_types(&d, "-");
        assert_eq!(c.code, Some(codigos::compile_time_error::UNDEFINED_EXTENSION_OPERATOR));
        assert_eq!(c.message, "The operator 'unary-' isn't defined for the extension 'E'.");
    }

    #[test]
    fn static_member_in_extension_override() {
        let d = Diagnostic::new(
            dartforge_types::codes::EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template,
            Span { start: 7, end: 12 },
        );
        let c = encode_types(&d, "empty");
        assert_eq!(c.code, Some(codigos::compile_time_error::EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER));
        assert_eq!(c.message, "An extension override can't be used to access a static member from an extension.");
        assert_eq!((c.span.start, c.span.end), (7, 12));
    }

    #[test]
    fn instance_member_in_static_extension_access() {
        let d = Diagnostic::new(
            format!("{}: 'g'", dartforge_types::codes::STATIC_ACCESS_TO_INSTANCE_MEMBER.template),
            Span { start: 7, end: 8 },
        );
        let c = encode_types(&d, "g");
        assert_eq!(c.code, Some(codigos::compile_time_error::STATIC_ACCESS_TO_INSTANCE_MEMBER));
        assert_eq!(c.message, "Instance member 'g' can't be accessed using static access.");
    }

    #[test]
    fn instance_member_in_class_instantiation() {
        let d = Diagnostic::new(
            format!("{}: 'i'", dartforge_types::codes::CLASS_INSTANTIATION_ACCESS_TO_INSTANCE_MEMBER.template),
            Span { start: 7, end: 15 },
        );
        let c = encode_types(&d, "A<int>.i");
        assert_eq!(c.code, Some(codigos::compile_time_error::CLASS_INSTANTIATION_ACCESS_TO_INSTANCE_MEMBER));
        assert_eq!(c.message, "The instance member 'i' can't be accessed on a class instantiation.");
        assert_eq!((c.span.start, c.span.end), (7, 15));
    }

    #[test]
    fn static_or_unknown_member_in_class_instantiation() {
        let static_ = Diagnostic::new(
            format!("{}: 'i'", dartforge_types::codes::CLASS_INSTANTIATION_ACCESS_TO_STATIC_MEMBER.template),
            Span { start: 7, end: 15 },
        );
        let c = encode_types(&static_, "A<int>.i");
        assert_eq!(c.code, Some(codigos::compile_time_error::CLASS_INSTANTIATION_ACCESS_TO_STATIC_MEMBER));
        assert_eq!(c.message, "The static member 'i' can't be accessed on a class instantiation.");
        let unknown = Diagnostic::new(
            format!("{}: 'A', 'i'", dartforge_types::codes::CLASS_INSTANTIATION_ACCESS_TO_UNKNOWN_MEMBER.template),
            Span { start: 7, end: 15 },
        );
        let c = encode_types(&unknown, "A<int>.i");
        assert_eq!(c.code, Some(codigos::compile_time_error::CLASS_INSTANTIATION_ACCESS_TO_UNKNOWN_MEMBER));
        assert_eq!(c.message, "The class 'A' doesn't have a constructor named 'i'.");
    }

    #[test]
    fn syntax_expected_token() {
        let d = Diagnostic::new("esperava ';'", Span { start: 3, end: 4 });
        let c = encode_syntax(&d);
        assert_eq!(c.code.unwrap().info().nome, "expected_token");
        assert_eq!(c.message, "Expected to find ';'.");
    }
}
