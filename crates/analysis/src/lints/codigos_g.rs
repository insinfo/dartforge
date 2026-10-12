// GERADO por `scripts/gerar-tabelas-analise.py` (frente INFRA-FASES). NÃO EDITE.
// Fonte: `linter/lib/src/linter_lint_codes.dart` do SDK 3.6.2 (LinterLintCode).
// TODO(catálogo): os `LintCode` não estão em `crates/diagnostics/src/codigos_g.rs`.
#![allow(missing_docs)]

use super::CodigoLint;

pub static ALWAYS_DECLARE_RETURN_TYPES_OF_FUNCTIONS: CodigoLint = CodigoLint { nome: "always_declare_return_types", unico: "always_declare_return_types_of_functions", mensagem: "The function '{0}' should have a return type but doesn't.", correcao: Some("Try adding a return type to the function."), documentado: true };
pub static ALWAYS_DECLARE_RETURN_TYPES_OF_METHODS: CodigoLint = CodigoLint { nome: "always_declare_return_types", unico: "always_declare_return_types_of_methods", mensagem: "The method '{0}' should have a return type but doesn't.", correcao: Some("Try adding a return type to the method."), documentado: true };
pub static ALWAYS_PUT_CONTROL_BODY_ON_NEW_LINE: CodigoLint = CodigoLint { nome: "always_put_control_body_on_new_line", unico: "always_put_control_body_on_new_line", mensagem: "Statement should be on a separate line.", correcao: Some("Try moving the statement to a new line."), documentado: true };
pub static ALWAYS_PUT_REQUIRED_NAMED_PARAMETERS_FIRST: CodigoLint = CodigoLint { nome: "always_put_required_named_parameters_first", unico: "always_put_required_named_parameters_first", mensagem: "Required named parameters should be before optional named parameters.", correcao: Some("Try moving the required named parameter to be before any optional named parameters."), documentado: true };
pub static ALWAYS_SPECIFY_TYPES_ADD_TYPE: CodigoLint = CodigoLint { nome: "always_specify_types", unico: "always_specify_types_add_type", mensagem: "Missing type annotation.", correcao: Some("Try adding a type annotation."), documentado: false };
pub static ALWAYS_SPECIFY_TYPES_REPLACE_KEYWORD: CodigoLint = CodigoLint { nome: "always_specify_types", unico: "always_specify_types_replace_keyword", mensagem: "Missing type annotation.", correcao: Some("Try replacing '{0}' with '{1}'."), documentado: false };
pub static ALWAYS_SPECIFY_TYPES_SPECIFY_TYPE: CodigoLint = CodigoLint { nome: "always_specify_types", unico: "always_specify_types_specify_type", mensagem: "Missing type annotation.", correcao: Some("Try specifying the type '{0}'."), documentado: false };
pub static ALWAYS_SPECIFY_TYPES_SPLIT_TO_TYPES: CodigoLint = CodigoLint { nome: "always_specify_types", unico: "always_specify_types_split_to_types", mensagem: "Missing type annotation.", correcao: Some("Try splitting the declaration and specify the different type annotations."), documentado: false };
pub static ALWAYS_USE_PACKAGE_IMPORTS: CodigoLint = CodigoLint { nome: "always_use_package_imports", unico: "always_use_package_imports", mensagem: "Use 'package:' imports for files in the 'lib' directory.", correcao: Some("Try converting the URI to a 'package:' URI."), documentado: true };
pub static ANNOTATE_OVERRIDES: CodigoLint = CodigoLint { nome: "annotate_overrides", unico: "annotate_overrides", mensagem: "The member '{0}' overrides an inherited member but isn't annotated with '@override'.", correcao: Some("Try adding the '@override' annotation."), documentado: true };
pub static ANNOTATE_REDECLARES: CodigoLint = CodigoLint { nome: "annotate_redeclares", unico: "annotate_redeclares", mensagem: "The member '{0}' is redeclaring but isn't annotated with '@redeclare'.", correcao: Some("Try adding the '@redeclare' annotation."), documentado: false };
pub static AVOID_ANNOTATING_WITH_DYNAMIC: CodigoLint = CodigoLint { nome: "avoid_annotating_with_dynamic", unico: "avoid_annotating_with_dynamic", mensagem: "Unnecessary 'dynamic' type annotation.", correcao: Some("Try removing the type 'dynamic'."), documentado: false };
pub static AVOID_BOOL_LITERALS_IN_CONDITIONAL_EXPRESSIONS: CodigoLint = CodigoLint { nome: "avoid_bool_literals_in_conditional_expressions", unico: "avoid_bool_literals_in_conditional_expressions", mensagem: "Conditional expressions with a 'bool' literal can be simplified.", correcao: Some("Try rewriting the expression to use either '&&' or '||'."), documentado: false };
pub static AVOID_CATCHES_WITHOUT_ON_CLAUSES: CodigoLint = CodigoLint { nome: "avoid_catches_without_on_clauses", unico: "avoid_catches_without_on_clauses", mensagem: "Catch clause should use 'on' to specify the type of exception being caught.", correcao: Some("Try adding an 'on' clause before the 'catch'."), documentado: false };
pub static AVOID_CATCHING_ERRORS_CLASS: CodigoLint = CodigoLint { nome: "avoid_catching_errors", unico: "avoid_catching_errors_class", mensagem: "The type 'Error' should not be caught.", correcao: Some("Try removing the catch or catching an 'Exception' instead."), documentado: false };
pub static AVOID_CATCHING_ERRORS_SUBCLASS: CodigoLint = CodigoLint { nome: "avoid_catching_errors", unico: "avoid_catching_errors_subclass", mensagem: "The type '{0}' should not be caught because it is a subclass of 'Error'.", correcao: Some("Try removing the catch or catching an 'Exception' instead."), documentado: false };
pub static AVOID_CLASSES_WITH_ONLY_STATIC_MEMBERS: CodigoLint = CodigoLint { nome: "avoid_classes_with_only_static_members", unico: "avoid_classes_with_only_static_members", mensagem: "Classes should define instance members.", correcao: Some("Try adding instance behavior or moving the members out of the class."), documentado: false };
pub static AVOID_DOUBLE_AND_INT_CHECKS: CodigoLint = CodigoLint { nome: "avoid_double_and_int_checks", unico: "avoid_double_and_int_checks", mensagem: "Explicit check for double or int.", correcao: Some("Try removing the check."), documentado: false };
pub static AVOID_DYNAMIC_CALLS: CodigoLint = CodigoLint { nome: "avoid_dynamic_calls", unico: "avoid_dynamic_calls", mensagem: "Method invocation or property access on a 'dynamic' target.", correcao: Some("Try giving the target a type."), documentado: false };
pub static AVOID_EMPTY_ELSE: CodigoLint = CodigoLint { nome: "avoid_empty_else", unico: "avoid_empty_else", mensagem: "Empty statements are not allowed in an 'else' clause.", correcao: Some("Try removing the empty statement or removing the else clause."), documentado: true };
pub static AVOID_EQUALS_AND_HASH_CODE_ON_MUTABLE_CLASSES: CodigoLint = CodigoLint { nome: "avoid_equals_and_hash_code_on_mutable_classes", unico: "avoid_equals_and_hash_code_on_mutable_classes", mensagem: "The method '{0}' should not be overridden in classes not annotated with '@immutable'.", correcao: Some("Try removing the override or annotating the class with '@immutable'."), documentado: false };
pub static AVOID_ESCAPING_INNER_QUOTES: CodigoLint = CodigoLint { nome: "avoid_escaping_inner_quotes", unico: "avoid_escaping_inner_quotes", mensagem: "Unnecessary escape of '{0}'.", correcao: Some("Try changing the outer quotes to '{1}'."), documentado: false };
pub static AVOID_FIELD_INITIALIZERS_IN_CONST_CLASSES: CodigoLint = CodigoLint { nome: "avoid_field_initializers_in_const_classes", unico: "avoid_field_initializers_in_const_classes", mensagem: "Fields in 'const' classes should not have initializers.", correcao: Some("Try converting the field to a getter or initialize the field in the constructors."), documentado: false };
pub static AVOID_FINAL_PARAMETERS: CodigoLint = CodigoLint { nome: "avoid_final_parameters", unico: "avoid_final_parameters", mensagem: "Parameters should not be marked as 'final'.", correcao: Some("Try removing the keyword 'final'."), documentado: false };
pub static AVOID_FUNCTION_LITERALS_IN_FOREACH_CALLS: CodigoLint = CodigoLint { nome: "avoid_function_literals_in_foreach_calls", unico: "avoid_function_literals_in_foreach_calls", mensagem: "Function literals shouldn't be passed to 'forEach'.", correcao: Some("Try using a 'for' loop."), documentado: true };
pub static AVOID_FUTUREOR_VOID: CodigoLint = CodigoLint { nome: "avoid_futureor_void", unico: "avoid_futureor_void", mensagem: "Don't use the type 'FutureOr<void>'.", correcao: Some("Try using 'Future<void>?' or 'void'."), documentado: true };
pub static AVOID_IMPLEMENTING_VALUE_TYPES: CodigoLint = CodigoLint { nome: "avoid_implementing_value_types", unico: "avoid_implementing_value_types", mensagem: "Classes that override '==' should not be implemented.", correcao: Some("Try removing the class from the 'implements' clause."), documentado: false };
pub static AVOID_INIT_TO_NULL: CodigoLint = CodigoLint { nome: "avoid_init_to_null", unico: "avoid_init_to_null", mensagem: "Redundant initialization to 'null'.", correcao: Some("Try removing the initializer."), documentado: true };
pub static AVOID_JS_ROUNDED_INTS: CodigoLint = CodigoLint { nome: "avoid_js_rounded_ints", unico: "avoid_js_rounded_ints", mensagem: "Integer literal can't be represented exactly when compiled to JavaScript.", correcao: Some("Try using a 'BigInt' to represent the value."), documentado: false };
pub static AVOID_MULTIPLE_DECLARATIONS_PER_LINE: CodigoLint = CodigoLint { nome: "avoid_multiple_declarations_per_line", unico: "avoid_multiple_declarations_per_line", mensagem: "Multiple variables declared on a single line.", correcao: Some("Try splitting the variable declarations into multiple lines."), documentado: false };
pub static AVOID_NULL_CHECKS_IN_EQUALITY_OPERATORS: CodigoLint = CodigoLint { nome: "avoid_null_checks_in_equality_operators", unico: "avoid_null_checks_in_equality_operators", mensagem: "Unnecessary null comparison in implementation of '=='.", correcao: Some("Try removing the comparison."), documentado: false };
pub static AVOID_POSITIONAL_BOOLEAN_PARAMETERS: CodigoLint = CodigoLint { nome: "avoid_positional_boolean_parameters", unico: "avoid_positional_boolean_parameters", mensagem: "'bool' parameters should be named parameters.", correcao: Some("Try converting the parameter to a named parameter."), documentado: false };
pub static AVOID_PRINT: CodigoLint = CodigoLint { nome: "avoid_print", unico: "avoid_print", mensagem: "Don't invoke 'print' in production code.", correcao: Some("Try using a logging framework."), documentado: true };
pub static AVOID_PRIVATE_TYPEDEF_FUNCTIONS: CodigoLint = CodigoLint { nome: "avoid_private_typedef_functions", unico: "avoid_private_typedef_functions", mensagem: "The typedef is unnecessary because it is only used in one place.", correcao: Some("Try inlining the type or using it in other places."), documentado: false };
pub static AVOID_REDUNDANT_ARGUMENT_VALUES: CodigoLint = CodigoLint { nome: "avoid_redundant_argument_values", unico: "avoid_redundant_argument_values", mensagem: "The value of the argument is redundant because it matches the default value.", correcao: Some("Try removing the argument."), documentado: false };
pub static AVOID_RELATIVE_LIB_IMPORTS: CodigoLint = CodigoLint { nome: "avoid_relative_lib_imports", unico: "avoid_relative_lib_imports", mensagem: "Can't use a relative path to import a library in 'lib'.", correcao: Some("Try fixing the relative path or changing the import to a 'package:' import."), documentado: true };
pub static AVOID_RENAMING_METHOD_PARAMETERS: CodigoLint = CodigoLint { nome: "avoid_renaming_method_parameters", unico: "avoid_renaming_method_parameters", mensagem: "The parameter name '{0}' doesn't match the name '{1}' in the overridden method.", correcao: Some("Try changing the name to '{1}'."), documentado: true };
pub static AVOID_RETURN_TYPES_ON_SETTERS: CodigoLint = CodigoLint { nome: "avoid_return_types_on_setters", unico: "avoid_return_types_on_setters", mensagem: "Unnecessary return type on a setter.", correcao: Some("Try removing the return type."), documentado: true };
pub static AVOID_RETURNING_NULL_FOR_VOID_FROM_FUNCTION: CodigoLint = CodigoLint { nome: "avoid_returning_null_for_void", unico: "avoid_returning_null_for_void_from_function", mensagem: "Don't return 'null' from a function with a return type of 'void'.", correcao: Some("Try removing the 'null'."), documentado: true };
pub static AVOID_RETURNING_NULL_FOR_VOID_FROM_METHOD: CodigoLint = CodigoLint { nome: "avoid_returning_null_for_void", unico: "avoid_returning_null_for_void_from_method", mensagem: "Don't return 'null' from a method with a return type of 'void'.", correcao: Some("Try removing the 'null'."), documentado: true };
pub static AVOID_RETURNING_THIS: CodigoLint = CodigoLint { nome: "avoid_returning_this", unico: "avoid_returning_this", mensagem: "Don't return 'this' from a method.", correcao: Some("Try changing the return type to 'void' and removing the return."), documentado: false };
pub static AVOID_SETTERS_WITHOUT_GETTERS: CodigoLint = CodigoLint { nome: "avoid_setters_without_getters", unico: "avoid_setters_without_getters", mensagem: "Setter has no corresponding getter.", correcao: Some("Try adding a corresponding getter or removing the setter."), documentado: false };
pub static AVOID_SHADOWING_TYPE_PARAMETERS: CodigoLint = CodigoLint { nome: "avoid_shadowing_type_parameters", unico: "avoid_shadowing_type_parameters", mensagem: "The type parameter '{0}' shadows a type parameter from the enclosing {1}.", correcao: Some("Try renaming one of the type parameters."), documentado: true };
pub static AVOID_SINGLE_CASCADE_IN_EXPRESSION_STATEMENTS: CodigoLint = CodigoLint { nome: "avoid_single_cascade_in_expression_statements", unico: "avoid_single_cascade_in_expression_statements", mensagem: "Unnecessary cascade expression.", correcao: Some("Try using the operator '{0}'."), documentado: true };
pub static AVOID_SLOW_ASYNC_IO: CodigoLint = CodigoLint { nome: "avoid_slow_async_io", unico: "avoid_slow_async_io", mensagem: "Use of an async 'dart:io' method.", correcao: Some("Try using the synchronous version of the method."), documentado: true };
pub static AVOID_TYPE_TO_STRING: CodigoLint = CodigoLint { nome: "avoid_type_to_string", unico: "avoid_type_to_string", mensagem: "Using 'toString' on a 'Type' is not safe in production code.", correcao: Some("Try a normal type check or compare the 'runtimeType' directly."), documentado: true };
pub static AVOID_TYPES_AS_PARAMETER_NAMES: CodigoLint = CodigoLint { nome: "avoid_types_as_parameter_names", unico: "avoid_types_as_parameter_names", mensagem: "The parameter name '{0}' matches a visible type name.", correcao: Some("Try adding a name for the parameter or changing the parameter name to not match an existing type."), documentado: true };
pub static AVOID_TYPES_ON_CLOSURE_PARAMETERS: CodigoLint = CodigoLint { nome: "avoid_types_on_closure_parameters", unico: "avoid_types_on_closure_parameters", mensagem: "Unnecessary type annotation on a function expression parameter.", correcao: Some("Try removing the type annotation."), documentado: false };
pub static AVOID_UNNECESSARY_CONTAINERS: CodigoLint = CodigoLint { nome: "avoid_unnecessary_containers", unico: "avoid_unnecessary_containers", mensagem: "Unnecessary instance of 'Container'.", correcao: Some("Try removing the 'Container' (but not its children) from the widget tree."), documentado: true };
pub static AVOID_UNUSED_CONSTRUCTOR_PARAMETERS: CodigoLint = CodigoLint { nome: "avoid_unused_constructor_parameters", unico: "avoid_unused_constructor_parameters", mensagem: "The parameter '{0}' is not used in the constructor.", correcao: Some("Try using the parameter or removing it."), documentado: false };
pub static AVOID_VOID_ASYNC: CodigoLint = CodigoLint { nome: "avoid_void_async", unico: "avoid_void_async", mensagem: "An 'async' function should have a 'Future' return type when it doesn't return a value.", correcao: Some("Try changing the return type."), documentado: false };
pub static AVOID_WEB_LIBRARIES_IN_FLUTTER: CodigoLint = CodigoLint { nome: "avoid_web_libraries_in_flutter", unico: "avoid_web_libraries_in_flutter", mensagem: "Don't use web-only libraries outside Flutter web plugins.", correcao: Some("Try finding a different library for your needs."), documentado: true };
pub static AWAIT_ONLY_FUTURES: CodigoLint = CodigoLint { nome: "await_only_futures", unico: "await_only_futures", mensagem: "Uses 'await' on an instance of '{0}', which is not a subtype of 'Future'.", correcao: Some("Try removing the 'await' or changing the expression."), documentado: true };
pub static CAMEL_CASE_EXTENSIONS: CodigoLint = CodigoLint { nome: "camel_case_extensions", unico: "camel_case_extensions", mensagem: "The extension name '{0}' isn't an UpperCamelCase identifier.", correcao: Some("Try changing the name to follow the UpperCamelCase style."), documentado: true };
pub static CAMEL_CASE_TYPES: CodigoLint = CodigoLint { nome: "camel_case_types", unico: "camel_case_types", mensagem: "The type name '{0}' isn't an UpperCamelCase identifier.", correcao: Some("Try changing the name to follow the UpperCamelCase style."), documentado: true };
pub static CANCEL_SUBSCRIPTIONS: CodigoLint = CodigoLint { nome: "cancel_subscriptions", unico: "cancel_subscriptions", mensagem: "Uncancelled instance of 'StreamSubscription'.", correcao: Some("Try invoking 'cancel' in the function in which the 'StreamSubscription' was created."), documentado: true };
pub static CASCADE_INVOCATIONS: CodigoLint = CodigoLint { nome: "cascade_invocations", unico: "cascade_invocations", mensagem: "Unnecessary duplication of receiver.", correcao: Some("Try using a cascade to avoid the duplication."), documentado: false };
pub static CAST_NULLABLE_TO_NON_NULLABLE: CodigoLint = CodigoLint { nome: "cast_nullable_to_non_nullable", unico: "cast_nullable_to_non_nullable", mensagem: "Don't cast a nullable value to a non-nullable type.", correcao: Some("Try adding a not-null assertion ('!') to make the type non-nullable."), documentado: false };
pub static CLOSE_SINKS: CodigoLint = CodigoLint { nome: "close_sinks", unico: "close_sinks", mensagem: "Unclosed instance of 'Sink'.", correcao: Some("Try invoking 'close' in the function in which the 'Sink' was created."), documentado: true };
pub static COLLECTION_METHODS_UNRELATED_TYPE: CodigoLint = CodigoLint { nome: "collection_methods_unrelated_type", unico: "collection_methods_unrelated_type", mensagem: "The argument type '{0}' isn't related to '{1}'.", correcao: Some("Try changing the argument or element type to match."), documentado: true };
pub static COMBINATORS_ORDERING: CodigoLint = CodigoLint { nome: "combinators_ordering", unico: "combinators_ordering", mensagem: "Sort combinator names alphabetically.", correcao: Some("Try sorting the combinator names alphabetically."), documentado: false };
pub static COMMENT_REFERENCES: CodigoLint = CodigoLint { nome: "comment_references", unico: "comment_references", mensagem: "The referenced name isn't visible in scope.", correcao: Some("Try adding an import for the referenced name."), documentado: false };
pub static CONDITIONAL_URI_DOES_NOT_EXIST: CodigoLint = CodigoLint { nome: "conditional_uri_does_not_exist", unico: "conditional_uri_does_not_exist", mensagem: "The target of the conditional URI '{0}' doesn't exist.", correcao: Some("Try creating the file referenced by the URI, or try using a URI for a file that does exist."), documentado: false };
pub static CONSTANT_IDENTIFIER_NAMES: CodigoLint = CodigoLint { nome: "constant_identifier_names", unico: "constant_identifier_names", mensagem: "The constant name '{0}' isn't a lowerCamelCase identifier.", correcao: Some("Try changing the name to follow the lowerCamelCase style."), documentado: true };
pub static CONTROL_FLOW_IN_FINALLY: CodigoLint = CodigoLint { nome: "control_flow_in_finally", unico: "control_flow_in_finally", mensagem: "Use of '{0}' in a 'finally' clause.", correcao: Some("Try restructuring the code."), documentado: true };
pub static CURLY_BRACES_IN_FLOW_CONTROL_STRUCTURES: CodigoLint = CodigoLint { nome: "curly_braces_in_flow_control_structures", unico: "curly_braces_in_flow_control_structures", mensagem: "Statements in {0} should be enclosed in a block.", correcao: Some("Try wrapping the statement in a block."), documentado: true };
pub static DANGLING_LIBRARY_DOC_COMMENTS: CodigoLint = CodigoLint { nome: "dangling_library_doc_comments", unico: "dangling_library_doc_comments", mensagem: "Dangling library doc comment.", correcao: Some("Add a 'library' directive after the library comment."), documentado: true };
pub static DEPEND_ON_REFERENCED_PACKAGES: CodigoLint = CodigoLint { nome: "depend_on_referenced_packages", unico: "depend_on_referenced_packages", mensagem: "The imported package '{0}' isn't a dependency of the importing package.", correcao: Some("Try adding a dependency for '{0}' in the 'pubspec.yaml' file."), documentado: true };
pub static DEPRECATED_CONSISTENCY_CONSTRUCTOR: CodigoLint = CodigoLint { nome: "deprecated_consistency", unico: "deprecated_consistency_constructor", mensagem: "Constructors in a deprecated class should be deprecated.", correcao: Some("Try marking the constructor as deprecated."), documentado: false };
pub static DEPRECATED_CONSISTENCY_FIELD: CodigoLint = CodigoLint { nome: "deprecated_consistency", unico: "deprecated_consistency_field", mensagem: "Fields that are initialized by a deprecated parameter should be deprecated.", correcao: Some("Try marking the field as deprecated."), documentado: false };
pub static DEPRECATED_CONSISTENCY_PARAMETER: CodigoLint = CodigoLint { nome: "deprecated_consistency", unico: "deprecated_consistency_parameter", mensagem: "Parameters that initialize a deprecated field should be deprecated.", correcao: Some("Try marking the parameter as deprecated."), documentado: false };
pub static DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE_WITH_MESSAGE: CodigoLint = CodigoLint { nome: "deprecated_member_use_from_same_package", unico: "deprecated_member_use_from_same_package_with_message", mensagem: "'{0}' is deprecated and shouldn't be used. {1}", correcao: Some("Try replacing the use of the deprecated member with the replacement, if a replacement is specified."), documentado: false };
pub static DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE_WITHOUT_MESSAGE: CodigoLint = CodigoLint { nome: "deprecated_member_use_from_same_package", unico: "deprecated_member_use_from_same_package_without_message", mensagem: "'{0}' is deprecated and shouldn't be used.", correcao: Some("Try replacing the use of the deprecated member with the replacement, if a replacement is specified."), documentado: false };
pub static DIAGNOSTIC_DESCRIBE_ALL_PROPERTIES: CodigoLint = CodigoLint { nome: "diagnostic_describe_all_properties", unico: "diagnostic_describe_all_properties", mensagem: "The public property isn't described by either 'debugFillProperties' or 'debugDescribeChildren'.", correcao: Some("Try describing the property."), documentado: false };
pub static DIRECTIVES_ORDERING_ALPHABETICAL: CodigoLint = CodigoLint { nome: "directives_ordering", unico: "directives_ordering_alphabetical", mensagem: "Sort directive sections alphabetically.", correcao: Some("Try sorting the directives."), documentado: false };
pub static DIRECTIVES_ORDERING_DART: CodigoLint = CodigoLint { nome: "directives_ordering", unico: "directives_ordering_dart", mensagem: "Place 'dart:' {0}s before other {0}s.", correcao: Some("Try sorting the directives."), documentado: false };
pub static DIRECTIVES_ORDERING_EXPORTS: CodigoLint = CodigoLint { nome: "directives_ordering", unico: "directives_ordering_exports", mensagem: "Specify exports in a separate section after all imports.", correcao: Some("Try sorting the directives."), documentado: false };
pub static DIRECTIVES_ORDERING_PACKAGE_BEFORE_RELATIVE: CodigoLint = CodigoLint { nome: "directives_ordering", unico: "directives_ordering_package_before_relative", mensagem: "Place 'package:' {0}s before relative {0}s.", correcao: Some("Try sorting the directives."), documentado: false };
pub static DISCARDED_FUTURES: CodigoLint = CodigoLint { nome: "discarded_futures", unico: "discarded_futures", mensagem: "Asynchronous function invoked in a non-'async' function.", correcao: Some("Try converting the enclosing function to be 'async' and then 'await' the future."), documentado: false };
pub static DO_NOT_USE_ENVIRONMENT: CodigoLint = CodigoLint { nome: "do_not_use_environment", unico: "do_not_use_environment", mensagem: "Invalid use of an environment declaration.", correcao: Some("Try removing the environment declaration usage."), documentado: false };
pub static DOCUMENT_IGNORES: CodigoLint = CodigoLint { nome: "document_ignores", unico: "document_ignores", mensagem: "Missing documentation explaining why the diagnostic is ignored.", correcao: Some("Try adding a comment immediately above the ignore comment."), documentado: false };
pub static EMPTY_CATCHES: CodigoLint = CodigoLint { nome: "empty_catches", unico: "empty_catches", mensagem: "Empty catch block.", correcao: Some("Try adding statements to the block, adding a comment to the block, or removing the 'catch' clause."), documentado: true };
pub static EMPTY_CONSTRUCTOR_BODIES: CodigoLint = CodigoLint { nome: "empty_constructor_bodies", unico: "empty_constructor_bodies", mensagem: "Empty constructor bodies should be written using a ';' rather than '{}'.", correcao: Some("Try replacing the constructor body with ';'."), documentado: true };
pub static EMPTY_STATEMENTS: CodigoLint = CodigoLint { nome: "empty_statements", unico: "empty_statements", mensagem: "Unnecessary empty statement.", correcao: Some("Try removing the empty statement or restructuring the code."), documentado: true };
pub static EOL_AT_END_OF_FILE: CodigoLint = CodigoLint { nome: "eol_at_end_of_file", unico: "eol_at_end_of_file", mensagem: "Missing a newline at the end of the file.", correcao: Some("Try adding a newline at the end of the file."), documentado: false };
pub static ERASE_DART_TYPE_EXTENSION_TYPES: CodigoLint = CodigoLint { nome: "erase_dart_type_extension_types", unico: "erase_dart_type_extension_types", mensagem: "Unsafe use of 'DartType' in an 'is' check.", correcao: Some("Ensure DartType extension types are erased by using a helper method."), documentado: false };
pub static EXHAUSTIVE_CASES: CodigoLint = CodigoLint { nome: "exhaustive_cases", unico: "exhaustive_cases", mensagem: "Missing case clauses for some constants in '{0}'.", correcao: Some("Try adding case clauses for the missing constants."), documentado: false };
pub static FILE_NAMES: CodigoLint = CodigoLint { nome: "file_names", unico: "file_names", mensagem: "The file name '{0}' isn't a lower_case_with_underscores identifier.", correcao: Some("Try changing the name to follow the lower_case_with_underscores style."), documentado: true };
pub static FLUTTER_STYLE_TODOS: CodigoLint = CodigoLint { nome: "flutter_style_todos", unico: "flutter_style_todos", mensagem: "To-do comment doesn't follow the Flutter style.", correcao: Some("Try following the Flutter style for to-do comments."), documentado: false };
pub static HASH_AND_EQUALS: CodigoLint = CodigoLint { nome: "hash_and_equals", unico: "hash_and_equals", mensagem: "Missing a corresponding override of '{0}'.", correcao: Some("Try overriding '{0}' or removing '{1}'."), documentado: true };
pub static IMPLEMENTATION_IMPORTS: CodigoLint = CodigoLint { nome: "implementation_imports", unico: "implementation_imports", mensagem: "Import of a library in the 'lib/src' directory of another package.", correcao: Some("Try importing a public library that exports this library, or removing the import."), documentado: true };
pub static IMPLICIT_CALL_TEAROFFS: CodigoLint = CodigoLint { nome: "implicit_call_tearoffs", unico: "implicit_call_tearoffs", mensagem: "Implicit tear-off of the 'call' method.", correcao: Some("Try explicitly tearing off the 'call' method."), documentado: true };
pub static IMPLICIT_REOPEN: CodigoLint = CodigoLint { nome: "implicit_reopen", unico: "implicit_reopen", mensagem: "The {0} '{1}' reopens '{2}' because it is not marked '{3}'.", correcao: Some("Try marking '{1}' '{3}' or annotating it with '@reopen'."), documentado: false };
pub static INVALID_CASE_PATTERNS: CodigoLint = CodigoLint { nome: "invalid_case_patterns", unico: "invalid_case_patterns", mensagem: "This expression is not valid in a 'case' clause in Dart 3.0.", correcao: Some("Try refactoring the expression to be valid in 3.0."), documentado: false };
pub static INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_DART_AS_JS: CodigoLint = CodigoLint { nome: "invalid_runtime_check_with_js_interop_types", unico: "invalid_runtime_check_with_js_interop_types_dart_as_js", mensagem: "Cast from '{0}' to '{1}' casts a Dart value to a JS interop type, which might not be platform-consistent.", correcao: Some("Try using conversion methods from 'dart:js_interop' to convert between Dart types and JS interop types."), documentado: false };
pub static INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_DART_IS_JS: CodigoLint = CodigoLint { nome: "invalid_runtime_check_with_js_interop_types", unico: "invalid_runtime_check_with_js_interop_types_dart_is_js", mensagem: "Runtime check between '{0}' and '{1}' checks whether a Dart value is a JS interop type, which might not be platform-consistent.", correcao: None, documentado: false };
pub static INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_JS_AS_DART: CodigoLint = CodigoLint { nome: "invalid_runtime_check_with_js_interop_types", unico: "invalid_runtime_check_with_js_interop_types_js_as_dart", mensagem: "Cast from '{0}' to '{1}' casts a JS interop value to a Dart type, which might not be platform-consistent.", correcao: Some("Try using conversion methods from 'dart:js_interop' to convert between JS interop types and Dart types."), documentado: false };
pub static INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_JS_AS_INCOMPATIBLE_JS: CodigoLint = CodigoLint { nome: "invalid_runtime_check_with_js_interop_types", unico: "invalid_runtime_check_with_js_interop_types_js_as_incompatible_js", mensagem: "Cast from '{0}' to '{1}' casts a JS interop value to an incompatible JS interop type, which might not be platform-consistent.", correcao: None, documentado: false };
pub static INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_JS_IS_DART: CodigoLint = CodigoLint { nome: "invalid_runtime_check_with_js_interop_types", unico: "invalid_runtime_check_with_js_interop_types_js_is_dart", mensagem: "Runtime check between '{0}' and '{1}' checks whether a JS interop value is a Dart type, which might not be platform-consistent.", correcao: None, documentado: false };
pub static INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_JS_IS_INCONSISTENT_JS: CodigoLint = CodigoLint { nome: "invalid_runtime_check_with_js_interop_types", unico: "invalid_runtime_check_with_js_interop_types_js_is_inconsistent_js", mensagem: "Runtime check between '{0}' and '{1}' involves a non-trivial runtime check between two JS interop types that might not be platform-consistent.", correcao: Some("Try using a JS interop member like 'isA' from 'dart:js_interop' to check the underlying type of JS interop values."), documentado: false };
pub static INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_JS_IS_UNRELATED_JS: CodigoLint = CodigoLint { nome: "invalid_runtime_check_with_js_interop_types", unico: "invalid_runtime_check_with_js_interop_types_js_is_unrelated_js", mensagem: "Runtime check between '{0}' and '{1}' involves a runtime check between a JS interop value and an unrelated JS interop type that will always be true and won't check the underlying type.", correcao: Some("Try using a JS interop member like 'isA' from 'dart:js_interop' to check the underlying type of JS interop values, or make the JS interop type a supertype using 'implements'."), documentado: false };
pub static JOIN_RETURN_WITH_ASSIGNMENT: CodigoLint = CodigoLint { nome: "join_return_with_assignment", unico: "join_return_with_assignment", mensagem: "Assignment could be inlined in 'return' statement.", correcao: Some("Try inlining the assigned value in the 'return' statement."), documentado: false };
pub static LEADING_NEWLINES_IN_MULTILINE_STRINGS: CodigoLint = CodigoLint { nome: "leading_newlines_in_multiline_strings", unico: "leading_newlines_in_multiline_strings", mensagem: "Missing a newline at the beginning of a multiline string.", correcao: Some("Try adding a newline at the beginning of the string."), documentado: false };
pub static LIBRARY_ANNOTATIONS: CodigoLint = CodigoLint { nome: "library_annotations", unico: "library_annotations", mensagem: "This annotation should be attached to a library directive.", correcao: Some("Try attaching the annotation to a library directive."), documentado: false };
pub static LIBRARY_NAMES: CodigoLint = CodigoLint { nome: "library_names", unico: "library_names", mensagem: "The library name '{0}' isn't a lower_case_with_underscores identifier.", correcao: Some("Try changing the name to follow the lower_case_with_underscores style."), documentado: true };
pub static LIBRARY_PREFIXES: CodigoLint = CodigoLint { nome: "library_prefixes", unico: "library_prefixes", mensagem: "The prefix '{0}' isn't a lower_case_with_underscores identifier.", correcao: Some("Try changing the prefix to follow the lower_case_with_underscores style."), documentado: true };
pub static LIBRARY_PRIVATE_TYPES_IN_PUBLIC_API: CodigoLint = CodigoLint { nome: "library_private_types_in_public_api", unico: "library_private_types_in_public_api", mensagem: "Invalid use of a private type in a public API.", correcao: Some("Try making the private type public, or making the API that uses the private type also be private."), documentado: true };
pub static LINES_LONGER_THAN_80_CHARS: CodigoLint = CodigoLint { nome: "lines_longer_than_80_chars", unico: "lines_longer_than_80_chars", mensagem: "The line length exceeds the 80-character limit.", correcao: Some("Try breaking the line across multiple lines."), documentado: false };
pub static LITERAL_ONLY_BOOLEAN_EXPRESSIONS: CodigoLint = CodigoLint { nome: "literal_only_boolean_expressions", unico: "literal_only_boolean_expressions", mensagem: "The Boolean expression has a constant value.", correcao: Some("Try changing the expression."), documentado: true };
pub static MATCHING_SUPER_PARAMETERS: CodigoLint = CodigoLint { nome: "matching_super_parameters", unico: "matching_super_parameters", mensagem: "The super parameter named '{0}'' does not share the same name as the corresponding parameter in the super constructor, '{1}'.", correcao: Some("Try using the name of the corresponding parameter in the super constructor."), documentado: false };
pub static MISSING_CODE_BLOCK_LANGUAGE_IN_DOC_COMMENT: CodigoLint = CodigoLint { nome: "missing_code_block_language_in_doc_comment", unico: "missing_code_block_language_in_doc_comment", mensagem: "The code block is missing a specified language.", correcao: Some("Try adding a language to the code block."), documentado: false };
pub static MISSING_WHITESPACE_BETWEEN_ADJACENT_STRINGS: CodigoLint = CodigoLint { nome: "missing_whitespace_between_adjacent_strings", unico: "missing_whitespace_between_adjacent_strings", mensagem: "Missing whitespace between adjacent strings.", correcao: Some("Try adding whitespace between the strings."), documentado: false };
pub static NO_ADJACENT_STRINGS_IN_LIST: CodigoLint = CodigoLint { nome: "no_adjacent_strings_in_list", unico: "no_adjacent_strings_in_list", mensagem: "Don't use adjacent strings in a list literal.", correcao: Some("Try adding a comma between the strings."), documentado: true };
pub static NO_DEFAULT_CASES: CodigoLint = CodigoLint { nome: "no_default_cases", unico: "no_default_cases", mensagem: "Invalid use of 'default' member in a switch.", correcao: Some("Try enumerating all the possible values of the switch expression."), documentado: false };
pub static NO_DUPLICATE_CASE_VALUES: CodigoLint = CodigoLint { nome: "no_duplicate_case_values", unico: "no_duplicate_case_values", mensagem: "The value of the case clause ('{0}') is equal to the value of an earlier case clause ('{1}').", correcao: Some("Try removing or changing the value."), documentado: true };
pub static NO_LEADING_UNDERSCORES_FOR_LIBRARY_PREFIXES: CodigoLint = CodigoLint { nome: "no_leading_underscores_for_library_prefixes", unico: "no_leading_underscores_for_library_prefixes", mensagem: "The library prefix '{0}' starts with an underscore.", correcao: Some("Try renaming the prefix to not start with an underscore."), documentado: true };
pub static NO_LEADING_UNDERSCORES_FOR_LOCAL_IDENTIFIERS: CodigoLint = CodigoLint { nome: "no_leading_underscores_for_local_identifiers", unico: "no_leading_underscores_for_local_identifiers", mensagem: "The local variable '{0}' starts with an underscore.", correcao: Some("Try renaming the variable to not start with an underscore."), documentado: true };
pub static NO_LITERAL_BOOL_COMPARISONS: CodigoLint = CodigoLint { nome: "no_literal_bool_comparisons", unico: "no_literal_bool_comparisons", mensagem: "Unnecessary comparison to a boolean literal.", correcao: Some("Remove the comparison and use the negate `!` operator if necessary."), documentado: false };
pub static NO_LOGIC_IN_CREATE_STATE: CodigoLint = CodigoLint { nome: "no_logic_in_create_state", unico: "no_logic_in_create_state", mensagem: "Don't put any logic in 'createState'.", correcao: Some("Try moving the logic out of 'createState'."), documentado: true };
pub static NO_RUNTIMETYPE_TOSTRING: CodigoLint = CodigoLint { nome: "no_runtimeType_toString", unico: "no_runtimeType_toString", mensagem: "Using 'toString' on a 'Type' is not safe in production code.", correcao: Some("Try removing the usage of 'toString' or restructuring the code."), documentado: false };
pub static NO_SELF_ASSIGNMENTS: CodigoLint = CodigoLint { nome: "no_self_assignments", unico: "no_self_assignments", mensagem: "The variable or property is being assigned to itself.", correcao: Some("Try removing the assignment that has no direct effect."), documentado: false };
pub static NO_WILDCARD_VARIABLE_USES: CodigoLint = CodigoLint { nome: "no_wildcard_variable_uses", unico: "no_wildcard_variable_uses", mensagem: "The referenced identifier is a wildcard.", correcao: Some("Use an identifier name that is not a wildcard."), documentado: true };
pub static NON_CONSTANT_IDENTIFIER_NAMES: CodigoLint = CodigoLint { nome: "non_constant_identifier_names", unico: "non_constant_identifier_names", mensagem: "The variable name '{0}' isn't a lowerCamelCase identifier.", correcao: Some("Try changing the name to follow the lowerCamelCase style."), documentado: true };
pub static NOOP_PRIMITIVE_OPERATIONS: CodigoLint = CodigoLint { nome: "noop_primitive_operations", unico: "noop_primitive_operations", mensagem: "The expression has no effect and can be removed.", correcao: Some("Try removing the expression."), documentado: false };
pub static NULL_CHECK_ON_NULLABLE_TYPE_PARAMETER: CodigoLint = CodigoLint { nome: "null_check_on_nullable_type_parameter", unico: "null_check_on_nullable_type_parameter", mensagem: "The null check operator shouldn't be used on a variable whose type is a potentially nullable type parameter.", correcao: Some("Try explicitly testing for 'null'."), documentado: true };
pub static NULL_CLOSURES: CodigoLint = CodigoLint { nome: "null_closures", unico: "null_closures", mensagem: "Closure can't be 'null' because it might be invoked.", correcao: Some("Try providing a non-null closure."), documentado: false };
pub static OMIT_LOCAL_VARIABLE_TYPES: CodigoLint = CodigoLint { nome: "omit_local_variable_types", unico: "omit_local_variable_types", mensagem: "Unnecessary type annotation on a local variable.", correcao: Some("Try removing the type annotation."), documentado: false };
pub static OMIT_OBVIOUS_LOCAL_VARIABLE_TYPES: CodigoLint = CodigoLint { nome: "omit_obvious_local_variable_types", unico: "omit_obvious_local_variable_types", mensagem: "Omit the type annotation on a local variable when the type is obvious.", correcao: Some("Try removing the type annotation."), documentado: false };
pub static ONE_MEMBER_ABSTRACTS: CodigoLint = CodigoLint { nome: "one_member_abstracts", unico: "one_member_abstracts", mensagem: "Unnecessary use of an abstract class.", correcao: Some("Try making '{0}' a top-level function and removing the class."), documentado: false };
pub static ONLY_THROW_ERRORS: CodigoLint = CodigoLint { nome: "only_throw_errors", unico: "only_throw_errors", mensagem: "Don't throw instances of classes that don't extend either 'Exception' or 'Error'.", correcao: Some("Try throwing a different class of object."), documentado: false };
pub static OVERRIDDEN_FIELDS: CodigoLint = CodigoLint { nome: "overridden_fields", unico: "overridden_fields", mensagem: "Field overrides a field inherited from '{0}'.", correcao: Some("Try removing the field, overriding the getter and setter if necessary."), documentado: true };
pub static PACKAGE_API_DOCS: CodigoLint = CodigoLint { nome: "package_api_docs", unico: "package_api_docs", mensagem: "Missing documentation for public API.", correcao: Some("Try adding a documentation comment."), documentado: false };
pub static PACKAGE_NAMES: CodigoLint = CodigoLint { nome: "package_names", unico: "package_names", mensagem: "The package name '{0}' isn't a lower_case_with_underscores identifier.", correcao: Some("Try changing the name to follow the lower_case_with_underscores style."), documentado: true };
pub static PACKAGE_PREFIXED_LIBRARY_NAMES: CodigoLint = CodigoLint { nome: "package_prefixed_library_names", unico: "package_prefixed_library_names", mensagem: "The library name is not a dot-separated path prefixed by the package name.", correcao: Some("Try changing the name to '{0}'."), documentado: true };
pub static PARAMETER_ASSIGNMENTS: CodigoLint = CodigoLint { nome: "parameter_assignments", unico: "parameter_assignments", mensagem: "Invalid assignment to the parameter '{0}'.", correcao: Some("Try using a local variable in place of the parameter."), documentado: false };
pub static PREFER_ADJACENT_STRING_CONCATENATION: CodigoLint = CodigoLint { nome: "prefer_adjacent_string_concatenation", unico: "prefer_adjacent_string_concatenation", mensagem: "String literals shouldn't be concatenated by the '+' operator.", correcao: Some("Try removing the operator to use adjacent strings."), documentado: true };
pub static PREFER_ASSERTS_IN_INITIALIZER_LISTS: CodigoLint = CodigoLint { nome: "prefer_asserts_in_initializer_lists", unico: "prefer_asserts_in_initializer_lists", mensagem: "Assert should be in the initializer list.", correcao: Some("Try moving the assert to the initializer list."), documentado: false };
pub static PREFER_ASSERTS_WITH_MESSAGE: CodigoLint = CodigoLint { nome: "prefer_asserts_with_message", unico: "prefer_asserts_with_message", mensagem: "Missing a message in an assert.", correcao: Some("Try adding a message to the assert."), documentado: false };
pub static PREFER_COLLECTION_LITERALS: CodigoLint = CodigoLint { nome: "prefer_collection_literals", unico: "prefer_collection_literals", mensagem: "Unnecessary constructor invocation.", correcao: Some("Try using a collection literal."), documentado: true };
pub static PREFER_CONDITIONAL_ASSIGNMENT: CodigoLint = CodigoLint { nome: "prefer_conditional_assignment", unico: "prefer_conditional_assignment", mensagem: "The 'if' statement could be replaced by a null-aware assignment.", correcao: Some("Try using the '??=' operator to conditionally assign a value."), documentado: true };
pub static PREFER_CONST_CONSTRUCTORS: CodigoLint = CodigoLint { nome: "prefer_const_constructors", unico: "prefer_const_constructors", mensagem: "Use 'const' with the constructor to improve performance.", correcao: Some("Try adding the 'const' keyword to the constructor invocation."), documentado: true };
pub static PREFER_CONST_CONSTRUCTORS_IN_IMMUTABLES: CodigoLint = CodigoLint { nome: "prefer_const_constructors_in_immutables", unico: "prefer_const_constructors_in_immutables", mensagem: "Constructors in '@immutable' classes should be declared as 'const'.", correcao: Some("Try adding 'const' to the constructor declaration."), documentado: true };
pub static PREFER_CONST_DECLARATIONS: CodigoLint = CodigoLint { nome: "prefer_const_declarations", unico: "prefer_const_declarations", mensagem: "Use 'const' for final variables initialized to a constant value.", correcao: Some("Try replacing 'final' with 'const'."), documentado: true };
pub static PREFER_CONST_LITERALS_TO_CREATE_IMMUTABLES: CodigoLint = CodigoLint { nome: "prefer_const_literals_to_create_immutables", unico: "prefer_const_literals_to_create_immutables", mensagem: "Use 'const' literals as arguments to constructors of '@immutable' classes.", correcao: Some("Try adding 'const' before the literal."), documentado: true };
pub static PREFER_CONSTRUCTORS_OVER_STATIC_METHODS: CodigoLint = CodigoLint { nome: "prefer_constructors_over_static_methods", unico: "prefer_constructors_over_static_methods", mensagem: "Static method should be a constructor.", correcao: Some("Try converting the method into a constructor."), documentado: false };
pub static PREFER_CONTAINS_ALWAYS_FALSE: CodigoLint = CodigoLint { nome: "prefer_contains", unico: "prefer_contains_always_false", mensagem: "Always 'false' because 'indexOf' is always greater than or equal to -1.", correcao: None, documentado: false };
pub static PREFER_CONTAINS_ALWAYS_TRUE: CodigoLint = CodigoLint { nome: "prefer_contains", unico: "prefer_contains_always_true", mensagem: "Always 'true' because 'indexOf' is always greater than or equal to -1.", correcao: None, documentado: false };
pub static PREFER_CONTAINS_USE_CONTAINS: CodigoLint = CodigoLint { nome: "prefer_contains", unico: "prefer_contains_use_contains", mensagem: "Unnecessary use of 'indexOf' to test for containment.", correcao: Some("Try using 'contains'."), documentado: true };
pub static PREFER_DOUBLE_QUOTES: CodigoLint = CodigoLint { nome: "prefer_double_quotes", unico: "prefer_double_quotes", mensagem: "Unnecessary use of single quotes.", correcao: Some("Try using double quotes unless the string contains double quotes."), documentado: true };
pub static PREFER_EXPRESSION_FUNCTION_BODIES: CodigoLint = CodigoLint { nome: "prefer_expression_function_bodies", unico: "prefer_expression_function_bodies", mensagem: "Unnecessary use of a block function body.", correcao: Some("Try using an expression function body."), documentado: false };
pub static PREFER_FINAL_FIELDS: CodigoLint = CodigoLint { nome: "prefer_final_fields", unico: "prefer_final_fields", mensagem: "The private field {0} could be 'final'.", correcao: Some("Try making the field 'final'."), documentado: true };
pub static PREFER_FINAL_IN_FOR_EACH_PATTERN: CodigoLint = CodigoLint { nome: "prefer_final_in_for_each", unico: "prefer_final_in_for_each_pattern", mensagem: "The pattern should be final.", correcao: Some("Try making the pattern final."), documentado: false };
pub static PREFER_FINAL_IN_FOR_EACH_VARIABLE: CodigoLint = CodigoLint { nome: "prefer_final_in_for_each", unico: "prefer_final_in_for_each_variable", mensagem: "The variable '{0}' should be final.", correcao: Some("Try making the variable final."), documentado: false };
pub static PREFER_FINAL_LOCALS: CodigoLint = CodigoLint { nome: "prefer_final_locals", unico: "prefer_final_locals", mensagem: "Local variables should be final.", correcao: Some("Try making the variable final."), documentado: false };
pub static PREFER_FINAL_PARAMETERS: CodigoLint = CodigoLint { nome: "prefer_final_parameters", unico: "prefer_final_parameters", mensagem: "The parameter '{0}' should be final.", correcao: Some("Try making the parameter final."), documentado: false };
pub static PREFER_FOR_ELEMENTS_TO_MAP_FROMITERABLE: CodigoLint = CodigoLint { nome: "prefer_for_elements_to_map_fromIterable", unico: "prefer_for_elements_to_map_fromIterable", mensagem: "Use 'for' elements when building maps from iterables.", correcao: Some("Try using a collection literal with a 'for' element."), documentado: true };
pub static PREFER_FOREACH: CodigoLint = CodigoLint { nome: "prefer_foreach", unico: "prefer_foreach", mensagem: "Use 'forEach' rather than a 'for' loop to apply a function to every element.", correcao: Some("Try using 'forEach' rather than a 'for' loop."), documentado: false };
pub static PREFER_FUNCTION_DECLARATIONS_OVER_VARIABLES: CodigoLint = CodigoLint { nome: "prefer_function_declarations_over_variables", unico: "prefer_function_declarations_over_variables", mensagem: "Use a function declaration rather than a variable assignment to bind a function to a name.", correcao: Some("Try rewriting the closure assignment as a function declaration."), documentado: true };
pub static PREFER_GENERIC_FUNCTION_TYPE_ALIASES: CodigoLint = CodigoLint { nome: "prefer_generic_function_type_aliases", unico: "prefer_generic_function_type_aliases", mensagem: "Use the generic function type syntax in 'typedef's.", correcao: Some("Try using the generic function type syntax ('{0}')."), documentado: true };
pub static PREFER_IF_ELEMENTS_TO_CONDITIONAL_EXPRESSIONS: CodigoLint = CodigoLint { nome: "prefer_if_elements_to_conditional_expressions", unico: "prefer_if_elements_to_conditional_expressions", mensagem: "Use an 'if' element to conditionally add elements.", correcao: Some("Try using an 'if' element rather than a conditional expression."), documentado: false };
pub static PREFER_IF_NULL_OPERATORS: CodigoLint = CodigoLint { nome: "prefer_if_null_operators", unico: "prefer_if_null_operators", mensagem: "Use the '??' operator rather than '?:' when testing for 'null'.", correcao: Some("Try rewriting the code to use '??'."), documentado: true };
pub static PREFER_INITIALIZING_FORMALS: CodigoLint = CodigoLint { nome: "prefer_initializing_formals", unico: "prefer_initializing_formals", mensagem: "Use an initializing formal to assign a parameter to a field.", correcao: Some("Try using an initialing formal ('this.{0}') to initialize the field."), documentado: true };
pub static PREFER_INLINED_ADDS_MULTIPLE: CodigoLint = CodigoLint { nome: "prefer_inlined_adds", unico: "prefer_inlined_adds_multiple", mensagem: "The addition of multiple list items could be inlined.", correcao: Some("Try adding the items to the list literal directly."), documentado: true };
pub static PREFER_INLINED_ADDS_SINGLE: CodigoLint = CodigoLint { nome: "prefer_inlined_adds", unico: "prefer_inlined_adds_single", mensagem: "The addition of a list item could be inlined.", correcao: Some("Try adding the item to the list literal directly."), documentado: true };
pub static PREFER_INT_LITERALS: CodigoLint = CodigoLint { nome: "prefer_int_literals", unico: "prefer_int_literals", mensagem: "Unnecessary use of a 'double' literal.", correcao: Some("Try using an 'int' literal."), documentado: false };
pub static PREFER_INTERPOLATION_TO_COMPOSE_STRINGS: CodigoLint = CodigoLint { nome: "prefer_interpolation_to_compose_strings", unico: "prefer_interpolation_to_compose_strings", mensagem: "Use interpolation to compose strings and values.", correcao: Some("Try using string interpolation to build the composite string."), documentado: true };
pub static PREFER_IS_EMPTY_ALWAYS_FALSE: CodigoLint = CodigoLint { nome: "prefer_is_empty", unico: "prefer_is_empty_always_false", mensagem: "The comparison is always 'false' because the length is always greater than or equal to 0.", correcao: None, documentado: false };
pub static PREFER_IS_EMPTY_ALWAYS_TRUE: CodigoLint = CodigoLint { nome: "prefer_is_empty", unico: "prefer_is_empty_always_true", mensagem: "The comparison is always 'true' because the length is always greater than or equal to 0.", correcao: None, documentado: false };
pub static PREFER_IS_EMPTY_USE_IS_EMPTY: CodigoLint = CodigoLint { nome: "prefer_is_empty", unico: "prefer_is_empty_use_is_empty", mensagem: "Use 'isEmpty' instead of 'length' to test whether the collection is empty.", correcao: Some("Try rewriting the expression to use 'isEmpty'."), documentado: true };
pub static PREFER_IS_EMPTY_USE_IS_NOT_EMPTY: CodigoLint = CodigoLint { nome: "prefer_is_empty", unico: "prefer_is_empty_use_is_not_empty", mensagem: "Use 'isNotEmpty' instead of 'length' to test whether the collection is empty.", correcao: Some("Try rewriting the expression to use 'isNotEmpty'."), documentado: true };
pub static PREFER_IS_NOT_EMPTY: CodigoLint = CodigoLint { nome: "prefer_is_not_empty", unico: "prefer_is_not_empty", mensagem: "Use 'isNotEmpty' rather than negating the result of 'isEmpty'.", correcao: Some("Try rewriting the expression to use 'isNotEmpty'."), documentado: true };
pub static PREFER_IS_NOT_OPERATOR: CodigoLint = CodigoLint { nome: "prefer_is_not_operator", unico: "prefer_is_not_operator", mensagem: "Use the 'is!' operator rather than negating the value of the 'is' operator.", correcao: Some("Try rewriting the condition to use the 'is!' operator."), documentado: true };
pub static PREFER_ITERABLE_WHERETYPE: CodigoLint = CodigoLint { nome: "prefer_iterable_whereType", unico: "prefer_iterable_whereType", mensagem: "Use 'whereType' to select elements of a given type.", correcao: Some("Try rewriting the expression to use 'whereType'."), documentado: true };
pub static PREFER_MIXIN: CodigoLint = CodigoLint { nome: "prefer_mixin", unico: "prefer_mixin", mensagem: "Only mixins should be mixed in.", correcao: Some("Try converting '{0}' to a mixin."), documentado: false };
pub static PREFER_NULL_AWARE_METHOD_CALLS: CodigoLint = CodigoLint { nome: "prefer_null_aware_method_calls", unico: "prefer_null_aware_method_calls", mensagem: "Use a null-aware invocation of the 'call' method rather than explicitly testing for 'null'.", correcao: Some("Try using '?.call()' to invoke the function."), documentado: false };
pub static PREFER_NULL_AWARE_OPERATORS: CodigoLint = CodigoLint { nome: "prefer_null_aware_operators", unico: "prefer_null_aware_operators", mensagem: "Use the null-aware operator '?.' rather than an explicit 'null' comparison.", correcao: Some("Try using '?.'."), documentado: true };
pub static PREFER_RELATIVE_IMPORTS: CodigoLint = CodigoLint { nome: "prefer_relative_imports", unico: "prefer_relative_imports", mensagem: "Use relative imports for files in the 'lib' directory.", correcao: Some("Try converting the URI to a relative URI."), documentado: true };
pub static PREFER_SINGLE_QUOTES: CodigoLint = CodigoLint { nome: "prefer_single_quotes", unico: "prefer_single_quotes", mensagem: "Unnecessary use of double quotes.", correcao: Some("Try using single quotes unless the string contains single quotes."), documentado: true };
pub static PREFER_SPREAD_COLLECTIONS: CodigoLint = CodigoLint { nome: "prefer_spread_collections", unico: "prefer_spread_collections", mensagem: "The addition of multiple elements could be inlined.", correcao: Some("Try using the spread operator ('...') to inline the addition."), documentado: false };
pub static PREFER_TYPING_UNINITIALIZED_VARIABLES_FOR_FIELD: CodigoLint = CodigoLint { nome: "prefer_typing_uninitialized_variables", unico: "prefer_typing_uninitialized_variables_for_field", mensagem: "An uninitialized field should have an explicit type annotation.", correcao: Some("Try adding a type annotation."), documentado: true };
pub static PREFER_TYPING_UNINITIALIZED_VARIABLES_FOR_LOCAL_VARIABLE: CodigoLint = CodigoLint { nome: "prefer_typing_uninitialized_variables", unico: "prefer_typing_uninitialized_variables_for_local_variable", mensagem: "An uninitialized variable should have an explicit type annotation.", correcao: Some("Try adding a type annotation."), documentado: true };
pub static PREFER_VOID_TO_NULL: CodigoLint = CodigoLint { nome: "prefer_void_to_null", unico: "prefer_void_to_null", mensagem: "Unnecessary use of the type 'Null'.", correcao: Some("Try using 'void' instead."), documentado: true };
pub static PROVIDE_DEPRECATION_MESSAGE: CodigoLint = CodigoLint { nome: "provide_deprecation_message", unico: "provide_deprecation_message", mensagem: "Missing a deprecation message.", correcao: Some("Try using the constructor to provide a message ('@Deprecated(\"message\")')."), documentado: true };
pub static PUBLIC_MEMBER_API_DOCS: CodigoLint = CodigoLint { nome: "public_member_api_docs", unico: "public_member_api_docs", mensagem: "Missing documentation for a public member.", correcao: Some("Try adding documentation for the member."), documentado: false };
pub static RECURSIVE_GETTERS: CodigoLint = CodigoLint { nome: "recursive_getters", unico: "recursive_getters", mensagem: "The getter '{0}' recursively returns itself.", correcao: Some("Try changing the value being returned."), documentado: true };
pub static REQUIRE_TRAILING_COMMAS: CodigoLint = CodigoLint { nome: "require_trailing_commas", unico: "require_trailing_commas", mensagem: "Missing a required trailing comma.", correcao: Some("Try adding a trailing comma."), documentado: false };
pub static SECURE_PUBSPEC_URLS: CodigoLint = CodigoLint { nome: "secure_pubspec_urls", unico: "secure_pubspec_urls", mensagem: "The '{0}' protocol shouldn't be used because it isn't secure.", correcao: Some("Try using a secure protocol, such as 'https'."), documentado: true };
pub static SIZED_BOX_FOR_WHITESPACE: CodigoLint = CodigoLint { nome: "sized_box_for_whitespace", unico: "sized_box_for_whitespace", mensagem: "Use a 'SizedBox' to add whitespace to a layout.", correcao: Some("Try using a 'SizedBox' rather than a 'Container'."), documentado: true };
pub static SIZED_BOX_SHRINK_EXPAND: CodigoLint = CodigoLint { nome: "sized_box_shrink_expand", unico: "sized_box_shrink_expand", mensagem: "Use 'SizedBox.{0}' to avoid needing to specify the 'height' and 'width'.", correcao: Some("Try using 'SizedBox.{0}' and removing the 'height' and 'width' arguments."), documentado: true };
pub static SLASH_FOR_DOC_COMMENTS: CodigoLint = CodigoLint { nome: "slash_for_doc_comments", unico: "slash_for_doc_comments", mensagem: "Use the end-of-line form ('///') for doc comments.", correcao: Some("Try rewriting the comment to use '///'."), documentado: true };
pub static SORT_CHILD_PROPERTIES_LAST: CodigoLint = CodigoLint { nome: "sort_child_properties_last", unico: "sort_child_properties_last", mensagem: "The '{0}' argument should be last in widget constructor invocations.", correcao: Some("Try moving the argument to the end of the argument list."), documentado: true };
pub static SORT_CONSTRUCTORS_FIRST: CodigoLint = CodigoLint { nome: "sort_constructors_first", unico: "sort_constructors_first", mensagem: "Constructor declarations should be before non-constructor declarations.", correcao: Some("Try moving the constructor declaration before all other members."), documentado: true };
pub static SORT_PUB_DEPENDENCIES: CodigoLint = CodigoLint { nome: "sort_pub_dependencies", unico: "sort_pub_dependencies", mensagem: "Dependencies not sorted alphabetically.", correcao: Some("Try sorting the dependencies alphabetically (A to Z)."), documentado: true };
pub static SORT_UNNAMED_CONSTRUCTORS_FIRST: CodigoLint = CodigoLint { nome: "sort_unnamed_constructors_first", unico: "sort_unnamed_constructors_first", mensagem: "Invalid location for the unnamed constructor.", correcao: Some("Try moving the unnamed constructor before all other constructors."), documentado: true };
pub static SPECIFY_NONOBVIOUS_LOCAL_VARIABLE_TYPES: CodigoLint = CodigoLint { nome: "specify_nonobvious_local_variable_types", unico: "specify_nonobvious_local_variable_types", mensagem: "Specify the type of a local variable when the type is non-obvious.", correcao: Some("Try adding a type annotation."), documentado: false };
pub static TEST_TYPES_IN_EQUALS: CodigoLint = CodigoLint { nome: "test_types_in_equals", unico: "test_types_in_equals", mensagem: "Missing type test for '{0}' in '=='.", correcao: Some("Try testing the type of '{0}'."), documentado: true };
pub static THROW_IN_FINALLY: CodigoLint = CodigoLint { nome: "throw_in_finally", unico: "throw_in_finally", mensagem: "Use of '{0}' in 'finally' block.", correcao: Some("Try moving the '{0}' outside the 'finally' block."), documentado: true };
pub static TIGHTEN_TYPE_OF_INITIALIZING_FORMALS: CodigoLint = CodigoLint { nome: "tighten_type_of_initializing_formals", unico: "tighten_type_of_initializing_formals", mensagem: "Use a type annotation rather than 'assert' to enforce non-nullability.", correcao: Some("Try adding a type annotation and removing the 'assert'."), documentado: false };
pub static TYPE_ANNOTATE_PUBLIC_APIS: CodigoLint = CodigoLint { nome: "type_annotate_public_apis", unico: "type_annotate_public_apis", mensagem: "Missing type annotation on a public API.", correcao: Some("Try adding a type annotation."), documentado: false };
pub static TYPE_INIT_FORMALS: CodigoLint = CodigoLint { nome: "type_init_formals", unico: "type_init_formals", mensagem: "Don't needlessly type annotate initializing formals.", correcao: Some("Try removing the type."), documentado: true };
pub static TYPE_LITERAL_IN_CONSTANT_PATTERN: CodigoLint = CodigoLint { nome: "type_literal_in_constant_pattern", unico: "type_literal_in_constant_pattern", mensagem: "Use 'TypeName _' instead of a type literal.", correcao: Some("Replace with 'TypeName _'."), documentado: true };
pub static UNAWAITED_FUTURES: CodigoLint = CodigoLint { nome: "unawaited_futures", unico: "unawaited_futures", mensagem: "Missing an 'await' for the 'Future' computed by this expression.", correcao: Some("Try adding an 'await' or wrapping the expression with 'unawaited'."), documentado: true };
pub static UNINTENDED_HTML_IN_DOC_COMMENT: CodigoLint = CodigoLint { nome: "unintended_html_in_doc_comment", unico: "unintended_html_in_doc_comment", mensagem: "Angle brackets will be interpreted as HTML.", correcao: Some("Try using backticks around the content with angle brackets, or try replacing `<` with `&lt;` and `>` with `&gt;`."), documentado: false };
pub static UNNECESSARY_AWAIT_IN_RETURN: CodigoLint = CodigoLint { nome: "unnecessary_await_in_return", unico: "unnecessary_await_in_return", mensagem: "Unnecessary 'await'.", correcao: Some("Try removing the 'await'."), documentado: false };
pub static UNNECESSARY_BRACE_IN_STRING_INTERPS: CodigoLint = CodigoLint { nome: "unnecessary_brace_in_string_interps", unico: "unnecessary_brace_in_string_interps", mensagem: "Unnecessary braces in a string interpolation.", correcao: Some("Try removing the braces."), documentado: true };
pub static UNNECESSARY_BREAKS: CodigoLint = CodigoLint { nome: "unnecessary_breaks", unico: "unnecessary_breaks", mensagem: "Unnecessary 'break' statement.", correcao: Some("Try removing the 'break'."), documentado: false };
pub static UNNECESSARY_CONST: CodigoLint = CodigoLint { nome: "unnecessary_const", unico: "unnecessary_const", mensagem: "Unnecessary 'const' keyword.", correcao: Some("Try removing the keyword."), documentado: true };
pub static UNNECESSARY_CONSTRUCTOR_NAME: CodigoLint = CodigoLint { nome: "unnecessary_constructor_name", unico: "unnecessary_constructor_name", mensagem: "Unnecessary '.new' constructor name.", correcao: Some("Try removing the '.new'."), documentado: true };
pub static UNNECESSARY_FINAL_WITH_TYPE: CodigoLint = CodigoLint { nome: "unnecessary_final", unico: "unnecessary_final_with_type", mensagem: "Local variables should not be marked as 'final'.", correcao: Some("Remove the 'final'."), documentado: true };
pub static UNNECESSARY_FINAL_WITHOUT_TYPE: CodigoLint = CodigoLint { nome: "unnecessary_final", unico: "unnecessary_final_without_type", mensagem: "Local variables should not be marked as 'final'.", correcao: Some("Replace 'final' with 'var'."), documentado: false };
pub static UNNECESSARY_GETTERS_SETTERS: CodigoLint = CodigoLint { nome: "unnecessary_getters_setters", unico: "unnecessary_getters_setters", mensagem: "Unnecessary use of getter and setter to wrap a field.", correcao: Some("Try removing the getter and setter and renaming the field."), documentado: true };
pub static UNNECESSARY_LAMBDAS: CodigoLint = CodigoLint { nome: "unnecessary_lambdas", unico: "unnecessary_lambdas", mensagem: "Closure should be a tearoff.", correcao: Some("Try using a tearoff rather than a closure."), documentado: true };
pub static UNNECESSARY_LATE: CodigoLint = CodigoLint { nome: "unnecessary_late", unico: "unnecessary_late", mensagem: "Unnecessary 'late' modifier.", correcao: Some("Try removing the 'late'."), documentado: true };
pub static UNNECESSARY_LIBRARY_DIRECTIVE: CodigoLint = CodigoLint { nome: "unnecessary_library_directive", unico: "unnecessary_library_directive", mensagem: "Library directives without comments or annotations should be avoided.", correcao: Some("Try deleting the library directive."), documentado: false };
pub static UNNECESSARY_LIBRARY_NAME: CodigoLint = CodigoLint { nome: "unnecessary_library_name", unico: "unnecessary_library_name", mensagem: "Library names are not necessary.", correcao: Some("Remove the library name."), documentado: false };
pub static UNNECESSARY_NEW: CodigoLint = CodigoLint { nome: "unnecessary_new", unico: "unnecessary_new", mensagem: "Unnecessary 'new' keyword.", correcao: Some("Try removing the 'new' keyword."), documentado: true };
pub static UNNECESSARY_NULL_AWARE_ASSIGNMENTS: CodigoLint = CodigoLint { nome: "unnecessary_null_aware_assignments", unico: "unnecessary_null_aware_assignments", mensagem: "Unnecessary assignment of 'null'.", correcao: Some("Try removing the assignment."), documentado: true };
pub static UNNECESSARY_NULL_AWARE_OPERATOR_ON_EXTENSION_ON_NULLABLE: CodigoLint = CodigoLint { nome: "unnecessary_null_aware_operator_on_extension_on_nullable", unico: "unnecessary_null_aware_operator_on_extension_on_nullable", mensagem: "Unnecessary use of a null-aware operator to invoke an extension method on a nullable type.", correcao: Some("Try removing the '?'."), documentado: false };
pub static UNNECESSARY_NULL_CHECKS: CodigoLint = CodigoLint { nome: "unnecessary_null_checks", unico: "unnecessary_null_checks", mensagem: "Unnecessary use of a null check ('!').", correcao: Some("Try removing the null check."), documentado: false };
pub static UNNECESSARY_NULL_IN_IF_NULL_OPERATORS: CodigoLint = CodigoLint { nome: "unnecessary_null_in_if_null_operators", unico: "unnecessary_null_in_if_null_operators", mensagem: "Unnecessary use of '??' with 'null'.", correcao: Some("Try removing the '??' operator and the 'null' operand."), documentado: true };
pub static UNNECESSARY_NULLABLE_FOR_FINAL_VARIABLE_DECLARATIONS: CodigoLint = CodigoLint { nome: "unnecessary_nullable_for_final_variable_declarations", unico: "unnecessary_nullable_for_final_variable_declarations", mensagem: "Type could be non-nullable.", correcao: Some("Try changing the type to be non-nullable."), documentado: true };
pub static UNNECESSARY_OVERRIDES: CodigoLint = CodigoLint { nome: "unnecessary_overrides", unico: "unnecessary_overrides", mensagem: "Unnecessary override.", correcao: Some("Try adding behavior in the overriding member or removing the override."), documentado: true };
pub static UNNECESSARY_PARENTHESIS: CodigoLint = CodigoLint { nome: "unnecessary_parenthesis", unico: "unnecessary_parenthesis", mensagem: "Unnecessary use of parentheses.", correcao: Some("Try removing the parentheses."), documentado: true };
pub static UNNECESSARY_RAW_STRINGS: CodigoLint = CodigoLint { nome: "unnecessary_raw_strings", unico: "unnecessary_raw_strings", mensagem: "Unnecessary use of a raw string.", correcao: Some("Try using a normal string."), documentado: true };
pub static UNNECESSARY_STATEMENTS: CodigoLint = CodigoLint { nome: "unnecessary_statements", unico: "unnecessary_statements", mensagem: "Unnecessary statement.", correcao: Some("Try completing the statement or breaking it up."), documentado: true };
pub static UNNECESSARY_STRING_ESCAPES: CodigoLint = CodigoLint { nome: "unnecessary_string_escapes", unico: "unnecessary_string_escapes", mensagem: "Unnecessary escape in string literal.", correcao: Some("Remove the '\\' escape."), documentado: true };
pub static UNNECESSARY_STRING_INTERPOLATIONS: CodigoLint = CodigoLint { nome: "unnecessary_string_interpolations", unico: "unnecessary_string_interpolations", mensagem: "Unnecessary use of string interpolation.", correcao: Some("Try replacing the string literal with the variable name."), documentado: true };
pub static UNNECESSARY_THIS: CodigoLint = CodigoLint { nome: "unnecessary_this", unico: "unnecessary_this", mensagem: "Unnecessary 'this.' qualifier.", correcao: Some("Try removing 'this.'."), documentado: true };
pub static UNNECESSARY_TO_LIST_IN_SPREADS: CodigoLint = CodigoLint { nome: "unnecessary_to_list_in_spreads", unico: "unnecessary_to_list_in_spreads", mensagem: "Unnecessary use of 'toList' in a spread.", correcao: Some("Try removing the invocation of 'toList'."), documentado: true };
pub static UNREACHABLE_FROM_MAIN: CodigoLint = CodigoLint { nome: "unreachable_from_main", unico: "unreachable_from_main", mensagem: "Unreachable member '{0}' in an executable library.", correcao: Some("Try referencing the member or removing it."), documentado: false };
pub static UNRELATED_TYPE_EQUALITY_CHECKS_IN_EXPRESSION: CodigoLint = CodigoLint { nome: "unrelated_type_equality_checks", unico: "unrelated_type_equality_checks_in_expression", mensagem: "The type of the right operand ('{0}') isn't a subtype or a supertype of the left operand ('{1}').", correcao: Some("Try changing one or both of the operands."), documentado: true };
pub static UNRELATED_TYPE_EQUALITY_CHECKS_IN_PATTERN: CodigoLint = CodigoLint { nome: "unrelated_type_equality_checks", unico: "unrelated_type_equality_checks_in_pattern", mensagem: "The type of the operand ('{0}') isn't a subtype or a supertype of the value being matched ('{1}').", correcao: Some("Try changing one or both of the operands."), documentado: true };
pub static UNSAFE_HTML_ATTRIBUTE: CodigoLint = CodigoLint { nome: "unsafe_html", unico: "unsafe_html_attribute", mensagem: "Assigning to the attribute '{0}' is unsafe.", correcao: Some("Try finding a different way to implement the page."), documentado: false };
pub static UNSAFE_HTML_CONSTRUCTOR: CodigoLint = CodigoLint { nome: "unsafe_html", unico: "unsafe_html_constructor", mensagem: "Invoking the constructor '{0}' is unsafe.", correcao: Some("Try finding a different way to implement the page."), documentado: false };
pub static UNSAFE_HTML_METHOD: CodigoLint = CodigoLint { nome: "unsafe_html", unico: "unsafe_html_method", mensagem: "Invoking the method '{0}' is unsafe.", correcao: Some("Try finding a different way to implement the page."), documentado: false };
pub static USE_BUILD_CONTEXT_SYNCHRONOUSLY_ASYNC_USE: CodigoLint = CodigoLint { nome: "use_build_context_synchronously", unico: "use_build_context_synchronously_async_use", mensagem: "Don't use 'BuildContext's across async gaps.", correcao: Some("Try rewriting the code to not use the 'BuildContext', or guard the use with a 'mounted' check."), documentado: true };
pub static USE_BUILD_CONTEXT_SYNCHRONOUSLY_WRONG_MOUNTED: CodigoLint = CodigoLint { nome: "use_build_context_synchronously", unico: "use_build_context_synchronously_wrong_mounted", mensagem: "Don't use 'BuildContext's across async gaps, guarded by an unrelated 'mounted' check.", correcao: Some("Guard a 'State.context' use with a 'mounted' check on the State, and other BuildContext use with a 'mounted' check on the BuildContext."), documentado: true };
pub static USE_COLORED_BOX: CodigoLint = CodigoLint { nome: "use_colored_box", unico: "use_colored_box", mensagem: "Use a 'ColoredBox' rather than a 'Container' with only a 'Color'.", correcao: Some("Try replacing the 'Container' with a 'ColoredBox'."), documentado: true };
pub static USE_DECORATED_BOX: CodigoLint = CodigoLint { nome: "use_decorated_box", unico: "use_decorated_box", mensagem: "Use 'DecoratedBox' rather than a 'Container' with only a 'Decoration'.", correcao: Some("Try replacing the 'Container' with a 'DecoratedBox'."), documentado: true };
pub static USE_ENUMS: CodigoLint = CodigoLint { nome: "use_enums", unico: "use_enums", mensagem: "Class should be an enum.", correcao: Some("Try using an enum rather than a class."), documentado: false };
pub static USE_FULL_HEX_VALUES_FOR_FLUTTER_COLORS: CodigoLint = CodigoLint { nome: "use_full_hex_values_for_flutter_colors", unico: "use_full_hex_values_for_flutter_colors", mensagem: "Instances of 'Color' should be created using an 8-digit hexadecimal integer (such as '0xFFFFFFFF').", correcao: None, documentado: true };
pub static USE_FUNCTION_TYPE_SYNTAX_FOR_PARAMETERS: CodigoLint = CodigoLint { nome: "use_function_type_syntax_for_parameters", unico: "use_function_type_syntax_for_parameters", mensagem: "Use the generic function type syntax to declare the parameter '{0}'.", correcao: Some("Try using the generic function type syntax."), documentado: true };
pub static USE_IF_NULL_TO_CONVERT_NULLS_TO_BOOLS: CodigoLint = CodigoLint { nome: "use_if_null_to_convert_nulls_to_bools", unico: "use_if_null_to_convert_nulls_to_bools", mensagem: "Use an if-null operator to convert a 'null' to a 'bool'.", correcao: Some("Try using an if-null operator."), documentado: true };
pub static USE_IS_EVEN_RATHER_THAN_MODULO: CodigoLint = CodigoLint { nome: "use_is_even_rather_than_modulo", unico: "use_is_even_rather_than_modulo", mensagem: "Use '{0}' rather than '% 2'.", correcao: Some("Try using '{0}'."), documentado: false };
pub static USE_KEY_IN_WIDGET_CONSTRUCTORS: CodigoLint = CodigoLint { nome: "use_key_in_widget_constructors", unico: "use_key_in_widget_constructors", mensagem: "Constructors for public widgets should have a named 'key' parameter.", correcao: Some("Try adding a named parameter to the constructor."), documentado: true };
pub static USE_LATE_FOR_PRIVATE_FIELDS_AND_VARIABLES: CodigoLint = CodigoLint { nome: "use_late_for_private_fields_and_variables", unico: "use_late_for_private_fields_and_variables", mensagem: "Use 'late' for private members with a non-nullable type.", correcao: Some("Try making adding the modifier 'late'."), documentado: true };
pub static USE_NAMED_CONSTANTS: CodigoLint = CodigoLint { nome: "use_named_constants", unico: "use_named_constants", mensagem: "Use the constant '{0}' rather than a constructor returning the same object.", correcao: Some("Try using '{0}'."), documentado: true };
pub static USE_RAW_STRINGS: CodigoLint = CodigoLint { nome: "use_raw_strings", unico: "use_raw_strings", mensagem: "Use a raw string to avoid using escapes.", correcao: Some("Try making the string a raw string and removing the escapes."), documentado: true };
pub static USE_RETHROW_WHEN_POSSIBLE: CodigoLint = CodigoLint { nome: "use_rethrow_when_possible", unico: "use_rethrow_when_possible", mensagem: "Use 'rethrow' to rethrow a caught exception.", correcao: Some("Try replacing the 'throw' with a 'rethrow'."), documentado: true };
pub static USE_SETTERS_TO_CHANGE_PROPERTIES: CodigoLint = CodigoLint { nome: "use_setters_to_change_properties", unico: "use_setters_to_change_properties", mensagem: "The method is used to change a property.", correcao: Some("Try converting the method to a setter."), documentado: true };
pub static USE_STRING_BUFFERS: CodigoLint = CodigoLint { nome: "use_string_buffers", unico: "use_string_buffers", mensagem: "Use a string buffer rather than '+' to compose strings.", correcao: Some("Try writing the parts of a string to a string buffer."), documentado: true };
pub static USE_STRING_IN_PART_OF_DIRECTIVES: CodigoLint = CodigoLint { nome: "use_string_in_part_of_directives", unico: "use_string_in_part_of_directives", mensagem: "The part-of directive uses a library name.", correcao: Some("Try converting the directive to use the URI of the library."), documentado: true };
pub static USE_SUPER_PARAMETERS_MULTIPLE: CodigoLint = CodigoLint { nome: "use_super_parameters", unico: "use_super_parameters_multiple", mensagem: "Parameters '{0}' could be super parameters.", correcao: Some("Trying converting '{0}' to super parameters."), documentado: true };
pub static USE_SUPER_PARAMETERS_SINGLE: CodigoLint = CodigoLint { nome: "use_super_parameters", unico: "use_super_parameters_single", mensagem: "Parameter '{0}' could be a super parameter.", correcao: Some("Trying converting '{0}' to a super parameter."), documentado: true };
pub static USE_TEST_THROWS_MATCHERS: CodigoLint = CodigoLint { nome: "use_test_throws_matchers", unico: "use_test_throws_matchers", mensagem: "Use the 'throwsA' matcher instead of using 'fail' when there is no exception thrown.", correcao: Some("Try removing the try-catch and using 'throwsA' to expect an exception."), documentado: false };
pub static USE_TO_AND_AS_IF_APPLICABLE: CodigoLint = CodigoLint { nome: "use_to_and_as_if_applicable", unico: "use_to_and_as_if_applicable", mensagem: "Start the name of the method with 'to' or 'as'.", correcao: Some("Try renaming the method to use either 'to' or 'as'."), documentado: false };
pub static USE_TRUNCATING_DIVISION: CodigoLint = CodigoLint { nome: "use_truncating_division", unico: "use_truncating_division", mensagem: "Use truncating division.", correcao: Some("Try using truncating division, '~/', instead of regular division ('/') followed by 'toInt()'."), documentado: false };
pub static VALID_REGEXPS: CodigoLint = CodigoLint { nome: "valid_regexps", unico: "valid_regexps", mensagem: "Invalid regular expression syntax.", correcao: Some("Try correcting the regular expression."), documentado: true };
pub static VOID_CHECKS: CodigoLint = CodigoLint { nome: "void_checks", unico: "void_checks", mensagem: "Assignment to a variable of type 'void'.", correcao: Some("Try removing the assignment or changing the type of the variable."), documentado: true };
pub static REMOVED_LINT: CodigoLint = CodigoLint { nome: "removed_lint", unico: "removed_lint", mensagem: "Removed lint.", correcao: None, documentado: false };

/// Todos os `LinterLintCode`, na ordem da fonte.
pub static TODOS: [&CodigoLint; 260] = [
    &ALWAYS_DECLARE_RETURN_TYPES_OF_FUNCTIONS,
    &ALWAYS_DECLARE_RETURN_TYPES_OF_METHODS,
    &ALWAYS_PUT_CONTROL_BODY_ON_NEW_LINE,
    &ALWAYS_PUT_REQUIRED_NAMED_PARAMETERS_FIRST,
    &ALWAYS_SPECIFY_TYPES_ADD_TYPE,
    &ALWAYS_SPECIFY_TYPES_REPLACE_KEYWORD,
    &ALWAYS_SPECIFY_TYPES_SPECIFY_TYPE,
    &ALWAYS_SPECIFY_TYPES_SPLIT_TO_TYPES,
    &ALWAYS_USE_PACKAGE_IMPORTS,
    &ANNOTATE_OVERRIDES,
    &ANNOTATE_REDECLARES,
    &AVOID_ANNOTATING_WITH_DYNAMIC,
    &AVOID_BOOL_LITERALS_IN_CONDITIONAL_EXPRESSIONS,
    &AVOID_CATCHES_WITHOUT_ON_CLAUSES,
    &AVOID_CATCHING_ERRORS_CLASS,
    &AVOID_CATCHING_ERRORS_SUBCLASS,
    &AVOID_CLASSES_WITH_ONLY_STATIC_MEMBERS,
    &AVOID_DOUBLE_AND_INT_CHECKS,
    &AVOID_DYNAMIC_CALLS,
    &AVOID_EMPTY_ELSE,
    &AVOID_EQUALS_AND_HASH_CODE_ON_MUTABLE_CLASSES,
    &AVOID_ESCAPING_INNER_QUOTES,
    &AVOID_FIELD_INITIALIZERS_IN_CONST_CLASSES,
    &AVOID_FINAL_PARAMETERS,
    &AVOID_FUNCTION_LITERALS_IN_FOREACH_CALLS,
    &AVOID_FUTUREOR_VOID,
    &AVOID_IMPLEMENTING_VALUE_TYPES,
    &AVOID_INIT_TO_NULL,
    &AVOID_JS_ROUNDED_INTS,
    &AVOID_MULTIPLE_DECLARATIONS_PER_LINE,
    &AVOID_NULL_CHECKS_IN_EQUALITY_OPERATORS,
    &AVOID_POSITIONAL_BOOLEAN_PARAMETERS,
    &AVOID_PRINT,
    &AVOID_PRIVATE_TYPEDEF_FUNCTIONS,
    &AVOID_REDUNDANT_ARGUMENT_VALUES,
    &AVOID_RELATIVE_LIB_IMPORTS,
    &AVOID_RENAMING_METHOD_PARAMETERS,
    &AVOID_RETURN_TYPES_ON_SETTERS,
    &AVOID_RETURNING_NULL_FOR_VOID_FROM_FUNCTION,
    &AVOID_RETURNING_NULL_FOR_VOID_FROM_METHOD,
    &AVOID_RETURNING_THIS,
    &AVOID_SETTERS_WITHOUT_GETTERS,
    &AVOID_SHADOWING_TYPE_PARAMETERS,
    &AVOID_SINGLE_CASCADE_IN_EXPRESSION_STATEMENTS,
    &AVOID_SLOW_ASYNC_IO,
    &AVOID_TYPE_TO_STRING,
    &AVOID_TYPES_AS_PARAMETER_NAMES,
    &AVOID_TYPES_ON_CLOSURE_PARAMETERS,
    &AVOID_UNNECESSARY_CONTAINERS,
    &AVOID_UNUSED_CONSTRUCTOR_PARAMETERS,
    &AVOID_VOID_ASYNC,
    &AVOID_WEB_LIBRARIES_IN_FLUTTER,
    &AWAIT_ONLY_FUTURES,
    &CAMEL_CASE_EXTENSIONS,
    &CAMEL_CASE_TYPES,
    &CANCEL_SUBSCRIPTIONS,
    &CASCADE_INVOCATIONS,
    &CAST_NULLABLE_TO_NON_NULLABLE,
    &CLOSE_SINKS,
    &COLLECTION_METHODS_UNRELATED_TYPE,
    &COMBINATORS_ORDERING,
    &COMMENT_REFERENCES,
    &CONDITIONAL_URI_DOES_NOT_EXIST,
    &CONSTANT_IDENTIFIER_NAMES,
    &CONTROL_FLOW_IN_FINALLY,
    &CURLY_BRACES_IN_FLOW_CONTROL_STRUCTURES,
    &DANGLING_LIBRARY_DOC_COMMENTS,
    &DEPEND_ON_REFERENCED_PACKAGES,
    &DEPRECATED_CONSISTENCY_CONSTRUCTOR,
    &DEPRECATED_CONSISTENCY_FIELD,
    &DEPRECATED_CONSISTENCY_PARAMETER,
    &DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE_WITH_MESSAGE,
    &DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE_WITHOUT_MESSAGE,
    &DIAGNOSTIC_DESCRIBE_ALL_PROPERTIES,
    &DIRECTIVES_ORDERING_ALPHABETICAL,
    &DIRECTIVES_ORDERING_DART,
    &DIRECTIVES_ORDERING_EXPORTS,
    &DIRECTIVES_ORDERING_PACKAGE_BEFORE_RELATIVE,
    &DISCARDED_FUTURES,
    &DO_NOT_USE_ENVIRONMENT,
    &DOCUMENT_IGNORES,
    &EMPTY_CATCHES,
    &EMPTY_CONSTRUCTOR_BODIES,
    &EMPTY_STATEMENTS,
    &EOL_AT_END_OF_FILE,
    &ERASE_DART_TYPE_EXTENSION_TYPES,
    &EXHAUSTIVE_CASES,
    &FILE_NAMES,
    &FLUTTER_STYLE_TODOS,
    &HASH_AND_EQUALS,
    &IMPLEMENTATION_IMPORTS,
    &IMPLICIT_CALL_TEAROFFS,
    &IMPLICIT_REOPEN,
    &INVALID_CASE_PATTERNS,
    &INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_DART_AS_JS,
    &INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_DART_IS_JS,
    &INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_JS_AS_DART,
    &INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_JS_AS_INCOMPATIBLE_JS,
    &INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_JS_IS_DART,
    &INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_JS_IS_INCONSISTENT_JS,
    &INVALID_RUNTIME_CHECK_WITH_JS_INTEROP_TYPES_JS_IS_UNRELATED_JS,
    &JOIN_RETURN_WITH_ASSIGNMENT,
    &LEADING_NEWLINES_IN_MULTILINE_STRINGS,
    &LIBRARY_ANNOTATIONS,
    &LIBRARY_NAMES,
    &LIBRARY_PREFIXES,
    &LIBRARY_PRIVATE_TYPES_IN_PUBLIC_API,
    &LINES_LONGER_THAN_80_CHARS,
    &LITERAL_ONLY_BOOLEAN_EXPRESSIONS,
    &MATCHING_SUPER_PARAMETERS,
    &MISSING_CODE_BLOCK_LANGUAGE_IN_DOC_COMMENT,
    &MISSING_WHITESPACE_BETWEEN_ADJACENT_STRINGS,
    &NO_ADJACENT_STRINGS_IN_LIST,
    &NO_DEFAULT_CASES,
    &NO_DUPLICATE_CASE_VALUES,
    &NO_LEADING_UNDERSCORES_FOR_LIBRARY_PREFIXES,
    &NO_LEADING_UNDERSCORES_FOR_LOCAL_IDENTIFIERS,
    &NO_LITERAL_BOOL_COMPARISONS,
    &NO_LOGIC_IN_CREATE_STATE,
    &NO_RUNTIMETYPE_TOSTRING,
    &NO_SELF_ASSIGNMENTS,
    &NO_WILDCARD_VARIABLE_USES,
    &NON_CONSTANT_IDENTIFIER_NAMES,
    &NOOP_PRIMITIVE_OPERATIONS,
    &NULL_CHECK_ON_NULLABLE_TYPE_PARAMETER,
    &NULL_CLOSURES,
    &OMIT_LOCAL_VARIABLE_TYPES,
    &OMIT_OBVIOUS_LOCAL_VARIABLE_TYPES,
    &ONE_MEMBER_ABSTRACTS,
    &ONLY_THROW_ERRORS,
    &OVERRIDDEN_FIELDS,
    &PACKAGE_API_DOCS,
    &PACKAGE_NAMES,
    &PACKAGE_PREFIXED_LIBRARY_NAMES,
    &PARAMETER_ASSIGNMENTS,
    &PREFER_ADJACENT_STRING_CONCATENATION,
    &PREFER_ASSERTS_IN_INITIALIZER_LISTS,
    &PREFER_ASSERTS_WITH_MESSAGE,
    &PREFER_COLLECTION_LITERALS,
    &PREFER_CONDITIONAL_ASSIGNMENT,
    &PREFER_CONST_CONSTRUCTORS,
    &PREFER_CONST_CONSTRUCTORS_IN_IMMUTABLES,
    &PREFER_CONST_DECLARATIONS,
    &PREFER_CONST_LITERALS_TO_CREATE_IMMUTABLES,
    &PREFER_CONSTRUCTORS_OVER_STATIC_METHODS,
    &PREFER_CONTAINS_ALWAYS_FALSE,
    &PREFER_CONTAINS_ALWAYS_TRUE,
    &PREFER_CONTAINS_USE_CONTAINS,
    &PREFER_DOUBLE_QUOTES,
    &PREFER_EXPRESSION_FUNCTION_BODIES,
    &PREFER_FINAL_FIELDS,
    &PREFER_FINAL_IN_FOR_EACH_PATTERN,
    &PREFER_FINAL_IN_FOR_EACH_VARIABLE,
    &PREFER_FINAL_LOCALS,
    &PREFER_FINAL_PARAMETERS,
    &PREFER_FOR_ELEMENTS_TO_MAP_FROMITERABLE,
    &PREFER_FOREACH,
    &PREFER_FUNCTION_DECLARATIONS_OVER_VARIABLES,
    &PREFER_GENERIC_FUNCTION_TYPE_ALIASES,
    &PREFER_IF_ELEMENTS_TO_CONDITIONAL_EXPRESSIONS,
    &PREFER_IF_NULL_OPERATORS,
    &PREFER_INITIALIZING_FORMALS,
    &PREFER_INLINED_ADDS_MULTIPLE,
    &PREFER_INLINED_ADDS_SINGLE,
    &PREFER_INT_LITERALS,
    &PREFER_INTERPOLATION_TO_COMPOSE_STRINGS,
    &PREFER_IS_EMPTY_ALWAYS_FALSE,
    &PREFER_IS_EMPTY_ALWAYS_TRUE,
    &PREFER_IS_EMPTY_USE_IS_EMPTY,
    &PREFER_IS_EMPTY_USE_IS_NOT_EMPTY,
    &PREFER_IS_NOT_EMPTY,
    &PREFER_IS_NOT_OPERATOR,
    &PREFER_ITERABLE_WHERETYPE,
    &PREFER_MIXIN,
    &PREFER_NULL_AWARE_METHOD_CALLS,
    &PREFER_NULL_AWARE_OPERATORS,
    &PREFER_RELATIVE_IMPORTS,
    &PREFER_SINGLE_QUOTES,
    &PREFER_SPREAD_COLLECTIONS,
    &PREFER_TYPING_UNINITIALIZED_VARIABLES_FOR_FIELD,
    &PREFER_TYPING_UNINITIALIZED_VARIABLES_FOR_LOCAL_VARIABLE,
    &PREFER_VOID_TO_NULL,
    &PROVIDE_DEPRECATION_MESSAGE,
    &PUBLIC_MEMBER_API_DOCS,
    &RECURSIVE_GETTERS,
    &REQUIRE_TRAILING_COMMAS,
    &SECURE_PUBSPEC_URLS,
    &SIZED_BOX_FOR_WHITESPACE,
    &SIZED_BOX_SHRINK_EXPAND,
    &SLASH_FOR_DOC_COMMENTS,
    &SORT_CHILD_PROPERTIES_LAST,
    &SORT_CONSTRUCTORS_FIRST,
    &SORT_PUB_DEPENDENCIES,
    &SORT_UNNAMED_CONSTRUCTORS_FIRST,
    &SPECIFY_NONOBVIOUS_LOCAL_VARIABLE_TYPES,
    &TEST_TYPES_IN_EQUALS,
    &THROW_IN_FINALLY,
    &TIGHTEN_TYPE_OF_INITIALIZING_FORMALS,
    &TYPE_ANNOTATE_PUBLIC_APIS,
    &TYPE_INIT_FORMALS,
    &TYPE_LITERAL_IN_CONSTANT_PATTERN,
    &UNAWAITED_FUTURES,
    &UNINTENDED_HTML_IN_DOC_COMMENT,
    &UNNECESSARY_AWAIT_IN_RETURN,
    &UNNECESSARY_BRACE_IN_STRING_INTERPS,
    &UNNECESSARY_BREAKS,
    &UNNECESSARY_CONST,
    &UNNECESSARY_CONSTRUCTOR_NAME,
    &UNNECESSARY_FINAL_WITH_TYPE,
    &UNNECESSARY_FINAL_WITHOUT_TYPE,
    &UNNECESSARY_GETTERS_SETTERS,
    &UNNECESSARY_LAMBDAS,
    &UNNECESSARY_LATE,
    &UNNECESSARY_LIBRARY_DIRECTIVE,
    &UNNECESSARY_LIBRARY_NAME,
    &UNNECESSARY_NEW,
    &UNNECESSARY_NULL_AWARE_ASSIGNMENTS,
    &UNNECESSARY_NULL_AWARE_OPERATOR_ON_EXTENSION_ON_NULLABLE,
    &UNNECESSARY_NULL_CHECKS,
    &UNNECESSARY_NULL_IN_IF_NULL_OPERATORS,
    &UNNECESSARY_NULLABLE_FOR_FINAL_VARIABLE_DECLARATIONS,
    &UNNECESSARY_OVERRIDES,
    &UNNECESSARY_PARENTHESIS,
    &UNNECESSARY_RAW_STRINGS,
    &UNNECESSARY_STATEMENTS,
    &UNNECESSARY_STRING_ESCAPES,
    &UNNECESSARY_STRING_INTERPOLATIONS,
    &UNNECESSARY_THIS,
    &UNNECESSARY_TO_LIST_IN_SPREADS,
    &UNREACHABLE_FROM_MAIN,
    &UNRELATED_TYPE_EQUALITY_CHECKS_IN_EXPRESSION,
    &UNRELATED_TYPE_EQUALITY_CHECKS_IN_PATTERN,
    &UNSAFE_HTML_ATTRIBUTE,
    &UNSAFE_HTML_CONSTRUCTOR,
    &UNSAFE_HTML_METHOD,
    &USE_BUILD_CONTEXT_SYNCHRONOUSLY_ASYNC_USE,
    &USE_BUILD_CONTEXT_SYNCHRONOUSLY_WRONG_MOUNTED,
    &USE_COLORED_BOX,
    &USE_DECORATED_BOX,
    &USE_ENUMS,
    &USE_FULL_HEX_VALUES_FOR_FLUTTER_COLORS,
    &USE_FUNCTION_TYPE_SYNTAX_FOR_PARAMETERS,
    &USE_IF_NULL_TO_CONVERT_NULLS_TO_BOOLS,
    &USE_IS_EVEN_RATHER_THAN_MODULO,
    &USE_KEY_IN_WIDGET_CONSTRUCTORS,
    &USE_LATE_FOR_PRIVATE_FIELDS_AND_VARIABLES,
    &USE_NAMED_CONSTANTS,
    &USE_RAW_STRINGS,
    &USE_RETHROW_WHEN_POSSIBLE,
    &USE_SETTERS_TO_CHANGE_PROPERTIES,
    &USE_STRING_BUFFERS,
    &USE_STRING_IN_PART_OF_DIRECTIVES,
    &USE_SUPER_PARAMETERS_MULTIPLE,
    &USE_SUPER_PARAMETERS_SINGLE,
    &USE_TEST_THROWS_MATCHERS,
    &USE_TO_AND_AS_IF_APPLICABLE,
    &USE_TRUNCATING_DIVISION,
    &VALID_REGEXPS,
    &VOID_CHECKS,
    &REMOVED_LINT,
];
