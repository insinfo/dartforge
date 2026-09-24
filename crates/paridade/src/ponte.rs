//! Ponte provisória: dá código do analyzer aos diagnósticos que as fases de
//! hoje emitem só com texto.
//!
//! O contrato de A2 é o `Diagnostic` com `code`, `severity` e `args`
//! ([`dartforge_diagnostics`]). O parser do `frontend` e a inferência de
//! `types` ainda não o usam: o parser escreve mensagens em português, e
//! `types` escreve `"{molde}: <resto>"` com os moldes de `types::codes`.
//! Até o pedido T1 (códigos com argumentos, emitidos por `types`) e a linha do
//! lexer/parser entrarem, esta ponte **reconhece** esses textos e os converte.
//! Ela não decide semântica: o código é o que o próprio emissor quis dizer
//! (o `name` do `DiagnosticCode`), e os argumentos são os trechos entre
//! aspas que ele escreveu. Onde isso não bate com o oráculo, o placar mostra
//! — é para isso que ele existe. Quando `types` emitir `code`, a ponte deixa
//! de ser consultada para aquele diagnóstico (ver [`codificar_tipos`]).

use dartforge_diagnostics::{Codigo, Diagnostic, codigos};

/// Os trechos entre aspas simples de `s`, na ordem.
fn aspas(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut resto = s;
    while let Some(i) = resto.find('\'') {
        let depois = &resto[i + 1..];
        match depois.find('\'') {
            Some(j) => {
                out.push(depois[..j].to_string());
                resto = &depois[j + 1..];
            }
            None => break,
        }
    }
    out
}

/// Nomes de `types::codes` que não são nomes de código do analyzer 3.6, e o
/// código oficial que descreve a mesma regra.
const APELIDOS: &[(&str, &str)] = &[
    // Leitura de local não anulável antes de atribuição definitiva.
    ("definitely_unassigned_variable", "not_assigned_potentially_non_nullable_local_variable"),
];

/// Nomes relatados que vários `uniqueName` compartilham: qual variante a
/// ponte usa quando o emissor não diz o contexto.
const VARIANTES: &[(&str, &str)] = &[
    ("return_of_invalid_type", "CompileTimeErrorCode.RETURN_OF_INVALID_TYPE_FROM_FUNCTION"),
    ("class_instantiation_access_to_instance_member", "CompileTimeErrorCode.CLASS_INSTANTIATION_ACCESS_TO_INSTANCE_MEMBER"),
];

fn por_nome(nome: &str) -> Option<Codigo> {
    let nome = APELIDOS.iter().find(|(a, _)| *a == nome).map(|(_, b)| *b).unwrap_or(nome);
    if let Some((_, u)) = VARIANTES.iter().find(|(n, _)| *n == nome) {
        return Codigo::por_unico(u);
    }
    Codigo::por_nome(nome)
}

/// Códigos cujo último argumento é informação adicional que, fora de
/// registros, é vazia (`ARGUMENT_TYPE_NOT_ASSIGNABLE` `{2}`).
const ULTIMO_OPCIONAL: &[&str] = &["argument_type_not_assignable"];

/// Os moldes de `types::codes` que a inferência usa hoje.
fn moldes_de_tipos() -> Vec<dartforge_types::DiagnosticCode> {
    use dartforge_types::codes as c;
    vec![
        c::ARGUMENT_TYPE_NOT_ASSIGNABLE,
        c::RETURN_OF_INVALID_TYPE,
        c::INVALID_ASSIGNMENT,
        c::ASSIGNMENT_TO_FINAL_LOCAL,
        c::ASSIGNMENT_TO_FINAL,
        c::ASSIGNMENT_TO_FINAL_NO_SETTER,
        c::UNDEFINED_EXTENSION_SETTER,
        c::UNDEFINED_EXTENSION_GETTER,
        c::UNDEFINED_EXTENSION_METHOD,
        c::STATIC_ACCESS_TO_INSTANCE_MEMBER,
        c::CLASS_INSTANTIATION_ACCESS_TO_INSTANCE_MEMBER,
        c::INVOCATION_OF_EXTENSION_WITHOUT_CALL,
        c::UNDEFINED_EXTENSION_OPERATOR,
        c::EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER,
        c::ASSIGNMENT_TO_CONST,
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

/// Diagnóstico de `types` (corpo ou outline) → diagnóstico com código.
/// `texto_no_span` é o trecho da fonte coberto pelo diagnóstico (serve de
/// argumento quando o emissor não escreveu o nome).
pub fn codificar_tipos(d: &Diagnostic, texto_no_span: &str) -> Diagnostic {
    if d.code.is_some() {
        return d.clone();
    }
    for m in moldes_de_tipos() {
        if let Some(resto) = d.message.strip_prefix(m.template) {
            let Some(codigo) = por_nome(m.name) else { break };
            let mut args = aspas(resto);
            if ULTIMO_OPCIONAL.contains(&codigo.info().nome) && args.len() == 2 {
                args.push(String::new());
            }
            return Diagnostic::com_codigo(codigo, d.span, args);
        }
    }
    // Outline (`types/src/resolve.rs`): mensagens fixas, o nome é o texto no span.
    let nome = texto_no_span.split('<').next().unwrap_or("").trim_end_matches('?').trim();
    let codigo = match d.message.as_str() {
        "Tipo não encontrado no escopo da biblioteca" => Some((codigos::compile_time_error::UNDEFINED_CLASS, vec![nome])),
        "O símbolo encontrado não é um tipo" | "O elemento prefixado não é um tipo" => {
            Some((codigos::compile_time_error::NOT_A_TYPE, vec![nome]))
        }
        "Tipo prefixado não encontrado" => Some((codigos::compile_time_error::UNDEFINED_CLASS, vec![nome])),
        "Parâmetro de tipo não aceita argumentos de tipo" => {
            Some((codigos::parser::TYPE_ARGUMENTS_ON_TYPE_VARIABLE, vec![nome]))
        }
        "Referência ambígua de tipo" => Some((codigos::compile_time_error::AMBIGUOUS_IMPORT, vec![nome, ""])),
        _ => None,
    };
    match codigo {
        Some((c, args)) => Diagnostic::com_codigo(c, d.span, args),
        None => d.clone(),
    }
}

/// Diagnóstico sintático do `frontend` (lexer e parser) → código do
/// `ParserErrorCode`/`ScannerErrorCode`. Sintaxe é publicada sempre; o código
/// só serve à paridade (métrica separada, não bloqueia).
pub fn codificar_sintaxe(d: &Diagnostic) -> Diagnostic {
    use codigos::{parser as p, scanner as s};
    if d.code.is_some() {
        return d.clone();
    }
    // O parser acrescenta o token encontrado: `esperava ';', encontrou '}'`.
    let m = d.message.split(", encontrou ").next().unwrap_or("");
    let (codigo, args): (Option<Codigo>, Vec<String>) = if let Some(c) = m.strip_prefix("caractere inesperado '") {
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
                let a = aspas(m);
                if m.starts_with("esperava '") && a.len() == 1 && !m.contains(" ou ") && !m.contains(" após ") {
                    (Some(p::EXPECTED_TOKEN), a)
                } else {
                    (None, vec![])
                }
            }
        }
    };
    match codigo {
        Some(c) => Diagnostic::com_codigo(c, d.span, args),
        None => d.clone(),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use dartforge_diagnostics::Span;

    #[test]
    fn tipos_pelo_molde() {
        let t = dartforge_types::codes::UNDEFINED_IDENTIFIER.template;
        let d = Diagnostic::new(format!("{t}: 'h'"), Span { start: 3, end: 4 });
        let c = codificar_tipos(&d, "h");
        assert_eq!(c.code.unwrap().info().nome, "undefined_identifier");
        assert_eq!(c.message, "Undefined name 'h'.");
    }

    #[test]
    fn atribuir_const_e_final_de_topo_tem_codigos_proprios() {
        let span = Span { start: 2, end: 3 };
        let constante = Diagnostic::new(dartforge_types::codes::ASSIGNMENT_TO_CONST.template, span);
        let final_ = Diagnostic::new(
            format!("{}: 'x'", dartforge_types::codes::ASSIGNMENT_TO_FINAL.template),
            span,
        );
        let constante = codificar_tipos(&constante, "x");
        assert_eq!(constante.code, Some(codigos::compile_time_error::ASSIGNMENT_TO_CONST));
        assert_eq!(constante.message, "Constant variables can't be assigned a value.");
        let final_ = codificar_tipos(&final_, "x");
        assert_eq!(final_.code, Some(codigos::compile_time_error::ASSIGNMENT_TO_FINAL));
        assert_eq!(final_.message, "'x' can't be used as a setter because it's final.");
    }

    #[test]
    fn getter_de_classe_sem_setter_preserva_argumentos() {
        let d = Diagnostic::new(
            format!("{}: 'x' na classe 'A'", dartforge_types::codes::ASSIGNMENT_TO_FINAL_NO_SETTER.template),
            Span { start: 4, end: 5 },
        );
        let c = codificar_tipos(&d, "x");
        assert_eq!(c.code, Some(codigos::compile_time_error::ASSIGNMENT_TO_FINAL_NO_SETTER));
        assert_eq!(c.message, "There isn't a setter named 'x' in class 'A'.");
    }

    #[test]
    fn setter_ausente_em_sobreposicao_de_extensao() {
        let d = Diagnostic::new(
            format!("{}: 'foo' em 'E'", dartforge_types::codes::UNDEFINED_EXTENSION_SETTER.template),
            Span { start: 7, end: 10 },
        );
        let c = codificar_tipos(&d, "foo");
        assert_eq!(c.code, Some(codigos::compile_time_error::UNDEFINED_EXTENSION_SETTER));
        assert_eq!(c.message, "The setter 'foo' isn't defined for the extension 'E'.");
    }

    #[test]
    fn getter_ausente_em_sobreposicao_de_extensao() {
        let d = Diagnostic::new(
            format!("{}: 'foo' em 'E'", dartforge_types::codes::UNDEFINED_EXTENSION_GETTER.template),
            Span { start: 7, end: 10 },
        );
        let c = codificar_tipos(&d, "foo");
        assert_eq!(c.code, Some(codigos::compile_time_error::UNDEFINED_EXTENSION_GETTER));
        assert_eq!(c.message, "The getter 'foo' isn't defined for the extension 'E'.");
    }

    #[test]
    fn metodo_ausente_em_sobreposicao_de_extensao() {
        let d = Diagnostic::new(
            format!("{}: 'm' em 'E'", dartforge_types::codes::UNDEFINED_EXTENSION_METHOD.template),
            Span { start: 7, end: 8 },
        );
        let c = codificar_tipos(&d, "m");
        assert_eq!(c.code, Some(codigos::compile_time_error::UNDEFINED_EXTENSION_METHOD));
        assert_eq!(c.message, "The method 'm' isn't defined for the extension 'E'.");
    }

    #[test]
    fn override_sem_call() {
        let d = Diagnostic::new(
            format!("{}: 'E'", dartforge_types::codes::INVOCATION_OF_EXTENSION_WITHOUT_CALL.template),
            Span { start: 7, end: 11 },
        );
        let c = codificar_tipos(&d, "E(0)");
        assert_eq!(c.code, Some(codigos::compile_time_error::INVOCATION_OF_EXTENSION_WITHOUT_CALL));
        assert_eq!(c.message, "The extension 'E' doesn't define a 'call' method so the override can't be used in an invocation.");
    }

    #[test]
    fn operador_ausente_em_override() {
        let d = Diagnostic::new(
            format!("{}: 'unary-' em 'E'", dartforge_types::codes::UNDEFINED_EXTENSION_OPERATOR.template),
            Span { start: 33, end: 34 },
        );
        let c = codificar_tipos(&d, "-");
        assert_eq!(c.code, Some(codigos::compile_time_error::UNDEFINED_EXTENSION_OPERATOR));
        assert_eq!(c.message, "The operator 'unary-' isn't defined for the extension 'E'.");
    }

    #[test]
    fn membro_estatico_em_sobreposicao_de_extensao() {
        let d = Diagnostic::new(
            dartforge_types::codes::EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER.template,
            Span { start: 7, end: 12 },
        );
        let c = codificar_tipos(&d, "empty");
        assert_eq!(c.code, Some(codigos::compile_time_error::EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER));
        assert_eq!(c.message, "An extension override can't be used to access a static member from an extension.");
        assert_eq!((c.span.start, c.span.end), (7, 12));
    }

    #[test]
    fn membro_de_instancia_em_acesso_estatico_a_extensao() {
        let d = Diagnostic::new(
            format!("{}: 'g'", dartforge_types::codes::STATIC_ACCESS_TO_INSTANCE_MEMBER.template),
            Span { start: 7, end: 8 },
        );
        let c = codificar_tipos(&d, "g");
        assert_eq!(c.code, Some(codigos::compile_time_error::STATIC_ACCESS_TO_INSTANCE_MEMBER));
        assert_eq!(c.message, "Instance member 'g' can't be accessed using static access.");
    }

    #[test]
    fn membro_de_instancia_em_instanciacao_de_classe() {
        let d = Diagnostic::new(
            format!("{}: 'i'", dartforge_types::codes::CLASS_INSTANTIATION_ACCESS_TO_INSTANCE_MEMBER.template),
            Span { start: 7, end: 15 },
        );
        let c = codificar_tipos(&d, "A<int>.i");
        assert_eq!(c.code, Some(codigos::compile_time_error::CLASS_INSTANTIATION_ACCESS_TO_INSTANCE_MEMBER));
        assert_eq!(c.message, "The instance member 'i' can't be accessed on a class instantiation.");
        assert_eq!((c.span.start, c.span.end), (7, 15));
    }

    #[test]
    fn sintaxe_token_esperado() {
        let d = Diagnostic::new("esperava ';'", Span { start: 3, end: 4 });
        let c = codificar_sintaxe(&d);
        assert_eq!(c.code.unwrap().info().nome, "expected_token");
        assert_eq!(c.message, "Expected to find ';'.");
    }
}
