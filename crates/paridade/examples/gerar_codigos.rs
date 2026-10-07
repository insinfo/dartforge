//! Gera `crates/diagnostics/src/codigos_g.rs`: a tabela de códigos do
//! `package:analyzer` 6.11.0 (o analyzer do SDK 3.6.2), lida das fontes na
//! cache do pub.
//!
//! ```text
//! cargo run -p dartforge-paridade --example gerar_codigos [-- <pub-cache/hosted/pub.dev>]
//! ```
//!
//! Determinístico: a mesma fonte dá o mesmo arquivo, byte a byte. As fontes
//! são os `.g.dart` gerados do `messages.yaml` do analyzer (e os dois arquivos
//! escritos à mão, do scanner e dos TODOs), lidos por um tokenizador de Dart
//! pequeno: `static const <Classe> <NOME> = [const] <Classe>(<args>);`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Classe do analyzer → módulo Rust, na ordem da tabela.
const CLASSES: &[(&str, &str, &str)] = &[
    ("CompileTimeErrorCode", "compile_time_error", "analyzer-6.11.0/lib/src/error/codes.g.dart"),
    ("StaticWarningCode", "static_warning", "analyzer-6.11.0/lib/src/error/codes.g.dart"),
    ("WarningCode", "warning", "analyzer-6.11.0/lib/src/error/codes.g.dart"),
    ("HintCode", "hint", "analyzer-6.11.0/lib/src/dart/error/hint_codes.g.dart"),
    ("FfiCode", "ffi", "analyzer-6.11.0/lib/src/dart/error/ffi_code.g.dart"),
    ("ParserErrorCode", "parser", "analyzer-6.11.0/lib/src/dart/error/syntactic_errors.g.dart"),
    ("ScannerErrorCode", "scanner", "_fe_analyzer_shared-76.0.0/lib/src/scanner/errors.dart"),
    ("TodoCode", "todo", "analyzer-6.11.0/lib/src/dart/error/todo_codes.dart"),
];

/// Códigos do analyzer do SDK 3.13.4 que a tabela 6.11 não tem (construtores
/// primários, atalhos de ponto) e as formas 3.13.4 de códigos que ela tem. Transcritos do
/// `messages.yaml` da referência (`references/dart-sdk/pkg/_fe_analyzer_shared`
/// e `pkg/analyzer`) e conferidos, texto e tipo, contra o oráculo 3.13.4
/// gravado em `corpus/diagnosticos`. Vão no fim da tabela: os índices dos
/// códigos da 6.11 não mudam.
///
/// (classe, módulo, constante, nome, unico, mensagem, correção, tipo)
const SUPLEMENTO_3_13: &[(&str, &str, &str, &str, &str, &str, &str, &str)] = &[
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "NON_REDIRECTING_GENERATIVE_CONSTRUCTOR_WITH_PRIMARY",
        "non_redirecting_generative_constructor_with_primary",
        "CompileTimeErrorCode.NON_REDIRECTING_GENERATIVE_CONSTRUCTOR_WITH_PRIMARY",
        "Classes with primary constructors can't have non-redirecting generative constructors.",
        "Try making the constructor redirect to the primary constructor, or remove the primary constructor.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "PRIMARY_CONSTRUCTOR_BODY_WITHOUT_DECLARATION",
        "primary_constructor_body_without_declaration",
        "CompileTimeErrorCode.PRIMARY_CONSTRUCTOR_BODY_WITHOUT_DECLARATION",
        "A primary constructor body requires a primary constructor declaration.",
        "Try adding the primary constructor declaration.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "MULTIPLE_PRIMARY_CONSTRUCTOR_BODY_DECLARATIONS",
        "multiple_primary_constructor_body_declarations",
        "CompileTimeErrorCode.MULTIPLE_PRIMARY_CONSTRUCTOR_BODY_DECLARATIONS",
        "Only one primary constructor body declaration is allowed.",
        "Try removing all but one of the primary constructor body declarations.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "PRIMARY_CONSTRUCTOR_BODY_WITH_EXPRESSION_BODY",
        "primary_constructor_body_with_expression_body",
        "CompileTimeErrorCode.PRIMARY_CONSTRUCTOR_BODY_WITH_EXPRESSION_BODY",
        "A primary constructor body can't use '=>'.",
        "Try using a block body.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "ParserErrorCode",
        "parser",
        "CONST_PRIMARY_CONSTRUCTOR_WITH_BLOCK_BODY",
        "const_primary_constructor_with_body",
        "ParserErrorCode.CONST_PRIMARY_CONSTRUCTOR_WITH_BLOCK_BODY",
        "The body part of a constant primary constructor can't have a block body.",
        "Try replacing the block body with a semicolon, or removing the 'const' modifier.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "ParserErrorCode",
        "parser",
        "CONST_PRIMARY_CONSTRUCTOR_WITH_EXPRESSION_BODY",
        "const_primary_constructor_with_body",
        "ParserErrorCode.CONST_PRIMARY_CONSTRUCTOR_WITH_EXPRESSION_BODY",
        "The body part of a constant primary constructor can't have an expression body.",
        "Try replacing the expression body with a semicolon, or removing the 'const' modifier.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "ParserErrorCode",
        "parser",
        "PRIMARY_CONSTRUCTOR_BODY_WITH_MODIFIER",
        "primary_constructor_body_with_modifier",
        "ParserErrorCode.PRIMARY_CONSTRUCTOR_BODY_WITH_MODIFIER",
        "A primary constructor body can't have the modifier '{0}'.",
        "Try removing the modifier.",
        "SYNTACTIC_ERROR",
    ),
    // Atalhos de ponto (3.10), do `ResolverVisitor` do analyzer 3.13.4;
    // o de contexto que falta não tem correção.
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "DOT_SHORTHAND_MISSING_CONTEXT",
        "dot_shorthand_missing_context",
        "CompileTimeErrorCode.DOT_SHORTHAND_MISSING_CONTEXT",
        "A dot shorthand can't be used where there is no context type.",
        "",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "DOT_SHORTHAND_UNDEFINED_GETTER",
        "dot_shorthand_undefined_member",
        "CompileTimeErrorCode.DOT_SHORTHAND_UNDEFINED_GETTER",
        "The static getter '{0}' isn't defined for the context type '{1}'.",
        "Try correcting the name to the name of an existing static getter, or defining a getter or field named '{0}'.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "DOT_SHORTHAND_UNDEFINED_INVOCATION",
        "dot_shorthand_undefined_member",
        "CompileTimeErrorCode.DOT_SHORTHAND_UNDEFINED_INVOCATION",
        "The static method or constructor '{0}' isn't defined for the context type '{1}'.",
        "Try correcting the name to the name of an existing static method or constructor, or defining a static method or constructor named '{0}'.",
        "COMPILE_TIME_ERROR",
    ),
    // Códigos que só o 3.13.4 tem (docs/ANALYZER-ESPECIFICACAO.md, T2 §1.3 (c)); textos
    // conferidos no oráculo 3.13.4 gravado em `corpus/diagnosticos`.
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "ASSIGNMENT_TO_PRIMARY_CONSTRUCTOR_PARAMETER",
        "assignment_to_primary_constructor_parameter",
        "CompileTimeErrorCode.ASSIGNMENT_TO_PRIMARY_CONSTRUCTOR_PARAMETER",
        "A primary constructor parameter can't be assigned to in an initializer.",
        "Try removing the assignment.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "PRIMARY_CONSTRUCTOR_CANNOT_REDIRECT",
        "primary_constructor_cannot_redirect",
        "CompileTimeErrorCode.PRIMARY_CONSTRUCTOR_CANNOT_REDIRECT",
        "A primary constructor can't be a redirecting constructor.",
        "Try removing the redirect.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "MIXIN_CLASS_DECLARES_NON_TRIVIAL_GENERATIVE_CONSTRUCTOR",
        "mixin_class_declares_non_trivial_generative_constructor",
        "CompileTimeErrorCode.MIXIN_CLASS_DECLARES_NON_TRIVIAL_GENERATIVE_CONSTRUCTOR",
        "The mixin class '{0}' can't declare a non-trivial generative constructor.",
        "",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "FIELD_INITIALIZED_IN_DECLARATION_AND_PARAMETER_OF_PRIMARY_CONSTRUCTOR",
        "field_initialized_in_declaration_and_parameter_of_primary_constructor",
        "CompileTimeErrorCode.FIELD_INITIALIZED_IN_DECLARATION_AND_PARAMETER_OF_PRIMARY_CONSTRUCTOR",
        "Fields can't be initialized in both the primary constructor parameter list and at their declaration.",
        "Try removing one of the initializations.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "FIELD_INITIALIZED_IN_DECLARATION_AND_INITIALIZER_OF_PRIMARY_CONSTRUCTOR",
        "field_initialized_in_declaration_and_initializer_of_primary_constructor",
        "CompileTimeErrorCode.FIELD_INITIALIZED_IN_DECLARATION_AND_INITIALIZER_OF_PRIMARY_CONSTRUCTOR",
        "Fields can't be initialized in both the primary constructor and at their declaration.",
        "Try removing one of the initializations.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "WRONG_NUMBER_OF_TYPE_ARGUMENTS_DOT_SHORTHAND_CONSTRUCTOR",
        "wrong_number_of_type_arguments_constructor",
        "CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_DOT_SHORTHAND_CONSTRUCTOR",
        "The dot shorthand resolves to the constructor '{0}.{1}', and type parameters can't be applied to dot shorthand constructor invocations.",
        "Try removing the type arguments, or adding a class name, followed by the type arguments, then the constructor name.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "ParserErrorCode",
        "parser",
        "INITIALIZING_DECLARING_PARAMETER",
        "initializing_declaring_parameter",
        "ParserErrorCode.INITIALIZING_DECLARING_PARAMETER",
        "Declaring parameters can't be initializing.",
        "Try removing the `this.` prefix or making the parameter non-declaring.",
        "SYNTACTIC_ERROR",
    ),
    (
        "ParserErrorCode",
        "parser",
        "SUPER_INITIALIZING_DECLARING_PARAMETER",
        "super_initializing_declaring_parameter",
        "ParserErrorCode.SUPER_INITIALIZING_DECLARING_PARAMETER",
        "Declaring parameters can't be super parameters.",
        "Try removing the `super.` prefix or making the parameter non-declaring.",
        "SYNTACTIC_ERROR",
    ),
    (
        "ParserErrorCode",
        "parser",
        "FACTORY_CONSTRUCTOR_NEW_NAME",
        "factory_constructor_new_name",
        "ParserErrorCode.FACTORY_CONSTRUCTOR_NEW_NAME",
        "Factory constructors can't be named 'new'.",
        "Try removing the 'new' keyword or changing it to a different name.",
        "SYNTACTIC_ERROR",
    ),
    (
        "WarningCode",
        "warning",
        "DEPRECATED_OPTIONAL",
        "deprecated_optional",
        "WarningCode.DEPRECATED_OPTIONAL",
        "Omitting an argument for the '{0}' parameter is deprecated.",
        "Try passing an argument for '{0}'.",
        "STATIC_WARNING",
    ),
    (
        "WarningCode",
        "warning",
        "UNUSED_FIELD_FROM_PRIMARY_CONSTRUCTOR",
        "unused_field_from_primary_constructor",
        "WarningCode.UNUSED_FIELD_FROM_PRIMARY_CONSTRUCTOR",
        "The value of the field '{0}' isn't used.",
        "Try removing the '{1}' keyword to avoid declaring a field, or try using the field, or removing it.",
        "STATIC_WARNING",
    ),
    // As formas 3.13.4 de códigos que a 6.11 já tem (T2 §1.3 (a) e (b)): os destinos de
    // `VARIANTES_3_13`. O sufixo `_3_13` só distingue o nome único; o nome relatado é o da coluna
    // seguinte. Textos do oráculo vivo 3.13.4 (casos `corpus/especificacao/t2/v`).
    (
        "WarningCode",
        "warning",
        "UNUSED_ELEMENT_PARAMETER_3_13",
        "unused_element_parameter",
        "WarningCode.UNUSED_ELEMENT_PARAMETER_3_13",
        "A value for optional parameter '{0}' isn't ever given.",
        "Try removing the unused parameter.",
        "STATIC_WARNING",
    ),
    (
        "ParserErrorCode",
        "parser",
        "REPRESENTATION_FIELD_MODIFIER_3_13",
        "representation_field_modifier",
        "ParserErrorCode.REPRESENTATION_FIELD_MODIFIER_3_13",
        "Representation fields can't have the modifier '{0}'.",
        "Try removing the modifier.",
        "SYNTACTIC_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "ENUM_WITHOUT_CONSTANTS_3_13",
        "enum_without_constants",
        "CompileTimeErrorCode.ENUM_WITHOUT_CONSTANTS_3_13",
        "The enum must have at least one enum constant.",
        "Try declaring an enum constant.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "ASSIGNMENT_TO_CONST_3_13",
        "assignment_to_const",
        "CompileTimeErrorCode.ASSIGNMENT_TO_CONST_3_13",
        "Constant variables can't be assigned a value after initialization.",
        "Try removing the assignment, or remove the modifier 'const' from the variable.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "NULLABLE_TYPE_IN_IMPLEMENTS_CLAUSE_3_13",
        "nullable_type_in_implements_clause",
        "CompileTimeErrorCode.NULLABLE_TYPE_IN_IMPLEMENTS_CLAUSE_3_13",
        "Nullable types can't be implemented.",
        "Try removing the question mark.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "SUPER_INVOCATION_NOT_LAST_3_13",
        "super_invocation_not_last",
        "CompileTimeErrorCode.SUPER_INVOCATION_NOT_LAST_3_13",
        "The superconstructor call must be last in an initializer list.",
        "",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "NON_EXHAUSTIVE_SWITCH_EXPRESSION_3_13",
        "non_exhaustive_switch_expression",
        "CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_EXPRESSION_3_13",
        "The type '{0}' isn't exhaustively matched by the switch cases since it doesn't match the pattern '{1}'.",
        "Try adding a wildcard pattern or cases that match '{2}'.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "NON_EXHAUSTIVE_SWITCH_EXPRESSION_PRIVATE",
        "non_exhaustive_switch_expression",
        "CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_EXPRESSION_PRIVATE",
        "The enum '{0}' isn't exhaustively matched by the switch cases because some of the enum constants are private.",
        "Try adding a wildcard pattern.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "NON_EXHAUSTIVE_SWITCH_STATEMENT_3_13",
        "non_exhaustive_switch_statement",
        "CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_STATEMENT_3_13",
        "The type '{0}' isn't exhaustively matched by the switch cases since it doesn't match the pattern '{1}'.",
        "Try adding a default case or cases that match '{2}'.",
        "COMPILE_TIME_ERROR",
    ),
    // A variante de comando com constantes privadas não foi rodada no oráculo vivo: texto do
    // `messages.yaml` do main (a de expressão foi, caso c29).
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "NON_EXHAUSTIVE_SWITCH_STATEMENT_PRIVATE",
        "non_exhaustive_switch_statement",
        "CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_STATEMENT_PRIVATE",
        "The enum '{0}' isn't exhaustively matched by the switch cases because some of the enum constants are private.",
        "Try adding a default case.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "WRONG_NUMBER_OF_TYPE_ARGUMENTS_ELEMENT",
        "wrong_number_of_type_arguments_element",
        "CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_ELEMENT",
        "The {0} '{1}' is declared with {2} type parameters, but {3} type arguments are given.",
        "Try adjusting the number of type arguments.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION_3_13",
        "wrong_number_of_type_arguments_function",
        "CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION_3_13",
        "The type of this function is '{0}', which has {1} type parameters, but {2} type arguments were given.",
        "Try adjusting the number of type arguments to match the number of type parameters.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "AMBIGUOUS_EXTENSION_MEMBER_ACCESS_TWO",
        "ambiguous_extension_member_access",
        "CompileTimeErrorCode.AMBIGUOUS_EXTENSION_MEMBER_ACCESS_TWO",
        "A member named '{0}' is defined in '{1}' and '{2}', and neither is more specific.",
        "Try using an extension override to specify the extension you want to be chosen.",
        "COMPILE_TIME_ERROR",
    ),
    (
        "CompileTimeErrorCode",
        "compile_time_error",
        "DEFERRED_IMPORT_OF_EXTENSION_3_13",
        "deferred_import_of_extension",
        "CompileTimeErrorCode.DEFERRED_IMPORT_OF_EXTENSION_3_13",
        "Deferred library imports must hide all extension declarations.",
        "Try adding either a show combinator listing the names you need to reference or a hide combinator listing all of the extension declarations.",
        "COMPILE_TIME_ERROR",
    ),
];

/// Como um código da 6.11 sai quando a referência do arquivo é o analyzer
/// 3.13.4 (docs/ANALYZER-ESPECIFICACAO.md, T2 §3). O emissor passa os
/// argumentos do molde 3.6 e, depois deles, os que só o 3.13 usa; a terceira
/// coluna diz quais vão, e em que ordem, para o molde de destino. A quarta é
/// o número mínimo de argumentos que o diagnóstico tem de trazer para a
/// linha valer (0: o que os índices pedem); com várias linhas para o mesmo
/// código, vale a primeira que couber, e se nenhuma couber o diagnóstico sai
/// como no 3.6. Destino vazio: o 3.13.4 não relata.
///
/// (de, para, argumentos, mínimo)
const VARIANTES_3_13: &[(&str, &str, &[u8], u8)] = &[
    // `{1}` da correção: `[recurso, versão 3.6.2, versão 3.13.4]`.
    ("ParserErrorCode.EXPERIMENT_NOT_ENABLED", "ParserErrorCode.EXPERIMENT_NOT_ENABLED", &[0, 2], 0),
    ("ParserErrorCode.REPRESENTATION_FIELD_MODIFIER", "ParserErrorCode.REPRESENTATION_FIELD_MODIFIER_3_13", &[0], 0),
    ("WarningCode.UNUSED_ELEMENT_PARAMETER", "WarningCode.UNUSED_ELEMENT_PARAMETER_3_13", &[0], 0),
    ("CompileTimeErrorCode.ENUM_WITHOUT_CONSTANTS", "CompileTimeErrorCode.ENUM_WITHOUT_CONSTANTS_3_13", &[], 0),
    ("CompileTimeErrorCode.ASSIGNMENT_TO_CONST", "CompileTimeErrorCode.ASSIGNMENT_TO_CONST_3_13", &[], 0),
    ("CompileTimeErrorCode.NULLABLE_TYPE_IN_IMPLEMENTS_CLAUSE", "CompileTimeErrorCode.NULLABLE_TYPE_IN_IMPLEMENTS_CLAUSE_3_13", &[], 0),
    ("CompileTimeErrorCode.SUPER_INVOCATION_NOT_LAST", "CompileTimeErrorCode.SUPER_INVOCATION_NOT_LAST_3_13", &[], 0),
    // `[classe do construtor, superclasse]`.
    ("CompileTimeErrorCode.CONST_CONSTRUCTOR_WITH_NON_CONST_SUPER", "CompileTimeErrorCode.CONST_CONSTRUCTOR_WITH_NON_CONST_SUPER", &[1], 0),
    // `[tipo, padrão, sugestão]` e, quando o enum é de outra biblioteca e
    // o que falta são constantes privadas, um quarto argumento qualquer.
    ("CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_EXPRESSION", "CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_EXPRESSION_PRIVATE", &[0], 4),
    ("CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_EXPRESSION", "CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_EXPRESSION_3_13", &[0, 1, 2], 0),
    ("CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_STATEMENT", "CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_STATEMENT_PRIVATE", &[0], 4),
    ("CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_STATEMENT", "CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_STATEMENT_3_13", &[0, 1, 2], 0),
    // `[tipo do alvo, declarados, dados]` e, quando o alvo é um elemento,
    // `espécie` (`method`, `function`) e nome.
    ("CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_METHOD", "CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_ELEMENT", &[3, 4, 1, 2], 0),
    ("CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_METHOD", "CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION_3_13", &[0, 1, 2], 0),
    // A variante anônima: `[declarados, dados, tipo da função]` (o tipo só no 3.13.4).
    ("CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_ANONYMOUS_FUNCTION", "CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION_3_13", &[2, 0, 1], 3),
    // `[membro, lista 3.6]` e, só quando são exatamente duas, as duas
    // extensões na exibição do 3.13 (`extension E1 on int`).
    ("CompileTimeErrorCode.AMBIGUOUS_EXTENSION_MEMBER_ACCESS", "CompileTimeErrorCode.AMBIGUOUS_EXTENSION_MEMBER_ACCESS_TWO", &[0, 2, 3], 0),
    // `[classe, caminho 3.6, caminho 3.13]`.
    ("CompileTimeErrorCode.RECURSIVE_INTERFACE_INHERITANCE", "CompileTimeErrorCode.RECURSIVE_INTERFACE_INHERITANCE", &[0, 2], 0),
    // A variante do verificador: no 3.13.4 só sai a do parser.
    ("CompileTimeErrorCode.FIELD_INITIALIZER_OUTSIDE_CONSTRUCTOR", "", &[], 0),
    ("CompileTimeErrorCode.MIXIN_CLASS_DECLARES_CONSTRUCTOR", "CompileTimeErrorCode.MIXIN_CLASS_DECLARES_NON_TRIVIAL_GENERATIVE_CONSTRUCTOR", &[0], 0),
    ("CompileTimeErrorCode.DEFERRED_IMPORT_OF_EXTENSION", "CompileTimeErrorCode.DEFERRED_IMPORT_OF_EXTENSION_3_13", &[], 0),
];

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Id(String),
    Str(String),
    P(char),
    Seta,
}

fn tokenizar(src: &str) -> Vec<Tok> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
        } else if b[i..].starts_with(b"//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if b[i..].starts_with(b"/*") {
            let mut prof = 0;
            while i < b.len() {
                if b[i..].starts_with(b"/*") {
                    prof += 1;
                    i += 2;
                } else if b[i..].starts_with(b"*/") {
                    prof -= 1;
                    i += 2;
                    if prof == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else if c == b'\'' || c == b'"' || (c == b'r' && matches!(b.get(i + 1), Some(b'\'') | Some(b'"'))) {
            let cru = c == b'r';
            if cru {
                i += 1;
            }
            let q = b[i];
            let triplo = b[i..].starts_with(&[q, q, q]);
            let fim: Vec<u8> = if triplo { vec![q, q, q] } else { vec![q] };
            i += fim.len();
            let mut s = Vec::new();
            while i < b.len() && !b[i..].starts_with(&fim) {
                if b[i] == b'\\' && !cru {
                    let e = b[i + 1];
                    match e {
                        b'n' => s.push(b'\n'),
                        b't' => s.push(b'\t'),
                        b'r' => s.push(b'\r'),
                        _ => s.push(e),
                    }
                    i += 2;
                } else {
                    s.push(b[i]);
                    i += 1;
                }
            }
            i += fim.len();
            out.push(Tok::Str(String::from_utf8(s).expect("UTF-8")));
        } else if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' {
            let ini = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
                i += 1;
            }
            out.push(Tok::Id(src[ini..i].to_string()));
        } else if b[i..].starts_with(b"=>") {
            out.push(Tok::Seta);
            i += 2;
        } else {
            let ch = src[i..].chars().next().unwrap();
            out.push(Tok::P(ch));
            i += ch.len_utf8();
        }
    }
    out
}

#[derive(Debug)]
struct Entrada {
    classe: String,
    constante: String,
    nome: String,
    unico: String,
    mensagem: String,
    correcao: Option<String>,
    documentado: bool,
}

/// Um argumento: nome (se nomeado) e tokens do valor.
fn argumentos(toks: &[Tok]) -> Vec<(Option<String>, Vec<Tok>)> {
    let mut args = Vec::new();
    let mut atual: Vec<Tok> = Vec::new();
    let mut prof = 0i32;
    for t in toks {
        match t {
            Tok::P('(') | Tok::P('[') | Tok::P('{') => prof += 1,
            Tok::P(')') | Tok::P(']') | Tok::P('}') => prof -= 1,
            _ => {}
        }
        if prof == 0 && *t == Tok::P(',') {
            args.push(std::mem::take(&mut atual));
        } else {
            atual.push(t.clone());
        }
    }
    if !atual.is_empty() {
        args.push(atual);
    }
    args.into_iter()
        .map(|a| {
            if let [Tok::Id(n), Tok::P(':'), ..] = a.as_slice() {
                (Some(n.clone()), a[2..].to_vec())
            } else {
                (None, a)
            }
        })
        .collect()
}

fn texto(v: &[Tok]) -> Option<String> {
    let mut s = String::new();
    let mut algum = false;
    for t in v {
        match t {
            Tok::Str(x) => {
                s.push_str(x);
                algum = true;
            }
            _ => return None,
        }
    }
    algum.then_some(s)
}

/// Lê as entradas e o par (tipo, severidade) de cada classe do arquivo.
fn ler_classe(src: &str, classe: &str) -> (Vec<Entrada>, String, String) {
    let t = tokenizar(src);
    let mut i = 0;
    // `class <classe> extends ... {`
    while i + 1 < t.len() && !(t[i] == Tok::Id("class".into()) && t[i + 1] == Tok::Id(classe.into())) {
        i += 1;
    }
    assert!(i + 1 < t.len(), "classe {classe} não encontrada");
    while t[i] != Tok::P('{') {
        i += 1;
    }
    i += 1;
    let mut prof = 1;
    let mut entradas = Vec::new();
    let mut tipo = None;
    let mut severidade = None;
    while i < t.len() && prof > 0 {
        match &t[i] {
            Tok::P('{') => prof += 1,
            Tok::P('}') => prof -= 1,
            Tok::Id(s) if prof == 1 && s == "static" && t.get(i + 1) == Some(&Tok::Id("const".into())) => {
                // static const <Classe> <NOME> = [const] <Classe>( ... ) ;
                let Some(Tok::Id(tipo_decl)) = t.get(i + 2) else { panic!("static const sem tipo") };
                let Some(Tok::Id(constante)) = t.get(i + 3) else { panic!("static const sem nome") };
                if tipo_decl != classe {
                    i += 1;
                    continue;
                }
                let mut j = i + 4;
                assert_eq!(t[j], Tok::P('='), "{constante}");
                j += 1;
                if t[j] == Tok::Id("const".into()) {
                    j += 1;
                }
                assert_eq!(t[j], Tok::Id(classe.into()), "{constante}");
                j += 1;
                assert_eq!(t[j], Tok::P('('), "{constante}");
                let ini = j + 1;
                let mut p = 1;
                j += 1;
                while p > 0 {
                    match t[j] {
                        Tok::P('(') => p += 1,
                        Tok::P(')') => p -= 1,
                        _ => {}
                    }
                    j += 1;
                }
                let args = argumentos(&t[ini..j - 1]);
                let pos: Vec<String> = args.iter().filter(|a| a.0.is_none()).map(|a| texto(&a.1).expect("string")).collect();
                let nomeado = |n: &str| args.iter().find(|a| a.0.as_deref() == Some(n)).map(|a| &a.1);
                let nome = pos[0].clone();
                let mensagem = if classe == "TodoCode" { "{0}".to_string() } else { pos[1].clone() };
                let unico_curto = nomeado("uniqueName").and_then(|v| texto(v)).unwrap_or_else(|| nome.clone());
                entradas.push(Entrada {
                    classe: classe.to_string(),
                    constante: constante.clone(),
                    nome: nome.to_lowercase(),
                    unico: format!("{classe}.{unico_curto}"),
                    mensagem,
                    correcao: nomeado("correctionMessage").and_then(|v| texto(v)),
                    documentado: nomeado("hasPublishedDocs").is_some_and(|v| v.as_slice() == [Tok::Id("true".into())]),
                });
                i = j;
                continue;
            }
            Tok::Id(s) if prof == 1 && (s == "ErrorSeverity" || s == "ErrorType") && t.get(i + 1) == Some(&Tok::Id("get".into())) => {
                let getter = match &t[i + 2] {
                    Tok::Id(g) => g.clone(),
                    _ => String::new(),
                };
                assert_eq!(t[i + 3], Tok::Seta);
                // `ErrorType.X.severity`, `ErrorSeverity.X` ou `ErrorType.X`.
                let mut j = i + 4;
                let mut expr = Vec::new();
                while t[j] != Tok::P(';') {
                    if let Tok::Id(x) = &t[j] {
                        expr.push(x.clone());
                    }
                    j += 1;
                }
                let valor = match expr.as_slice() {
                    [a, x, s] if a == "ErrorType" && s == "severity" => format!("tipo:{x}"),
                    [a, x] if a == "ErrorSeverity" || a == "ErrorType" => x.clone(),
                    _ => panic!("getter {getter} de {classe} não reconhecido: {expr:?}"),
                };
                if getter == "errorSeverity" {
                    severidade = Some(valor);
                } else if getter == "type" {
                    tipo = Some(valor);
                }
                i = j;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    (entradas, tipo.expect("getter type"), severidade.expect("getter errorSeverity"))
}

fn tipo_rust(t: &str) -> &'static str {
    match t {
        "TODO" => "TipoErro::Todo",
        "HINT" => "TipoErro::Hint",
        "COMPILE_TIME_ERROR" => "TipoErro::CompileTimeError",
        "CHECKED_MODE_COMPILE_TIME_ERROR" => "TipoErro::CheckedModeCompileTimeError",
        "STATIC_WARNING" => "TipoErro::StaticWarning",
        "SYNTACTIC_ERROR" => "TipoErro::SyntacticError",
        "LINT" => "TipoErro::Lint",
        _ => panic!("ErrorType {t}"),
    }
}

/// `ErrorType.X.severity` (`errors.dart:196-238` do `_fe_analyzer_shared`).
fn severidade_rust(s: &str) -> &'static str {
    match s {
        "ERROR" | "tipo:COMPILE_TIME_ERROR" | "tipo:SYNTACTIC_ERROR" | "tipo:CHECKED_MODE_COMPILE_TIME_ERROR" => {
            "Severidade::Error"
        }
        "WARNING" | "tipo:STATIC_WARNING" => "Severidade::Warning",
        "INFO" | "tipo:HINT" | "tipo:LINT" | "tipo:TODO" => "Severidade::Info",
        _ => panic!("severidade {s}"),
    }
}

fn main() {
    let raiz = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        let base = std::env::var_os("PUB_CACHE")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("LOCALAPPDATA").map(|l| Path::new(&l).join("Pub").join("Cache")))
            .expect("defina PUB_CACHE");
        base.join("hosted").join("pub.dev")
    });
    let mut todas: Vec<(Entrada, &str, String, String)> = Vec::new();
    let mut resumo = String::new();
    for (classe, modulo, arquivo) in CLASSES {
        let caminho = raiz.join(arquivo);
        let src = std::fs::read_to_string(&caminho).unwrap_or_else(|e| panic!("{}: {e}", caminho.display()));
        let (entradas, tipo, sev) = ler_classe(&src, classe);
        let _ = writeln!(resumo, "{classe}: {}", entradas.len());
        for e in entradas {
            todas.push((e, modulo, tipo.clone(), sev.clone()));
        }
    }
    for (classe, modulo, constante, nome, unico, mensagem, correcao, tipo) in SUPLEMENTO_3_13 {
        let e = Entrada {
            classe: classe.to_string(),
            constante: constante.to_string(),
            nome: nome.to_string(),
            unico: unico.to_string(),
            mensagem: mensagem.to_string(),
            correcao: (!correcao.is_empty()).then(|| correcao.to_string()),
            documentado: false,
        };
        todas.push((e, modulo, tipo.to_string(), format!("tipo:{tipo}")));
    }
    let _ = writeln!(resumo, "suplemento 3.13.4 (códigos novos e formas novas de códigos da 6.11): {}", SUPLEMENTO_3_13.len());
    // As variantes: nomes únicos viram índices; a ordenação é estável, e a
    // ordem das linhas de um mesmo código é a da tabela.
    let mut variantes: Vec<(usize, Option<usize>, &'static [u8], u8)> = Vec::new();
    {
        let indice = |unico: &str| todas.iter().position(|e| e.0.unico == unico).unwrap_or_else(|| panic!("variante: {unico} não está na tabela"));
        for (de, para, args, exige) in VARIANTES_3_13 {
            let minimo = args.iter().map(|a| *a + 1).max().unwrap_or(0).max(*exige);
            variantes.push((indice(*de), (!para.is_empty()).then(|| indice(*para)), *args, minimo));
        }
    }
    // A forma nova de um código de mesmo nome herda a documentação publicada.
    for (de, para, ..) in &variantes {
        if let Some(para) = para
            && para != de
            && todas[*para].0.nome == todas[*de].0.nome
        {
            todas[*para].0.documentado = todas[*de].0.documentado;
        }
    }
    variantes.sort_by_key(|v| v.0);
    let _ = writeln!(resumo, "variantes 3.13.4: {}", variantes.len());
    let n = todas.len();
    let mut s = String::new();
    s.push_str("// GERADO por `cargo run -p dartforge-paridade --example gerar_codigos`. NÃO EDITE.\n");
    s.push_str("// Fonte: analyzer-6.11.0 e _fe_analyzer_shared-76.0.0 (SDK Dart 3.6), cache do pub.\n");
    for l in resumo.lines() {
        let _ = writeln!(s, "// {l}");
    }
    s.push_str("#![allow(missing_docs)]\n\nuse crate::{Codigo, InfoCodigo, Severidade, TipoErro, Variante313};\n\n");
    let _ = writeln!(s, "pub(crate) static TABELA: [InfoCodigo; {n}] = [");
    for (e, _, tipo, sev) in &todas {
        let _ = writeln!(
            s,
            "    InfoCodigo {{ nome: {:?}, unico: {:?}, mensagem: {:?}, correcao: {}, tipo: {}, severidade: {}, documentado: {} }},",
            e.nome,
            e.unico,
            e.mensagem,
            match &e.correcao {
                Some(c) => format!("Some({c:?})"),
                None => "None".into(),
            },
            tipo_rust(tipo),
            severidade_rust(sev),
            e.documentado
        );
    }
    s.push_str("];\n\n");
    let mut por_unico: Vec<(&str, usize)> = todas.iter().enumerate().map(|(i, e)| (e.0.unico.as_str(), i)).collect();
    por_unico.sort();
    for w in por_unico.windows(2) {
        assert_ne!(w[0].0, w[1].0, "uniqueName repetido");
    }
    let _ = writeln!(s, "pub(crate) static POR_UNICO: [(&str, u16); {n}] = [");
    for (u, i) in &por_unico {
        let _ = writeln!(s, "    ({u:?}, {i}),");
    }
    s.push_str("];\n\n");
    let _ = writeln!(s, "/// Ordenada por `de`; ver `Codigo::variantes_3_13`.");
    let _ = writeln!(s, "pub(crate) static VARIANTES_3_13: [Variante313; {}] = [", variantes.len());
    for (de, para, args, exige) in &variantes {
        let para = match para {
            Some(p) => format!("Some(Codigo({p}))"),
            None => "None".to_string(),
        };
        let _ = writeln!(s, "    Variante313 {{ de: Codigo({de}), para: {para}, args: &{args:?}, exige: {exige} }},");
    }
    s.push_str("];\n\npub mod modulos {\n");
    for (classe, modulo, _) in CLASSES {
        let _ = writeln!(s, "    /// `{classe}`.\n    pub mod {modulo} {{\n        use crate::Codigo;");
        for (i, (e, m, _, _)) in todas.iter().enumerate() {
            if m == modulo {
                let _ = writeln!(s, "        pub const {}: Codigo = Codigo({i});", e.constante.to_uppercase());
            }
        }
        s.push_str("    }\n");
        let _ = &classe;
    }
    s.push_str("}\n");
    let destino = Path::new(env!("CARGO_MANIFEST_DIR")).join("../diagnostics/src/codigos_g.rs");
    std::fs::write(&destino, s).expect("gravar tabela");
    print!("{resumo}");
    println!("total: {n} -> {}", destino.display());
    for (e, ..) in &todas {
        debug_assert!(!e.classe.is_empty());
    }
}
