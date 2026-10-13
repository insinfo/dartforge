//! Generates `crates/diagnostics/src/codes_g.rs`: the code table of
//! `package:analyzer` 6.11.0 (the analyzer of SDK 3.6.2), read from the
//! sources in the pub cache.
//!
//! ```text
//! cargo run -p dartforge-parity --example generate_codes [-- <pub-cache/hosted/pub.dev>]
//! ```
//!
//! Deterministic: the same source gives the same file, byte for byte. The
//! sources are the `.g.dart` files generated from the analyzer's
//! `messages.yaml` (plus the two hand-written files, for the scanner and the
//! TODOs), read by a small Dart tokenizer:
//! `static const <Class> <NAME> = [const] <Class>(<args>);`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Analyzer class → Rust module, in table order.
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

/// Codes of the SDK 3.13.4 analyzer that the 6.11 table lacks (primary
/// constructors, dot shorthands) and the 3.13.4 shapes of codes it has.
/// Transcribed from the reference `messages.yaml`
/// (`references/dart-sdk/pkg/_fe_analyzer_shared` and `pkg/analyzer`) and
/// checked, text and type, against the 3.13.4 oracle recorded in
/// `corpus/diagnosticos`. They go at the end of the table: the indices of the
/// 6.11 codes do not change.
///
/// (class, module, constant, name, unique, message, correction, type)
const SUPPLEMENT_3_13: &[(&str, &str, &str, &str, &str, &str, &str, &str)] = &[
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
    // Dot shorthands (3.10), from the `ResolverVisitor` of analyzer 3.13.4;
    // the missing-context one has no correction.
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
    // Codes only 3.13.4 has (docs/ANALYZER-ESPECIFICACAO.md, T2 §1.3 (c)); texts
    // checked against the 3.13.4 oracle recorded in `corpus/diagnosticos`.
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
    // `const` in the header without a primary constructor
    // (`parser_impl.dart:3829-3856` of 3.13.4, with the feature enabled); texts
    // from the tag's `pkg/_fe_analyzer_shared/messages.yaml`.
    (
        "ParserErrorCode",
        "parser",
        "CONST_WITHOUT_PRIMARY_CONSTRUCTOR",
        "const_without_primary_constructor",
        "ParserErrorCode.CONST_WITHOUT_PRIMARY_CONSTRUCTOR",
        "'const' can only be used together with a primary constructor declaration.",
        "Try removing the 'const' keyword or adding a primary constructor declaration.",
        "SYNTACTIC_ERROR",
    ),
    (
        "ParserErrorCode",
        "parser",
        "MIXIN_PRIMARY_CONSTRUCTOR",
        "mixin_primary_constructor",
        "ParserErrorCode.MIXIN_PRIMARY_CONSTRUCTOR",
        "Mixins can't have primary constructors.",
        "Try removing the primary constructor or changing the mixin to a class.",
        "SYNTACTIC_ERROR",
    ),
    (
        "ParserErrorCode",
        "parser",
        "EXTENSION_PRIMARY_CONSTRUCTOR",
        "extension_primary_constructor",
        "ParserErrorCode.EXTENSION_PRIMARY_CONSTRUCTOR",
        "Extensions can't have primary constructors.",
        "Try removing the primary constructor or changing the extension to an extension type.",
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
    // The 3.13.4 shapes of codes 6.11 already has (T2 §1.3 (a) and (b)): the targets of
    // `VARIANTS_3_13`. The `_3_13` suffix only distinguishes the unique name; the reported
    // name is the next column. Texts from the live 3.13.4 oracle (cases `corpus/especificacao/t2/v`).
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
    // The statement variant with private constants was not run on the live oracle: text from
    // main's `messages.yaml` (the expression one was, case c29).
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

/// How a 6.11 code is reported when the file's reference is analyzer 3.13.4
/// (docs/ANALYZER-ESPECIFICACAO.md, T2 §3). The emitter passes the arguments
/// of the 3.6 template and, after them, the ones only 3.13 uses; the third
/// column says which go, and in what order, to the target template. The
/// fourth is the minimum number of arguments the diagnostic must carry for
/// the row to apply (0: what the indices require); with several rows for the
/// same code, the first that fits applies, and if none fits the diagnostic is
/// reported as in 3.6. Empty target: 3.13.4 does not report it.
///
/// (from, to, arguments, minimum)
const VARIANTS_3_13: &[(&str, &str, &[u8], u8)] = &[
    // `{1}` of the correction: `[feature, version 3.6.2, version 3.13.4]`.
    ("ParserErrorCode.EXPERIMENT_NOT_ENABLED", "ParserErrorCode.EXPERIMENT_NOT_ENABLED", &[0, 2], 0),
    ("ParserErrorCode.REPRESENTATION_FIELD_MODIFIER", "ParserErrorCode.REPRESENTATION_FIELD_MODIFIER_3_13", &[0], 0),
    ("WarningCode.UNUSED_ELEMENT_PARAMETER", "WarningCode.UNUSED_ELEMENT_PARAMETER_3_13", &[0], 0),
    ("CompileTimeErrorCode.ENUM_WITHOUT_CONSTANTS", "CompileTimeErrorCode.ENUM_WITHOUT_CONSTANTS_3_13", &[], 0),
    ("CompileTimeErrorCode.ASSIGNMENT_TO_CONST", "CompileTimeErrorCode.ASSIGNMENT_TO_CONST_3_13", &[], 0),
    ("CompileTimeErrorCode.NULLABLE_TYPE_IN_IMPLEMENTS_CLAUSE", "CompileTimeErrorCode.NULLABLE_TYPE_IN_IMPLEMENTS_CLAUSE_3_13", &[], 0),
    ("CompileTimeErrorCode.SUPER_INVOCATION_NOT_LAST", "CompileTimeErrorCode.SUPER_INVOCATION_NOT_LAST_3_13", &[], 0),
    // `[constructor's class, superclass]`.
    ("CompileTimeErrorCode.CONST_CONSTRUCTOR_WITH_NON_CONST_SUPER", "CompileTimeErrorCode.CONST_CONSTRUCTOR_WITH_NON_CONST_SUPER", &[1], 0),
    // `[type, pattern, suggestion]` and, when the enum is from another library
    // and what is missing are private constants, any fourth argument.
    ("CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_EXPRESSION", "CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_EXPRESSION_PRIVATE", &[0], 4),
    ("CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_EXPRESSION", "CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_EXPRESSION_3_13", &[0, 1, 2], 0),
    ("CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_STATEMENT", "CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_STATEMENT_PRIVATE", &[0], 4),
    ("CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_STATEMENT", "CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_STATEMENT_3_13", &[0, 1, 2], 0),
    // `[target type, declared, given]` and, when the target is an element,
    // `kind` (`method`, `function`) and name.
    ("CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_METHOD", "CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_ELEMENT", &[3, 4, 1, 2], 0),
    ("CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_METHOD", "CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION_3_13", &[0, 1, 2], 0),
    // The anonymous variant: `[declared, given, function type]` (the type only in 3.13.4).
    ("CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_ANONYMOUS_FUNCTION", "CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION_3_13", &[2, 0, 1], 3),
    // `[member, 3.6 list]` and, only when there are exactly two, the two
    // extensions as 3.13 displays them (`extension E1 on int`).
    ("CompileTimeErrorCode.AMBIGUOUS_EXTENSION_MEMBER_ACCESS", "CompileTimeErrorCode.AMBIGUOUS_EXTENSION_MEMBER_ACCESS_TWO", &[0, 2, 3], 0),
    // `[class, 3.6 path, 3.13 path]`.
    ("CompileTimeErrorCode.RECURSIVE_INTERFACE_INHERITANCE", "CompileTimeErrorCode.RECURSIVE_INTERFACE_INHERITANCE", &[0, 2], 0),
    // The verifier's variant: in 3.13.4 only the parser's is reported.
    ("CompileTimeErrorCode.FIELD_INITIALIZER_OUTSIDE_CONSTRUCTOR", "", &[], 0),
    ("CompileTimeErrorCode.MIXIN_CLASS_DECLARES_CONSTRUCTOR", "CompileTimeErrorCode.MIXIN_CLASS_DECLARES_NON_TRIVIAL_GENERATIVE_CONSTRUCTOR", &[0], 0),
    ("CompileTimeErrorCode.DEFERRED_IMPORT_OF_EXTENSION", "CompileTimeErrorCode.DEFERRED_IMPORT_OF_EXTENSION_3_13", &[], 0),
];

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Id(String),
    Str(String),
    P(char),
    Arrow,
}

fn tokenize(src: &str) -> Vec<Tok> {
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
            let mut depth = 0;
            while i < b.len() {
                if b[i..].starts_with(b"/*") {
                    depth += 1;
                    i += 2;
                } else if b[i..].starts_with(b"*/") {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else if c == b'\'' || c == b'"' || (c == b'r' && matches!(b.get(i + 1), Some(b'\'') | Some(b'"'))) {
            let raw = c == b'r';
            if raw {
                i += 1;
            }
            let q = b[i];
            let triple = b[i..].starts_with(&[q, q, q]);
            let end: Vec<u8> = if triple { vec![q, q, q] } else { vec![q] };
            i += end.len();
            let mut s = Vec::new();
            while i < b.len() && !b[i..].starts_with(&end) {
                if b[i] == b'\\' && !raw {
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
            i += end.len();
            out.push(Tok::Str(String::from_utf8(s).expect("UTF-8")));
        } else if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
                i += 1;
            }
            out.push(Tok::Id(src[start..i].to_string()));
        } else if b[i..].starts_with(b"=>") {
            out.push(Tok::Arrow);
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
struct Entry {
    class: String,
    constant: String,
    name: String,
    unique: String,
    message: String,
    correction: Option<String>,
    documented: bool,
}

/// The arguments: name (if named) and the value's tokens.
fn arguments(toks: &[Tok]) -> Vec<(Option<String>, Vec<Tok>)> {
    let mut args = Vec::new();
    let mut current: Vec<Tok> = Vec::new();
    let mut depth = 0i32;
    for t in toks {
        match t {
            Tok::P('(') | Tok::P('[') | Tok::P('{') => depth += 1,
            Tok::P(')') | Tok::P(']') | Tok::P('}') => depth -= 1,
            _ => {}
        }
        if depth == 0 && *t == Tok::P(',') {
            args.push(std::mem::take(&mut current));
        } else {
            current.push(t.clone());
        }
    }
    if !current.is_empty() {
        args.push(current);
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

fn text(v: &[Tok]) -> Option<String> {
    let mut s = String::new();
    let mut any = false;
    for t in v {
        match t {
            Tok::Str(x) => {
                s.push_str(x);
                any = true;
            }
            _ => return None,
        }
    }
    any.then_some(s)
}

/// Reads the entries and the (type, severity) pair of a class in the file.
fn read_class(src: &str, class: &str) -> (Vec<Entry>, String, String) {
    let t = tokenize(src);
    let mut i = 0;
    // `class <class> extends ... {`
    while i + 1 < t.len() && !(t[i] == Tok::Id("class".into()) && t[i + 1] == Tok::Id(class.into())) {
        i += 1;
    }
    assert!(i + 1 < t.len(), "class {class} not found");
    while t[i] != Tok::P('{') {
        i += 1;
    }
    i += 1;
    let mut depth = 1;
    let mut entries = Vec::new();
    let mut ty = None;
    let mut severity = None;
    while i < t.len() && depth > 0 {
        match &t[i] {
            Tok::P('{') => depth += 1,
            Tok::P('}') => depth -= 1,
            Tok::Id(s) if depth == 1 && s == "static" && t.get(i + 1) == Some(&Tok::Id("const".into())) => {
                // static const <Class> <NAME> = [const] <Class>( ... ) ;
                let Some(Tok::Id(decl_type)) = t.get(i + 2) else { panic!("static const without a type") };
                let Some(Tok::Id(constant)) = t.get(i + 3) else { panic!("static const without a name") };
                if decl_type != class {
                    i += 1;
                    continue;
                }
                let mut j = i + 4;
                assert_eq!(t[j], Tok::P('='), "{constant}");
                j += 1;
                if t[j] == Tok::Id("const".into()) {
                    j += 1;
                }
                assert_eq!(t[j], Tok::Id(class.into()), "{constant}");
                j += 1;
                assert_eq!(t[j], Tok::P('('), "{constant}");
                let start = j + 1;
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
                let args = arguments(&t[start..j - 1]);
                let pos: Vec<String> = args.iter().filter(|a| a.0.is_none()).map(|a| text(&a.1).expect("string")).collect();
                let named = |n: &str| args.iter().find(|a| a.0.as_deref() == Some(n)).map(|a| &a.1);
                let name = pos[0].clone();
                let message = if class == "TodoCode" { "{0}".to_string() } else { pos[1].clone() };
                let short_unique = named("uniqueName").and_then(|v| text(v)).unwrap_or_else(|| name.clone());
                entries.push(Entry {
                    class: class.to_string(),
                    constant: constant.clone(),
                    name: name.to_lowercase(),
                    unique: format!("{class}.{short_unique}"),
                    message,
                    correction: named("correctionMessage").and_then(|v| text(v)),
                    documented: named("hasPublishedDocs").is_some_and(|v| v.as_slice() == [Tok::Id("true".into())]),
                });
                i = j;
                continue;
            }
            Tok::Id(s) if depth == 1 && (s == "ErrorSeverity" || s == "ErrorType") && t.get(i + 1) == Some(&Tok::Id("get".into())) => {
                let getter = match &t[i + 2] {
                    Tok::Id(g) => g.clone(),
                    _ => String::new(),
                };
                assert_eq!(t[i + 3], Tok::Arrow);
                // `ErrorType.X.severity`, `ErrorSeverity.X` or `ErrorType.X`.
                let mut j = i + 4;
                let mut expr = Vec::new();
                while t[j] != Tok::P(';') {
                    if let Tok::Id(x) = &t[j] {
                        expr.push(x.clone());
                    }
                    j += 1;
                }
                let value = match expr.as_slice() {
                    [a, x, s] if a == "ErrorType" && s == "severity" => format!("type:{x}"),
                    [a, x] if a == "ErrorSeverity" || a == "ErrorType" => x.clone(),
                    _ => panic!("getter {getter} of {class} not recognized: {expr:?}"),
                };
                if getter == "errorSeverity" {
                    severity = Some(value);
                } else if getter == "type" {
                    ty = Some(value);
                }
                i = j;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    (entries, ty.expect("getter type"), severity.expect("getter errorSeverity"))
}

fn rust_type(t: &str) -> &'static str {
    match t {
        "TODO" => "ErrorType::Todo",
        "HINT" => "ErrorType::Hint",
        "COMPILE_TIME_ERROR" => "ErrorType::CompileTimeError",
        "CHECKED_MODE_COMPILE_TIME_ERROR" => "ErrorType::CheckedModeCompileTimeError",
        "STATIC_WARNING" => "ErrorType::StaticWarning",
        "SYNTACTIC_ERROR" => "ErrorType::SyntacticError",
        "LINT" => "ErrorType::Lint",
        _ => panic!("ErrorType {t}"),
    }
}

/// `ErrorType.X.severity` (`errors.dart:196-238` of `_fe_analyzer_shared`).
fn rust_severity(s: &str) -> &'static str {
    match s {
        "ERROR" | "type:COMPILE_TIME_ERROR" | "type:SYNTACTIC_ERROR" | "type:CHECKED_MODE_COMPILE_TIME_ERROR" => {
            "Severity::Error"
        }
        "WARNING" | "type:STATIC_WARNING" => "Severity::Warning",
        "INFO" | "type:HINT" | "type:LINT" | "type:TODO" => "Severity::Info",
        _ => panic!("severity {s}"),
    }
}

fn main() {
    let root = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        let base = std::env::var_os("PUB_CACHE")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("LOCALAPPDATA").map(|l| Path::new(&l).join("Pub").join("Cache")))
            .expect("set PUB_CACHE");
        base.join("hosted").join("pub.dev")
    });
    let mut all: Vec<(Entry, &str, String, String)> = Vec::new();
    let mut summary = String::new();
    for (class, module, file) in CLASSES {
        let file_path = root.join(file);
        let src = std::fs::read_to_string(&file_path).unwrap_or_else(|e| panic!("{}: {e}", file_path.display()));
        let (entries, ty, sev) = read_class(&src, class);
        let _ = writeln!(summary, "{class}: {}", entries.len());
        for e in entries {
            all.push((e, module, ty.clone(), sev.clone()));
        }
    }
    for (class, module, constant, name, unique, message, correction, ty) in SUPPLEMENT_3_13 {
        let e = Entry {
            class: class.to_string(),
            constant: constant.to_string(),
            name: name.to_string(),
            unique: unique.to_string(),
            message: message.to_string(),
            correction: (!correction.is_empty()).then(|| correction.to_string()),
            documented: false,
        };
        all.push((e, module, ty.to_string(), format!("type:{ty}")));
    }
    let _ = writeln!(summary, "3.13.4 supplement (new codes and new shapes of 6.11 codes): {}", SUPPLEMENT_3_13.len());
    // The variants: unique names become indices; the sort is stable, and the
    // order of the rows of one code is the table's.
    let mut variants: Vec<(usize, Option<usize>, &'static [u8], u8)> = Vec::new();
    {
        let index = |unique: &str| all.iter().position(|e| e.0.unique == unique).unwrap_or_else(|| panic!("variant: {unique} is not in the table"));
        for (from_code, to, args, requires) in VARIANTS_3_13 {
            let minimum = args.iter().map(|a| *a + 1).max().unwrap_or(0).max(*requires);
            variants.push((index(*from_code), (!to.is_empty()).then(|| index(*to)), *args, minimum));
        }
    }
    // The new shape of a code with the same name inherits the published documentation.
    for (from_code, to, ..) in &variants {
        if let Some(to) = to
            && to != from_code
            && all[*to].0.name == all[*from_code].0.name
        {
            all[*to].0.documented = all[*from_code].0.documented;
        }
    }
    variants.sort_by_key(|v| v.0);
    let _ = writeln!(summary, "3.13.4 variants: {}", variants.len());
    let n = all.len();
    let mut s = String::new();
    s.push_str("// GENERATED by `cargo run -p dartforge-parity --example generate_codes`. DO NOT EDIT.\n");
    s.push_str("// Source: analyzer-6.11.0 and _fe_analyzer_shared-76.0.0 (Dart SDK 3.6), pub cache.\n");
    for l in summary.lines() {
        let _ = writeln!(s, "// {l}");
    }
    s.push_str("#![allow(missing_docs)]\n\nuse crate::{Code, CodeInfo, Severity, ErrorType, Variant313};\n\n");
    let _ = writeln!(s, "pub(crate) static TABLE: [CodeInfo; {n}] = [");
    for (e, _, ty, sev) in &all {
        let _ = writeln!(
            s,
            "    CodeInfo {{ name: {:?}, unique: {:?}, message: {:?}, correction: {}, error_type: {}, severity: {}, documented: {} }},",
            e.name,
            e.unique,
            e.message,
            match &e.correction {
                Some(c) => format!("Some({c:?})"),
                None => "None".into(),
            },
            rust_type(ty),
            rust_severity(sev),
            e.documented
        );
    }
    s.push_str("];\n\n");
    let mut by_unique: Vec<(&str, usize)> = all.iter().enumerate().map(|(i, e)| (e.0.unique.as_str(), i)).collect();
    by_unique.sort();
    for w in by_unique.windows(2) {
        assert_ne!(w[0].0, w[1].0, "repeated uniqueName");
    }
    let _ = writeln!(s, "pub(crate) static BY_UNIQUE: [(&str, u16); {n}] = [");
    for (u, i) in &by_unique {
        let _ = writeln!(s, "    ({u:?}, {i}),");
    }
    s.push_str("];\n\n");
    let _ = writeln!(s, "/// Sorted by `from`; see `Code::variants_3_13`.");
    let _ = writeln!(s, "pub(crate) static VARIANTS_3_13: [Variant313; {}] = [", variants.len());
    for (from_code, to, args, requires) in &variants {
        let to = match to {
            Some(p) => format!("Some(Code({p}))"),
            None => "None".to_string(),
        };
        let _ = writeln!(s, "    Variant313 {{ from: Code({from_code}), to: {to}, args: &{args:?}, requires: {requires} }},");
    }
    s.push_str("];\n\npub mod modules {\n");
    for (class, module, _) in CLASSES {
        let _ = writeln!(s, "    /// `{class}`.\n    pub mod {module} {{\n        use crate::Code;");
        for (i, (e, m, _, _)) in all.iter().enumerate() {
            if m == module {
                let _ = writeln!(s, "        pub const {}: Code = Code({i});", e.constant.to_uppercase());
            }
        }
        s.push_str("    }\n");
        let _ = &class;
    }
    s.push_str("}\n");
    let dest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../diagnostics/src/codes_g.rs");
    std::fs::write(&dest, s).expect("write table");
    print!("{summary}");
    println!("total: {n} -> {}", dest.display());
    for (e, ..) in &all {
        debug_assert!(!e.class.is_empty());
    }
}
