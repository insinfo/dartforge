//! Canonical diagnostic codes and identifiers aligned with the Dart SDK `analyzer`.
//!
//! Used for categorization, structured error messages and negative conformance
//! tests against the official compiler.

/// Canonical Dart Analyzer error code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiagnosticCode {
    pub name: &'static str,
    pub template: &'static str,
}

impl DiagnosticCode {
    pub const fn new(name: &'static str, template: &'static str) -> Self {
        Self { name, template }
    }
}

// Typing and assignment errors
pub const ARGUMENT_TYPE_NOT_ASSIGNABLE: DiagnosticCode = DiagnosticCode::new(
    "argument_type_not_assignable",
    "The argument type '{0}' can't be assigned to the parameter type '{1}'.",
);

pub const RETURN_OF_INVALID_TYPE: DiagnosticCode = DiagnosticCode::new(
    "return_of_invalid_type",
    "A value of type '{0}' can't be returned from a function with return type '{1}'.",
);

pub const INVALID_ASSIGNMENT: DiagnosticCode = DiagnosticCode::new(
    "invalid_assignment",
    "A value of type '{0}' can't be assigned to a variable of type '{1}'.",
);

pub const ASSIGNMENT_TO_FINAL_LOCAL: DiagnosticCode = DiagnosticCode::new(
    "assignment_to_final_local",
    "The final local variable '{0}' can only be assigned once.",
);

pub const ASSIGNMENT_TO_FINAL: DiagnosticCode = DiagnosticCode::new(
    "assignment_to_final",
    "The final variable has no setter.",
);

pub const ASSIGNMENT_TO_FINAL_NO_SETTER: DiagnosticCode = DiagnosticCode::new(
    "assignment_to_final_no_setter",
    "There is no setter for '{0}' in class '{1}'.",
);

pub const ASSIGNMENT_TO_METHOD: DiagnosticCode = DiagnosticCode::new(
    "assignment_to_method",
    "A value can't be assigned to a method.",
);

pub const UNDEFINED_EXTENSION_SETTER: DiagnosticCode = DiagnosticCode::new(
    "undefined_extension_setter",
    "Extension setter not found",
);

pub const UNDEFINED_EXTENSION_GETTER: DiagnosticCode = DiagnosticCode::new(
    "undefined_extension_getter",
    "Extension getter not found",
);

pub const UNDEFINED_EXTENSION_METHOD: DiagnosticCode = DiagnosticCode::new(
    "undefined_extension_method",
    "Extension method not found",
);

pub const STATIC_ACCESS_TO_INSTANCE_MEMBER: DiagnosticCode = DiagnosticCode::new(
    "static_access_to_instance_member",
    "Static access to an instance member",
);

pub const CLASS_INSTANTIATION_ACCESS_TO_INSTANCE_MEMBER: DiagnosticCode = DiagnosticCode::new(
    "class_instantiation_access_to_instance_member",
    "Instance member in a class instantiation",
);

pub const CLASS_INSTANTIATION_ACCESS_TO_STATIC_MEMBER: DiagnosticCode = DiagnosticCode::new(
    "class_instantiation_access_to_static_member",
    "Static member in a class instantiation",
);

pub const CLASS_INSTANTIATION_ACCESS_TO_UNKNOWN_MEMBER: DiagnosticCode = DiagnosticCode::new(
    "class_instantiation_access_to_unknown_member",
    "Unknown member in a class instantiation",
);

pub const NEW_WITH_UNDEFINED_CONSTRUCTOR: DiagnosticCode = DiagnosticCode::new(
    "new_with_undefined_constructor",
    "Named constructor not found",
);

pub const NEW_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT: DiagnosticCode = DiagnosticCode::new(
    "new_with_undefined_constructor_default",
    "Unnamed constructor not found",
);

pub const INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR: DiagnosticCode = DiagnosticCode::new(
    "invalid_reference_to_generative_enum_constructor",
    "Invalid reference to an enum generative constructor",
);

pub const INVOCATION_OF_EXTENSION_WITHOUT_CALL: DiagnosticCode = DiagnosticCode::new(
    "invocation_of_extension_without_call",
    "Extension invocation without call",
);

pub const UNDEFINED_EXTENSION_OPERATOR: DiagnosticCode = DiagnosticCode::new(
    "undefined_extension_operator",
    "Extension operator not found",
);

pub const EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER: DiagnosticCode = DiagnosticCode::new(
    "extension_override_access_to_static_member",
    "An extension override can't be used to access a static member from an extension.",
);

pub const ASSIGNMENT_TO_CONST: DiagnosticCode = DiagnosticCode::new(
    "assignment_to_const",
    "Constant variables can't be assigned a new value.",
);

pub const ASSIGNMENT_TO_TYPE: DiagnosticCode = DiagnosticCode::new(
    "assignment_to_type",
    "Types can't be assigned a value.",
);

pub const ASSIGNMENT_TO_FUNCTION: DiagnosticCode = DiagnosticCode::new(
    "assignment_to_function",
    "Functions can't be assigned a value.",
);

pub const NOT_INITIALIZED_NON_NULLABLE_VARIABLE: DiagnosticCode = DiagnosticCode::new(
    "not_initialized_non_nullable_variable",
    "The non-nullable variable '{0}' must be definitely assigned before it is used.",
);

pub const DEFINITELY_UNASSIGNED_VARIABLE: DiagnosticCode = DiagnosticCode::new(
    "definitely_unassigned_variable",
    "The variable '{0}' is read before it is assigned.",
);

// Member and identifier resolution errors
pub const UNDEFINED_IDENTIFIER: DiagnosticCode = DiagnosticCode::new(
    "undefined_identifier",
    "Nome indefinido '{0}'.",
);

pub const REFERENCED_BEFORE_DECLARATION: DiagnosticCode = DiagnosticCode::new(
    "referenced_before_declaration",
    "The local variable '{0}' can't be referenced before it is declared.",
);

pub const UNDEFINED_GETTER: DiagnosticCode = DiagnosticCode::new(
    "undefined_getter",
    "The getter '{0}' isn't defined for the type '{1}'.",
);

pub const UNDEFINED_SETTER: DiagnosticCode = DiagnosticCode::new(
    "undefined_setter",
    "The setter '{0}' isn't defined for the type '{1}'.",
);

pub const UNDEFINED_METHOD: DiagnosticCode = DiagnosticCode::new(
    "undefined_method",
    "The method '{0}' isn't defined for the type '{1}'.",
);

pub const UNDEFINED_OPERATOR: DiagnosticCode = DiagnosticCode::new(
    "undefined_operator",
    "The operator '{0}' isn't defined for the type '{1}'.",
);

pub const AMBIGUOUS_EXTENSION_MEMBER_ACCESS: DiagnosticCode = DiagnosticCode::new(
    "ambiguous_extension_member_access",
    "Ambiguous member '{0}' found in extensions '{1}' and '{2}'.",
);

// Flow and control errors
pub const AWAIT_IN_WRONG_CONTEXT: DiagnosticCode = DiagnosticCode::new(
    "await_in_wrong_context",
    "The 'await' expression can only be used in asynchronous functions.",
);

pub const YIELD_EACH_IN_NON_GENERATOR: DiagnosticCode = DiagnosticCode::new(
    "yield_each_in_non_generator",
    "The 'yield*' statement can only be used in generator functions.",
);

pub const NON_BOOL_CONDITION: DiagnosticCode = DiagnosticCode::new(
    "non_bool_condition",
    "Conditions must have a static type of 'bool'.",
);

pub const NON_BOOL_NEGATION_EXPRESSION: DiagnosticCode = DiagnosticCode::new(
    "non_bool_negation_expression",
    "A negated expression must have a static type of 'bool'.",
);

// Argument and parameter errors
pub const EXTRA_POSITIONAL_ARGUMENTS: DiagnosticCode = DiagnosticCode::new(
    "extra_positional_arguments",
    "Too many positional arguments: expected {0}, but got {1}.",
);

pub const NOT_ENOUGH_POSITIONAL_ARGUMENTS: DiagnosticCode = DiagnosticCode::new(
    "not_enough_positional_arguments",
    "Too few positional arguments: expected at least {0}, but got {1}.",
);

pub const MISSING_REQUIRED_ARGUMENT: DiagnosticCode = DiagnosticCode::new(
    "missing_required_argument",
    "The required named parameter '{0}' was not provided.",
);

pub const UNDEFINED_NAMED_PARAMETER: DiagnosticCode = DiagnosticCode::new(
    "undefined_named_parameter",
    "The named parameter '{0}' isn't defined for the function.",
);

pub const TYPE_ARGUMENT_NOT_MATCHING_BOUNDS: DiagnosticCode = DiagnosticCode::new(
    "type_argument_not_matching_bounds",
    "The type argument '{0}' doesn't extend the bound '{1}'.",
);

// Constant errors
pub const CONST_WITH_NON_CONSTANT_ARGUMENT: DiagnosticCode = DiagnosticCode::new(
    "const_with_non_constant_argument",
    "Arguments of a const instantiation must be constant expressions.",
);

pub const CONST_EVAL_THROWS_EXCEPTION: DiagnosticCode = DiagnosticCode::new(
    "const_eval_throws_exception",
    "Evaluation of the constant expression threw an exception: {0}.",
);

pub const EQUAL_ELEMENTS_IN_CONST_SET: DiagnosticCode = DiagnosticCode::new(
    "equal_elements_in_const_set",
    "Two elements in a constant set evaluate to the same value.",
);

pub const EQUAL_KEYS_IN_CONST_MAP: DiagnosticCode = DiagnosticCode::new(
    "equal_keys_in_const_map",
    "Two keys in a constant map evaluate to the same value.",
);

pub const CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE: DiagnosticCode = DiagnosticCode::new(
    "const_initialized_with_non_constant_value",
    "A const variable must be initialized with a constant value.",
);

// Dead code and unnecessary construct warnings
pub const INVALID_NULL_AWARE_OPERATOR: DiagnosticCode = DiagnosticCode::new(
    "invalid_null_aware_operator",
    "The null check operator is unnecessary because the receiver can't be null.",
);

/// `e!` with `e` strictly non-nullable (warning, at the `!`).
pub const UNNECESSARY_NON_NULL_ASSERTION: DiagnosticCode = DiagnosticCode::new(
    "unnecessary_non_null_assertion",
    "The '!' has no effect because the receiver can't be null.",
);

pub const UNNECESSARY_CAST: DiagnosticCode = DiagnosticCode::new(
    "unnecessary_cast",
    "The type cast is unnecessary because the value already has this type.",
);

pub const UNNECESSARY_TYPE_CHECK_TRUE: DiagnosticCode = DiagnosticCode::new(
    "unnecessary_type_check",
    "The type test always evaluates to true.",
);

pub const DEAD_CODE: DiagnosticCode = DiagnosticCode::new(
    "dead_code",
    "Unreachable code (dead code).",
);

// Language errors of the 3.7–3.13 features (docs/VERSOES-LINGUAGEM.md §3).
//
// The `types` diagnostics are warnings, because the inference still has
// gaps; these are not: they are what the CFE rejects in the new features and
// what a negative corpus program needs to see rejected. The message starts
// with [`LANGUAGE_ERROR`], and whoever compiles (`compile-js`,
// `dartforge-jsprod`, native) aborts as it aborts on load errors.

/// Prefix of language error messages.
pub const LANGUAGE_ERROR: &str = "language error: ";

/// Is the message a language error (and not a types warning)?
pub fn is_language_error(message: &str) -> bool {
    message.starts_with(LANGUAGE_ERROR)
}

/// The `types` diagnostic rejects the program: a language error
/// ([`is_language_error`]) or the read of a non-nullable (or `final`) local
/// that is not definitely assigned ([`DEFINITELY_UNASSIGNED_VARIABLE`]: the
/// analyzer's `NOT_ASSIGNED_POTENTIALLY_NON_NULLABLE_LOCAL_VARIABLE` /
/// `READ_POTENTIALLY_UNASSIGNED_FINAL`, which the CFE also rejects).
///
/// Definite assignment comes from the same flow model that promotes types
/// (`inference/flow.rs`), and it is what separates the programs the language
/// version accepts from those it rejects under sound flow (3.9,
/// docs/VERSOES-LINGUAGEM.md §4.6). The other diagnostics remain warnings.
///
/// ```
/// use dartforge_types::codes::{is_compile_error, DEFINITELY_UNASSIGNED_VARIABLE, LANGUAGE_ERROR};
/// assert!(is_compile_error(&format!("{}: 'y'", DEFINITELY_UNASSIGNED_VARIABLE.template)));
/// assert!(is_compile_error(&format!("{LANGUAGE_ERROR}a.dart:3: x")));
/// assert!(!is_compile_error("The type 'int' can't be assigned to 'String'."));
/// ```
pub fn is_compile_error(message: &str) -> bool {
    is_language_error(message)
        || message.starts_with(DEFINITELY_UNASSIGNED_VARIABLE.template)
        // `READ_POTENTIALLY_UNASSIGNED_FINAL` (with an analyzer code): reading a
        // local `final` that is possibly unassigned also aborts, as before.
        || (message.starts_with("The final variable '") && message.ends_with("because it's potentially unassigned at this point."))
}

pub const WILDCARD_DOES_NOT_BIND: DiagnosticCode = DiagnosticCode::new(
    "undefined_identifier",
    "no visible '_': in a 3.7+ library, locals and parameters named '_' are wildcards and do not bind a name",
);

pub const DOT_SHORTHAND_NO_CONTEXT: DiagnosticCode = DiagnosticCode::new(
    "dot_shorthand_missing_context",
    "the dot shorthand '.{0}' needs a context type that denotes a class, mixin, enum or extension type",
);

pub const DOT_SHORTHAND_NO_MEMBER: DiagnosticCode = DiagnosticCode::new(
    "dot_shorthand_undefined_member",
    "'{1}' has no static member or constructor '{0}' for the dot shorthand",
);

pub const PRIMARY_CONSTRUCTOR: DiagnosticCode = DiagnosticCode::new(
    "primary_constructor",
    "primary constructor: {0}",
);
