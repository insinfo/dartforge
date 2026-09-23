// GERADO por `cargo run -p dartforge-paridade --example gerar_codigos`. NÃO EDITE.
// Fonte: analyzer-6.11.0 e _fe_analyzer_shared-76.0.0 (SDK Dart 3.6), cache do pub.
// CompileTimeErrorCode: 542
// StaticWarningCode: 7
// WarningCode: 144
// HintCode: 8
// FfiCode: 48
// ParserErrorCode: 265
// ScannerErrorCode: 12
// TodoCode: 4
#![allow(missing_docs)]

use crate::{InfoCodigo, Severidade, TipoErro};

pub(crate) static TABELA: [InfoCodigo; 1030] = [
    InfoCodigo { nome: "abstract_field_initializer", unico: "CompileTimeErrorCode.ABSTRACT_FIELD_CONSTRUCTOR_INITIALIZER", mensagem: "Abstract fields can't have initializers.", correcao: Some("Try removing the field initializer or the 'abstract' keyword from the field declaration."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "abstract_field_initializer", unico: "CompileTimeErrorCode.ABSTRACT_FIELD_INITIALIZER", mensagem: "Abstract fields can't have initializers.", correcao: Some("Try removing the initializer or the 'abstract' keyword."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "abstract_super_member_reference", unico: "CompileTimeErrorCode.ABSTRACT_SUPER_MEMBER_REFERENCE", mensagem: "The {0} '{1}' is always abstract in the supertype.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "ambiguous_export", unico: "CompileTimeErrorCode.AMBIGUOUS_EXPORT", mensagem: "The name '{0}' is defined in the libraries '{1}' and '{2}'.", correcao: Some("Try removing the export of one of the libraries, or explicitly hiding the name in one of the export directives."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "ambiguous_extension_member_access", unico: "CompileTimeErrorCode.AMBIGUOUS_EXTENSION_MEMBER_ACCESS", mensagem: "A member named '{0}' is defined in {1}, and none are more specific.", correcao: Some("Try using an extension override to specify the extension you want to be chosen."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "ambiguous_import", unico: "CompileTimeErrorCode.AMBIGUOUS_IMPORT", mensagem: "The name '{0}' is defined in the libraries {1}.", correcao: Some("Try using 'as prefix' for one of the import directives, or hiding the name from all but one of the imports."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "ambiguous_set_or_map_literal_both", unico: "CompileTimeErrorCode.AMBIGUOUS_SET_OR_MAP_LITERAL_BOTH", mensagem: "The literal can't be either a map or a set because it contains at least one literal map entry or a spread operator spreading a 'Map', and at least one element which is neither of these.", correcao: Some("Try removing or changing some of the elements so that all of the elements are consistent."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "ambiguous_set_or_map_literal_either", unico: "CompileTimeErrorCode.AMBIGUOUS_SET_OR_MAP_LITERAL_EITHER", mensagem: "This literal must be either a map or a set, but the elements don't have enough information for type inference to work.", correcao: Some("Try adding type arguments to the literal (one for sets, two for maps)."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "argument_type_not_assignable", unico: "CompileTimeErrorCode.ARGUMENT_TYPE_NOT_ASSIGNABLE", mensagem: "The argument type '{0}' can't be assigned to the parameter type '{1}'. {2}", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "assert_in_redirecting_constructor", unico: "CompileTimeErrorCode.ASSERT_IN_REDIRECTING_CONSTRUCTOR", mensagem: "A redirecting constructor can't have an 'assert' initializer.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "assignment_to_const", unico: "CompileTimeErrorCode.ASSIGNMENT_TO_CONST", mensagem: "Constant variables can't be assigned a value.", correcao: Some("Try removing the assignment, or remove the modifier 'const' from the variable."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "assignment_to_final", unico: "CompileTimeErrorCode.ASSIGNMENT_TO_FINAL", mensagem: "'{0}' can't be used as a setter because it's final.", correcao: Some("Try finding a different setter, or making '{0}' non-final."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "assignment_to_final_local", unico: "CompileTimeErrorCode.ASSIGNMENT_TO_FINAL_LOCAL", mensagem: "The final variable '{0}' can only be set once.", correcao: Some("Try making '{0}' non-final."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "assignment_to_final_no_setter", unico: "CompileTimeErrorCode.ASSIGNMENT_TO_FINAL_NO_SETTER", mensagem: "There isn't a setter named '{0}' in class '{1}'.", correcao: Some("Try correcting the name to reference an existing setter, or declare the setter."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "assignment_to_function", unico: "CompileTimeErrorCode.ASSIGNMENT_TO_FUNCTION", mensagem: "Functions can't be assigned a value.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "assignment_to_method", unico: "CompileTimeErrorCode.ASSIGNMENT_TO_METHOD", mensagem: "Methods can't be assigned a value.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "assignment_to_type", unico: "CompileTimeErrorCode.ASSIGNMENT_TO_TYPE", mensagem: "Types can't be assigned a value.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "async_for_in_wrong_context", unico: "CompileTimeErrorCode.ASYNC_FOR_IN_WRONG_CONTEXT", mensagem: "The async for-in loop can only be used in an async function.", correcao: Some("Try marking the function body with either 'async' or 'async*', or removing the 'await' before the for-in loop."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "augmentation_extends_clause_already_present", unico: "CompileTimeErrorCode.AUGMENTATION_EXTENDS_CLAUSE_ALREADY_PRESENT", mensagem: "The augmentation has an 'extends' clause, but an augmentation target already includes an 'extends' clause and it isn't allowed to be repeated or changed.", correcao: Some("Try removing the 'extends' clause, either here or in the augmentation target."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "augmentation_modifier_extra", unico: "CompileTimeErrorCode.AUGMENTATION_MODIFIER_EXTRA", mensagem: "The augmentation has the '{0}' modifier that the declaration doesn't have.", correcao: Some("Try removing the '{0}' modifier, or adding it to the declaration."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "augmentation_modifier_missing", unico: "CompileTimeErrorCode.AUGMENTATION_MODIFIER_MISSING", mensagem: "The augmentation is missing the '{0}' modifier that the declaration has.", correcao: Some("Try adding the '{0}' modifier, or removing it from the declaration."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "augmentation_of_different_declaration_kind", unico: "CompileTimeErrorCode.AUGMENTATION_OF_DIFFERENT_DECLARATION_KIND", mensagem: "Can't augment a {0} with a {1}.", correcao: Some("Try changing the augmentation to match the declaration kind."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "augmentation_type_parameter_bound", unico: "CompileTimeErrorCode.AUGMENTATION_TYPE_PARAMETER_BOUND", mensagem: "The augmentation type parameter must have the same bound as the corresponding type parameter of the declaration.", correcao: Some("Try changing the augmentation to match the declaration type parameters."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "augmentation_type_parameter_count", unico: "CompileTimeErrorCode.AUGMENTATION_TYPE_PARAMETER_COUNT", mensagem: "The augmentation must have the same number of type parameters as the declaration.", correcao: Some("Try changing the augmentation to match the declaration type parameters."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "augmentation_type_parameter_name", unico: "CompileTimeErrorCode.AUGMENTATION_TYPE_PARAMETER_NAME", mensagem: "The augmentation type parameter must have the same name as the corresponding type parameter of the declaration.", correcao: Some("Try changing the augmentation to match the declaration type parameters."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "augmentation_without_declaration", unico: "CompileTimeErrorCode.AUGMENTATION_WITHOUT_DECLARATION", mensagem: "The declaration being augmented doesn't exist.", correcao: Some("Try changing the augmentation to match an existing declaration."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "augmented_expression_is_not_setter", unico: "CompileTimeErrorCode.AUGMENTED_EXPRESSION_IS_NOT_SETTER", mensagem: "The augmented declaration is not a setter, it can't be used to write a value.", correcao: Some("Try assigning a value to a setter."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "augmented_expression_is_setter", unico: "CompileTimeErrorCode.AUGMENTED_EXPRESSION_IS_SETTER", mensagem: "The augmented declaration is a setter, it can't be used to read a value.", correcao: Some("Try assigning a value to the augmented setter."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "augmented_expression_not_operator", unico: "CompileTimeErrorCode.AUGMENTED_EXPRESSION_NOT_OPERATOR", mensagem: "The enclosing augmentation doesn't augment the operator '{0}'.", correcao: Some("Try augmenting or invoking the correct operator."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "await_in_late_local_variable_initializer", unico: "CompileTimeErrorCode.AWAIT_IN_LATE_LOCAL_VARIABLE_INITIALIZER", mensagem: "The 'await' expression can't be used in a 'late' local variable's initializer.", correcao: Some("Try removing the 'late' modifier, or rewriting the initializer without using the 'await' expression."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "await_in_wrong_context", unico: "CompileTimeErrorCode.AWAIT_IN_WRONG_CONTEXT", mensagem: "The await expression can only be used in an async function.", correcao: Some("Try marking the function body with either 'async' or 'async*'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "await_of_incompatible_type", unico: "CompileTimeErrorCode.AWAIT_OF_INCOMPATIBLE_TYPE", mensagem: "The 'await' expression can't be used for an expression with an extension type that is not a subtype of 'Future'.", correcao: Some("Try removing the `await`, or updating the extension type to implement 'Future'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_use_of_type_outside_library", unico: "CompileTimeErrorCode.BASE_CLASS_IMPLEMENTED_OUTSIDE_OF_LIBRARY", mensagem: "The class '{0}' can't be implemented outside of its library because it's a base class.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_use_of_type_outside_library", unico: "CompileTimeErrorCode.BASE_MIXIN_IMPLEMENTED_OUTSIDE_OF_LIBRARY", mensagem: "The mixin '{0}' can't be implemented outside of its library because it's a base mixin.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "body_might_complete_normally", unico: "CompileTimeErrorCode.BODY_MIGHT_COMPLETE_NORMALLY", mensagem: "The body might complete normally, causing 'null' to be returned, but the return type, '{0}', is a potentially non-nullable type.", correcao: Some("Try adding either a return or a throw statement at the end."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "break_label_on_switch_member", unico: "CompileTimeErrorCode.BREAK_LABEL_ON_SWITCH_MEMBER", mensagem: "A break label resolves to the 'case' or 'default' statement.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "built_in_identifier_in_declaration", unico: "CompileTimeErrorCode.BUILT_IN_IDENTIFIER_AS_EXTENSION_NAME", mensagem: "The built-in identifier '{0}' can't be used as an extension name.", correcao: Some("Try choosing a different name for the extension."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "built_in_identifier_in_declaration", unico: "CompileTimeErrorCode.BUILT_IN_IDENTIFIER_AS_EXTENSION_TYPE_NAME", mensagem: "The built-in identifier '{0}' can't be used as an extension type name.", correcao: Some("Try choosing a different name for the extension type."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "built_in_identifier_in_declaration", unico: "CompileTimeErrorCode.BUILT_IN_IDENTIFIER_AS_PREFIX_NAME", mensagem: "The built-in identifier '{0}' can't be used as a prefix name.", correcao: Some("Try choosing a different name for the prefix."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "built_in_identifier_as_type", unico: "CompileTimeErrorCode.BUILT_IN_IDENTIFIER_AS_TYPE", mensagem: "The built-in identifier '{0}' can't be used as a type.", correcao: Some("Try correcting the name to match an existing type."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "built_in_identifier_in_declaration", unico: "CompileTimeErrorCode.BUILT_IN_IDENTIFIER_AS_TYPEDEF_NAME", mensagem: "The built-in identifier '{0}' can't be used as a typedef name.", correcao: Some("Try choosing a different name for the typedef."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "built_in_identifier_in_declaration", unico: "CompileTimeErrorCode.BUILT_IN_IDENTIFIER_AS_TYPE_NAME", mensagem: "The built-in identifier '{0}' can't be used as a type name.", correcao: Some("Try choosing a different name for the type."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "built_in_identifier_in_declaration", unico: "CompileTimeErrorCode.BUILT_IN_IDENTIFIER_AS_TYPE_PARAMETER_NAME", mensagem: "The built-in identifier '{0}' can't be used as a type parameter name.", correcao: Some("Try choosing a different name for the type parameter."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "case_expression_type_implements_equals", unico: "CompileTimeErrorCode.CASE_EXPRESSION_TYPE_IMPLEMENTS_EQUALS", mensagem: "The switch case expression type '{0}' can't override the '==' operator.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "case_expression_type_is_not_switch_expression_subtype", unico: "CompileTimeErrorCode.CASE_EXPRESSION_TYPE_IS_NOT_SWITCH_EXPRESSION_SUBTYPE", mensagem: "The switch case expression type '{0}' must be a subtype of the switch expression type '{1}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "cast_to_non_type", unico: "CompileTimeErrorCode.CAST_TO_NON_TYPE", mensagem: "The name '{0}' isn't a type, so it can't be used in an 'as' expression.", correcao: Some("Try changing the name to the name of an existing type, or creating a type with the name '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "class_instantiation_access_to_member", unico: "CompileTimeErrorCode.CLASS_INSTANTIATION_ACCESS_TO_INSTANCE_MEMBER", mensagem: "The instance member '{0}' can't be accessed on a class instantiation.", correcao: Some("Try changing the member name to the name of a constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "class_instantiation_access_to_member", unico: "CompileTimeErrorCode.CLASS_INSTANTIATION_ACCESS_TO_STATIC_MEMBER", mensagem: "The static member '{0}' can't be accessed on a class instantiation.", correcao: Some("Try removing the type arguments from the class name, or changing the member name to the name of a constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "class_instantiation_access_to_member", unico: "CompileTimeErrorCode.CLASS_INSTANTIATION_ACCESS_TO_UNKNOWN_MEMBER", mensagem: "The class '{0}' doesn't have a constructor named '{1}'.", correcao: Some("Try invoking a different constructor, or defining a constructor named '{1}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "class_used_as_mixin", unico: "CompileTimeErrorCode.CLASS_USED_AS_MIXIN", mensagem: "The class '{0}' can't be used as a mixin because it's neither a mixin class nor a mixin.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "concrete_class_has_enum_superinterface", unico: "CompileTimeErrorCode.CONCRETE_CLASS_HAS_ENUM_SUPERINTERFACE", mensagem: "Concrete classes can't have 'Enum' as a superinterface.", correcao: Some("Try specifying a different interface, or remove it from the list."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "concrete_class_with_abstract_member", unico: "CompileTimeErrorCode.CONCRETE_CLASS_WITH_ABSTRACT_MEMBER", mensagem: "'{0}' must have a method body because '{1}' isn't abstract.", correcao: Some("Try making '{1}' abstract, or adding a body to '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "conflicting_constructor_and_static_member", unico: "CompileTimeErrorCode.CONFLICTING_CONSTRUCTOR_AND_STATIC_FIELD", mensagem: "'{0}' can't be used to name both a constructor and a static field in this class.", correcao: Some("Try renaming either the constructor or the field."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "conflicting_constructor_and_static_member", unico: "CompileTimeErrorCode.CONFLICTING_CONSTRUCTOR_AND_STATIC_GETTER", mensagem: "'{0}' can't be used to name both a constructor and a static getter in this class.", correcao: Some("Try renaming either the constructor or the getter."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "conflicting_constructor_and_static_member", unico: "CompileTimeErrorCode.CONFLICTING_CONSTRUCTOR_AND_STATIC_METHOD", mensagem: "'{0}' can't be used to name both a constructor and a static method in this class.", correcao: Some("Try renaming either the constructor or the method."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "conflicting_constructor_and_static_member", unico: "CompileTimeErrorCode.CONFLICTING_CONSTRUCTOR_AND_STATIC_SETTER", mensagem: "'{0}' can't be used to name both a constructor and a static setter in this class.", correcao: Some("Try renaming either the constructor or the setter."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "conflicting_field_and_method", unico: "CompileTimeErrorCode.CONFLICTING_FIELD_AND_METHOD", mensagem: "Class '{0}' can't define field '{1}' and have method '{2}.{1}' with the same name.", correcao: Some("Try converting the getter to a method, or renaming the field to a name that doesn't conflict."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "conflicting_generic_interfaces", unico: "CompileTimeErrorCode.CONFLICTING_GENERIC_INTERFACES", mensagem: "The {0} '{1}' can't implement both '{2}' and '{3}' because the type arguments are different.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "conflicting_inherited_method_and_setter", unico: "CompileTimeErrorCode.CONFLICTING_INHERITED_METHOD_AND_SETTER", mensagem: "The {0} '{1}' can't inherit both a method and a setter named '{2}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "conflicting_method_and_field", unico: "CompileTimeErrorCode.CONFLICTING_METHOD_AND_FIELD", mensagem: "Class '{0}' can't define method '{1}' and have field '{2}.{1}' with the same name.", correcao: Some("Try converting the method to a getter, or renaming the method to a name that doesn't conflict."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "conflicting_static_and_instance", unico: "CompileTimeErrorCode.CONFLICTING_STATIC_AND_INSTANCE", mensagem: "Class '{0}' can't define static member '{1}' and have instance member '{2}.{1}' with the same name.", correcao: Some("Try renaming the member to a name that doesn't conflict."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "conflicting_type_variable_and_container", unico: "CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_CLASS", mensagem: "'{0}' can't be used to name both a type parameter and the class in which the type parameter is defined.", correcao: Some("Try renaming either the type parameter or the class."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "conflicting_type_variable_and_container", unico: "CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_ENUM", mensagem: "'{0}' can't be used to name both a type parameter and the enum in which the type parameter is defined.", correcao: Some("Try renaming either the type parameter or the enum."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "conflicting_type_variable_and_container", unico: "CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_EXTENSION", mensagem: "'{0}' can't be used to name both a type parameter and the extension in which the type parameter is defined.", correcao: Some("Try renaming either the type variaparameterble or the extension."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "conflicting_type_variable_and_container", unico: "CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_EXTENSION_TYPE", mensagem: "'{0}' can't be used to name both a type parameter and the extension type in which the type parameter is defined.", correcao: Some("Try renaming either the type parameter or the extension."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "conflicting_type_variable_and_member", unico: "CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_MEMBER_CLASS", mensagem: "'{0}' can't be used to name both a type parameter and a member in this class.", correcao: Some("Try renaming either the type parameter or the member."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "conflicting_type_variable_and_member", unico: "CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_MEMBER_ENUM", mensagem: "'{0}' can't be used to name both a type parameter and a member in this enum.", correcao: Some("Try renaming either the type parameter or the member."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "conflicting_type_variable_and_member", unico: "CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_MEMBER_EXTENSION", mensagem: "'{0}' can't be used to name both a type parameter and a member in this extension.", correcao: Some("Try renaming either the type parameter or the member."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "conflicting_type_variable_and_member", unico: "CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_MEMBER_EXTENSION_TYPE", mensagem: "'{0}' can't be used to name both a type parameter and a member in this extension type.", correcao: Some("Try renaming either the type parameter or the member."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "conflicting_type_variable_and_member", unico: "CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_MEMBER_MIXIN", mensagem: "'{0}' can't be used to name both a type parameter and a member in this mixin.", correcao: Some("Try renaming either the type parameter or the member."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "conflicting_type_variable_and_container", unico: "CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_MIXIN", mensagem: "'{0}' can't be used to name both a type parameter and the mixin in which the type parameter is defined.", correcao: Some("Try renaming either the type parameter or the mixin."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "constant_pattern_with_non_constant_expression", unico: "CompileTimeErrorCode.CONSTANT_PATTERN_WITH_NON_CONSTANT_EXPRESSION", mensagem: "The expression of a constant pattern must be a valid constant.", correcao: Some("Try making the expression a valid constant."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "collection_element_from_deferred_library", unico: "CompileTimeErrorCode.CONST_CONSTRUCTOR_CONSTANT_FROM_DEFERRED_LIBRARY", mensagem: "Constant values from a deferred library can't be used as values in a 'const' constructor.", correcao: Some("Try removing the keyword 'const' from the constructor or removing the keyword 'deferred' from the import."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "const_constructor_field_type_mismatch", unico: "CompileTimeErrorCode.CONST_CONSTRUCTOR_FIELD_TYPE_MISMATCH", mensagem: "In a const constructor, a value of type '{0}' can't be assigned to the field '{1}', which has type '{2}'.", correcao: Some("Try using a subtype, or removing the keyword 'const'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_constructor_param_type_mismatch", unico: "CompileTimeErrorCode.CONST_CONSTRUCTOR_PARAM_TYPE_MISMATCH", mensagem: "A value of type '{0}' can't be assigned to a parameter of type '{1}' in a const constructor.", correcao: Some("Try using a subtype, or removing the keyword 'const'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "const_constructor_throws_exception", unico: "CompileTimeErrorCode.CONST_CONSTRUCTOR_THROWS_EXCEPTION", mensagem: "Const constructors can't throw exceptions.", correcao: Some("Try removing the throw statement, or removing the keyword 'const'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_constructor_with_field_initialized_by_non_const", unico: "CompileTimeErrorCode.CONST_CONSTRUCTOR_WITH_FIELD_INITIALIZED_BY_NON_CONST", mensagem: "Can't define the 'const' constructor because the field '{0}' is initialized with a non-constant value.", correcao: Some("Try initializing the field to a constant value, or removing the keyword 'const' from the constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "const_constructor_with_mixin_with_field", unico: "CompileTimeErrorCode.CONST_CONSTRUCTOR_WITH_MIXIN_WITH_FIELD", mensagem: "This constructor can't be declared 'const' because a mixin adds the instance field: {0}.", correcao: Some("Try removing the 'const' keyword or removing the 'with' clause from the class declaration, or removing the field from the mixin class."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_constructor_with_mixin_with_field", unico: "CompileTimeErrorCode.CONST_CONSTRUCTOR_WITH_MIXIN_WITH_FIELDS", mensagem: "This constructor can't be declared 'const' because the mixins add the instance fields: {0}.", correcao: Some("Try removing the 'const' keyword or removing the 'with' clause from the class declaration, or removing the fields from the mixin classes."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_constructor_with_non_const_super", unico: "CompileTimeErrorCode.CONST_CONSTRUCTOR_WITH_NON_CONST_SUPER", mensagem: "A constant constructor can't call a non-constant super constructor of '{0}'.", correcao: Some("Try calling a constant constructor in the superclass, or removing the keyword 'const' from the constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "const_constructor_with_non_final_field", unico: "CompileTimeErrorCode.CONST_CONSTRUCTOR_WITH_NON_FINAL_FIELD", mensagem: "Can't define a const constructor for a class with non-final fields.", correcao: Some("Try making all of the fields final, or removing the keyword 'const' from the constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "const_deferred_class", unico: "CompileTimeErrorCode.CONST_DEFERRED_CLASS", mensagem: "Deferred classes can't be created with 'const'.", correcao: Some("Try using 'new' to create the instance, or changing the import to not be deferred."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "const_eval_assertion_failure", unico: "CompileTimeErrorCode.CONST_EVAL_ASSERTION_FAILURE", mensagem: "The assertion in this constant expression failed.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_eval_assertion_failure_with_message", unico: "CompileTimeErrorCode.CONST_EVAL_ASSERTION_FAILURE_WITH_MESSAGE", mensagem: "An assertion failed with message '{0}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_eval_extension_method", unico: "CompileTimeErrorCode.CONST_EVAL_EXTENSION_METHOD", mensagem: "Extension methods can't be used in constant expressions.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_eval_extension_type_method", unico: "CompileTimeErrorCode.CONST_EVAL_EXTENSION_TYPE_METHOD", mensagem: "Extension type methods can't be used in constant expressions.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_eval_for_element", unico: "CompileTimeErrorCode.CONST_EVAL_FOR_ELEMENT", mensagem: "Constant expressions don't support 'for' elements.", correcao: Some("Try replacing the 'for' element with a spread, or removing 'const'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_eval_method_invocation", unico: "CompileTimeErrorCode.CONST_EVAL_METHOD_INVOCATION", mensagem: "Methods can't be invoked in constant expressions.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_eval_property_access", unico: "CompileTimeErrorCode.CONST_EVAL_PROPERTY_ACCESS", mensagem: "The property '{0}' can't be accessed on the type '{1}' in a constant expression.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_eval_throws_exception", unico: "CompileTimeErrorCode.CONST_EVAL_THROWS_EXCEPTION", mensagem: "Evaluation of this constant expression throws an exception.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_eval_throws_idbze", unico: "CompileTimeErrorCode.CONST_EVAL_THROWS_IDBZE", mensagem: "Evaluation of this constant expression throws an IntegerDivisionByZeroException.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_eval_type_bool", unico: "CompileTimeErrorCode.CONST_EVAL_TYPE_BOOL", mensagem: "In constant expressions, operands of this operator must be of type 'bool'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_eval_type_bool_int", unico: "CompileTimeErrorCode.CONST_EVAL_TYPE_BOOL_INT", mensagem: "In constant expressions, operands of this operator must be of type 'bool' or 'int'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_eval_type_bool_num_string", unico: "CompileTimeErrorCode.CONST_EVAL_TYPE_BOOL_NUM_STRING", mensagem: "In constant expressions, operands of this operator must be of type 'bool', 'num', 'String' or 'null'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_eval_type_int", unico: "CompileTimeErrorCode.CONST_EVAL_TYPE_INT", mensagem: "In constant expressions, operands of this operator must be of type 'int'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_eval_type_num", unico: "CompileTimeErrorCode.CONST_EVAL_TYPE_NUM", mensagem: "In constant expressions, operands of this operator must be of type 'num'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_eval_type_num_string", unico: "CompileTimeErrorCode.CONST_EVAL_TYPE_NUM_STRING", mensagem: "In constant expressions, operands of this operator must be of type 'num' or 'String'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_eval_type_string", unico: "CompileTimeErrorCode.CONST_EVAL_TYPE_STRING", mensagem: "In constant expressions, operands of this operator must be of type 'String'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_eval_type_type", unico: "CompileTimeErrorCode.CONST_EVAL_TYPE_TYPE", mensagem: "In constant expressions, operands of this operator must be of type 'Type'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "field_initializer_not_assignable", unico: "CompileTimeErrorCode.CONST_FIELD_INITIALIZER_NOT_ASSIGNABLE", mensagem: "The initializer type '{0}' can't be assigned to the field type '{1}' in a const constructor.", correcao: Some("Try using a subtype, or removing the 'const' keyword"), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "const_initialized_with_non_constant_value", unico: "CompileTimeErrorCode.CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE", mensagem: "Const variables must be initialized with a constant value.", correcao: Some("Try changing the initializer to be a constant expression."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "const_initialized_with_non_constant_value_from_deferred_library", unico: "CompileTimeErrorCode.CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE_FROM_DEFERRED_LIBRARY", mensagem: "Constant values from a deferred library can't be used to initialize a 'const' variable.", correcao: Some("Try initializing the variable without referencing members of the deferred library, or changing the import to not be deferred."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "const_instance_field", unico: "CompileTimeErrorCode.CONST_INSTANCE_FIELD", mensagem: "Only static fields can be declared as const.", correcao: Some("Try declaring the field as final, or adding the keyword 'static'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "const_map_key_not_primitive_equality", unico: "CompileTimeErrorCode.CONST_MAP_KEY_NOT_PRIMITIVE_EQUALITY", mensagem: "The type of a key in a constant map can't override the '==' operator, or 'hashCode', but the class '{0}' does.", correcao: Some("Try using a different value for the key, or removing the keyword 'const' from the map."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "const_not_initialized", unico: "CompileTimeErrorCode.CONST_NOT_INITIALIZED", mensagem: "The constant '{0}' must be initialized.", correcao: Some("Try adding an initialization to the declaration."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "const_set_element_not_primitive_equality", unico: "CompileTimeErrorCode.CONST_SET_ELEMENT_NOT_PRIMITIVE_EQUALITY", mensagem: "An element in a constant set can't override the '==' operator, or 'hashCode', but the type '{0}' does.", correcao: Some("Try using a different value for the element, or removing the keyword 'const' from the set."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "const_spread_expected_list_or_set", unico: "CompileTimeErrorCode.CONST_SPREAD_EXPECTED_LIST_OR_SET", mensagem: "A list or a set is expected in this spread.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "const_spread_expected_map", unico: "CompileTimeErrorCode.CONST_SPREAD_EXPECTED_MAP", mensagem: "A map is expected in this spread.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "const_type_parameter", unico: "CompileTimeErrorCode.CONST_TYPE_PARAMETER", mensagem: "Type parameters can't be used in a constant expression.", correcao: Some("Try replacing the type parameter with a different type."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_with_non_const", unico: "CompileTimeErrorCode.CONST_WITH_NON_CONST", mensagem: "The constructor being called isn't a const constructor.", correcao: Some("Try removing 'const' from the constructor invocation."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "const_with_non_constant_argument", unico: "CompileTimeErrorCode.CONST_WITH_NON_CONSTANT_ARGUMENT", mensagem: "Arguments of a constant creation must be constant expressions.", correcao: Some("Try making the argument a valid constant, or use 'new' to call the constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "creation_with_non_type", unico: "CompileTimeErrorCode.CONST_WITH_NON_TYPE", mensagem: "The name '{0}' isn't a class.", correcao: Some("Try correcting the name to match an existing class."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "const_with_type_parameters", unico: "CompileTimeErrorCode.CONST_WITH_TYPE_PARAMETERS", mensagem: "A constant creation can't use a type parameter as a type argument.", correcao: Some("Try replacing the type parameter with a different type."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "const_with_type_parameters", unico: "CompileTimeErrorCode.CONST_WITH_TYPE_PARAMETERS_CONSTRUCTOR_TEAROFF", mensagem: "A constant constructor tearoff can't use a type parameter as a type argument.", correcao: Some("Try replacing the type parameter with a different type."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "const_with_type_parameters", unico: "CompileTimeErrorCode.CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF", mensagem: "A constant function tearoff can't use a type parameter as a type argument.", correcao: Some("Try replacing the type parameter with a different type."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "const_with_undefined_constructor", unico: "CompileTimeErrorCode.CONST_WITH_UNDEFINED_CONSTRUCTOR", mensagem: "The class '{0}' doesn't have a constant constructor '{1}'.", correcao: Some("Try calling a different constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_with_undefined_constructor_default", unico: "CompileTimeErrorCode.CONST_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT", mensagem: "The class '{0}' doesn't have an unnamed constant constructor.", correcao: Some("Try calling a different constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "continue_label_invalid", unico: "CompileTimeErrorCode.CONTINUE_LABEL_INVALID", mensagem: "The label used in a 'continue' statement must be defined on either a loop or a switch member.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "could_not_infer", unico: "CompileTimeErrorCode.COULD_NOT_INFER", mensagem: "Couldn't infer type parameter '{0}'.{1}", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "default_value_in_redirecting_factory_constructor", unico: "CompileTimeErrorCode.DEFAULT_VALUE_IN_REDIRECTING_FACTORY_CONSTRUCTOR", mensagem: "Default values aren't allowed in factory constructors that redirect to another constructor.", correcao: Some("Try removing the default value."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "default_value_on_required_parameter", unico: "CompileTimeErrorCode.DEFAULT_VALUE_ON_REQUIRED_PARAMETER", mensagem: "Required named parameters can't have a default value.", correcao: Some("Try removing either the default value or the 'required' modifier."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "deferred_import_of_extension", unico: "CompileTimeErrorCode.DEFERRED_IMPORT_OF_EXTENSION", mensagem: "Imports of deferred libraries must hide all extensions.", correcao: Some("Try adding either a show combinator listing the names you need to reference or a hide combinator listing all of the extensions."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "definitely_unassigned_late_local_variable", unico: "CompileTimeErrorCode.DEFINITELY_UNASSIGNED_LATE_LOCAL_VARIABLE", mensagem: "The late local variable '{0}' is definitely unassigned at this point.", correcao: Some("Ensure that it is assigned on necessary execution paths."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "disallowed_type_instantiation_expression", unico: "CompileTimeErrorCode.DISALLOWED_TYPE_INSTANTIATION_EXPRESSION", mensagem: "Only a generic type, generic function, generic instance method, or generic constructor can have type arguments.", correcao: Some("Try removing the type arguments, or instantiating the type(s) of a generic type, generic function, generic instance method, or generic constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "duplicate_constructor", unico: "CompileTimeErrorCode.DUPLICATE_CONSTRUCTOR_DEFAULT", mensagem: "The unnamed constructor is already defined.", correcao: Some("Try giving one of the constructors a name."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "duplicate_constructor", unico: "CompileTimeErrorCode.DUPLICATE_CONSTRUCTOR_NAME", mensagem: "The constructor with name '{0}' is already defined.", correcao: Some("Try renaming one of the constructors."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "duplicate_definition", unico: "CompileTimeErrorCode.DUPLICATE_DEFINITION", mensagem: "The name '{0}' is already defined.", correcao: Some("Try renaming one of the declarations."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "duplicate_field_formal_parameter", unico: "CompileTimeErrorCode.DUPLICATE_FIELD_FORMAL_PARAMETER", mensagem: "The field '{0}' can't be initialized by multiple parameters in the same constructor.", correcao: Some("Try removing one of the parameters, or using different fields."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "duplicate_field_name", unico: "CompileTimeErrorCode.DUPLICATE_FIELD_NAME", mensagem: "The field name '{0}' is already used in this record.", correcao: Some("Try renaming the field."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "duplicate_named_argument", unico: "CompileTimeErrorCode.DUPLICATE_NAMED_ARGUMENT", mensagem: "The argument for the named parameter '{0}' was already specified.", correcao: Some("Try removing one of the named arguments, or correcting one of the names to reference a different named parameter."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "duplicate_part", unico: "CompileTimeErrorCode.DUPLICATE_PART", mensagem: "The library already contains a part with the URI '{0}'.", correcao: Some("Try removing all except one of the duplicated part directives."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "duplicate_pattern_assignment_variable", unico: "CompileTimeErrorCode.DUPLICATE_PATTERN_ASSIGNMENT_VARIABLE", mensagem: "The variable '{0}' is already assigned in this pattern.", correcao: Some("Try renaming the variable."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "duplicate_pattern_field", unico: "CompileTimeErrorCode.DUPLICATE_PATTERN_FIELD", mensagem: "The field '{0}' is already matched in this pattern.", correcao: Some("Try removing the duplicate field."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "duplicate_rest_element_in_pattern", unico: "CompileTimeErrorCode.DUPLICATE_REST_ELEMENT_IN_PATTERN", mensagem: "At most one rest element is allowed in a list or map pattern.", correcao: Some("Try removing the duplicate rest element."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "duplicate_variable_pattern", unico: "CompileTimeErrorCode.DUPLICATE_VARIABLE_PATTERN", mensagem: "The variable '{0}' is already defined in this pattern.", correcao: Some("Try renaming the variable."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "empty_map_pattern", unico: "CompileTimeErrorCode.EMPTY_MAP_PATTERN", mensagem: "A map pattern must have at least one entry.", correcao: Some("Try replacing it with an object pattern 'Map()'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "enum_constant_invokes_factory_constructor", unico: "CompileTimeErrorCode.ENUM_CONSTANT_INVOKES_FACTORY_CONSTRUCTOR", mensagem: "An enum value can't invoke a factory constructor.", correcao: Some("Try using a generative constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "enum_constant_same_name_as_enclosing", unico: "CompileTimeErrorCode.ENUM_CONSTANT_SAME_NAME_AS_ENCLOSING", mensagem: "The name of the enum value can't be the same as the enum's name.", correcao: Some("Try renaming the constant."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "enum_instantiated_to_bounds_is_not_well_bounded", unico: "CompileTimeErrorCode.ENUM_INSTANTIATED_TO_BOUNDS_IS_NOT_WELL_BOUNDED", mensagem: "The result of instantiating the enum to bounds is not well-bounded.", correcao: Some("Try using different bounds for type parameters."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "enum_mixin_with_instance_variable", unico: "CompileTimeErrorCode.ENUM_MIXIN_WITH_INSTANCE_VARIABLE", mensagem: "Mixins applied to enums can't have instance variables.", correcao: Some("Try replacing the instance variables with getters."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "enum_without_constants", unico: "CompileTimeErrorCode.ENUM_WITHOUT_CONSTANTS", mensagem: "The enum must have at least one constant.", correcao: Some("Try declaring a constant."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "enum_with_abstract_member", unico: "CompileTimeErrorCode.ENUM_WITH_ABSTRACT_MEMBER", mensagem: "'{0}' must have a method body because '{1}' is an enum.", correcao: Some("Try adding a body to '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "enum_with_name_values", unico: "CompileTimeErrorCode.ENUM_WITH_NAME_VALUES", mensagem: "The name 'values' is not a valid name for an enum.", correcao: Some("Try using a different name."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "equal_elements_in_const_set", unico: "CompileTimeErrorCode.EQUAL_ELEMENTS_IN_CONST_SET", mensagem: "Two elements in a constant set literal can't be equal.", correcao: Some("Change or remove the duplicate element."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "equal_keys_in_const_map", unico: "CompileTimeErrorCode.EQUAL_KEYS_IN_CONST_MAP", mensagem: "Two keys in a constant map literal can't be equal.", correcao: Some("Change or remove the duplicate key."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "equal_keys_in_map_pattern", unico: "CompileTimeErrorCode.EQUAL_KEYS_IN_MAP_PATTERN", mensagem: "Two keys in a map pattern can't be equal.", correcao: Some("Change or remove the duplicate key."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "expected_one_list_pattern_type_arguments", unico: "CompileTimeErrorCode.EXPECTED_ONE_LIST_PATTERN_TYPE_ARGUMENTS", mensagem: "List patterns require one type argument or none, but {0} found.", correcao: Some("Try adjusting the number of type arguments."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "expected_one_list_type_arguments", unico: "CompileTimeErrorCode.EXPECTED_ONE_LIST_TYPE_ARGUMENTS", mensagem: "List literals require one type argument or none, but {0} found.", correcao: Some("Try adjusting the number of type arguments."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "expected_one_set_type_arguments", unico: "CompileTimeErrorCode.EXPECTED_ONE_SET_TYPE_ARGUMENTS", mensagem: "Set literals require one type argument or none, but {0} were found.", correcao: Some("Try adjusting the number of type arguments."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "expected_two_map_pattern_type_arguments", unico: "CompileTimeErrorCode.EXPECTED_TWO_MAP_PATTERN_TYPE_ARGUMENTS", mensagem: "Map patterns require two type arguments or none, but {0} found.", correcao: Some("Try adjusting the number of type arguments."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "expected_two_map_type_arguments", unico: "CompileTimeErrorCode.EXPECTED_TWO_MAP_TYPE_ARGUMENTS", mensagem: "Map literals require two type arguments or none, but {0} found.", correcao: Some("Try adjusting the number of type arguments."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "export_internal_library", unico: "CompileTimeErrorCode.EXPORT_INTERNAL_LIBRARY", mensagem: "The library '{0}' is internal and can't be exported.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "export_of_non_library", unico: "CompileTimeErrorCode.EXPORT_OF_NON_LIBRARY", mensagem: "The exported library '{0}' can't have a part-of directive.", correcao: Some("Try exporting the library that the part is a part of."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "expression_in_map", unico: "CompileTimeErrorCode.EXPRESSION_IN_MAP", mensagem: "Expressions can't be used in a map literal.", correcao: Some("Try removing the expression or converting it to be a map entry."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "subtype_of_deferred_class", unico: "CompileTimeErrorCode.EXTENDS_DEFERRED_CLASS", mensagem: "Classes can't extend deferred classes.", correcao: Some("Try specifying a different superclass, or removing the extends clause."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "subtype_of_disallowed_type", unico: "CompileTimeErrorCode.EXTENDS_DISALLOWED_CLASS", mensagem: "Classes can't extend '{0}'.", correcao: Some("Try specifying a different superclass, or removing the extends clause."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extends_non_class", unico: "CompileTimeErrorCode.EXTENDS_NON_CLASS", mensagem: "Classes can only extend other classes.", correcao: Some("Try specifying a different superclass, or removing the extends clause."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "supertype_expands_to_type_parameter", unico: "CompileTimeErrorCode.EXTENDS_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER", mensagem: "A type alias that expands to a type parameter can't be used as a superclass.", correcao: Some("Try specifying a different superclass, or removing the extends clause."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extension_as_expression", unico: "CompileTimeErrorCode.EXTENSION_AS_EXPRESSION", mensagem: "Extension '{0}' can't be used as an expression.", correcao: Some("Try replacing it with a valid expression."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extension_conflicting_static_and_instance", unico: "CompileTimeErrorCode.EXTENSION_CONFLICTING_STATIC_AND_INSTANCE", mensagem: "An extension can't define static member '{0}' and an instance member with the same name.", correcao: Some("Try renaming the member to a name that doesn't conflict."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extension_declares_member_of_object", unico: "CompileTimeErrorCode.EXTENSION_DECLARES_MEMBER_OF_OBJECT", mensagem: "Extensions can't declare members with the same name as a member declared by 'Object'.", correcao: Some("Try specifying a different name for the member."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extension_override_access_to_static_member", unico: "CompileTimeErrorCode.EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER", mensagem: "An extension override can't be used to access a static member from an extension.", correcao: Some("Try using just the name of the extension."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extension_override_argument_not_assignable", unico: "CompileTimeErrorCode.EXTENSION_OVERRIDE_ARGUMENT_NOT_ASSIGNABLE", mensagem: "The type of the argument to the extension override '{0}' isn't assignable to the extended type '{1}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extension_override_without_access", unico: "CompileTimeErrorCode.EXTENSION_OVERRIDE_WITHOUT_ACCESS", mensagem: "An extension override can only be used to access instance members.", correcao: Some("Consider adding an access to an instance member."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extension_override_with_cascade", unico: "CompileTimeErrorCode.EXTENSION_OVERRIDE_WITH_CASCADE", mensagem: "Extension overrides have no value so they can't be used as the receiver of a cascade expression.", correcao: Some("Try using '.' instead of '..'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extension_type_constructor_with_super_formal_parameter", unico: "CompileTimeErrorCode.EXTENSION_TYPE_CONSTRUCTOR_WITH_SUPER_FORMAL_PARAMETER", mensagem: "Extension type constructors can't declare super formal parameters.", correcao: Some("Try removing the super formal parameter declaration."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extension_type_constructor_with_super_invocation", unico: "CompileTimeErrorCode.EXTENSION_TYPE_CONSTRUCTOR_WITH_SUPER_INVOCATION", mensagem: "Extension type constructors can't include super initializers.", correcao: Some("Try removing the super constructor invocation."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extension_type_declares_instance_field", unico: "CompileTimeErrorCode.EXTENSION_TYPE_DECLARES_INSTANCE_FIELD", mensagem: "Extension types can't declare instance fields.", correcao: Some("Try replacing the field with a getter."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extension_type_declares_member_of_object", unico: "CompileTimeErrorCode.EXTENSION_TYPE_DECLARES_MEMBER_OF_OBJECT", mensagem: "Extension types can't declare members with the same name as a member declared by 'Object'.", correcao: Some("Try specifying a different name for the member."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extension_type_implements_disallowed_type", unico: "CompileTimeErrorCode.EXTENSION_TYPE_IMPLEMENTS_DISALLOWED_TYPE", mensagem: "Extension types can't implement '{0}'.", correcao: Some("Try specifying a different type, or remove the type from the list."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extension_type_implements_itself", unico: "CompileTimeErrorCode.EXTENSION_TYPE_IMPLEMENTS_ITSELF", mensagem: "The extension type can't implement itself.", correcao: Some("Try removing the superinterface that references this extension type."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extension_type_implements_not_supertype", unico: "CompileTimeErrorCode.EXTENSION_TYPE_IMPLEMENTS_NOT_SUPERTYPE", mensagem: "'{0}' is not a supertype of '{1}', the representation type.", correcao: Some("Try specifying a different type, or remove the type from the list."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extension_type_implements_representation_not_supertype", unico: "CompileTimeErrorCode.EXTENSION_TYPE_IMPLEMENTS_REPRESENTATION_NOT_SUPERTYPE", mensagem: "'{0}', the representation type of '{1}', is not a supertype of '{2}', the representation type of '{3}'.", correcao: Some("Try specifying a different type, or remove the type from the list."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extension_type_inherited_member_conflict", unico: "CompileTimeErrorCode.EXTENSION_TYPE_INHERITED_MEMBER_CONFLICT", mensagem: "The extension type '{0}' has more than one distinct member named '{1}' from implemented types.", correcao: Some("Try redeclaring the corresponding member in this extension type."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extension_type_representation_depends_on_itself", unico: "CompileTimeErrorCode.EXTENSION_TYPE_REPRESENTATION_DEPENDS_ON_ITSELF", mensagem: "The extension type representation can't depend on itself.", correcao: Some("Try specifying a different type."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extension_type_representation_type_bottom", unico: "CompileTimeErrorCode.EXTENSION_TYPE_REPRESENTATION_TYPE_BOTTOM", mensagem: "The representation type can't be a bottom type.", correcao: Some("Try specifying a different type."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extension_type_with_abstract_member", unico: "CompileTimeErrorCode.EXTENSION_TYPE_WITH_ABSTRACT_MEMBER", mensagem: "'{0}' must have a method body because '{1}' is an extension type.", correcao: Some("Try adding a body to '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "external_with_initializer", unico: "CompileTimeErrorCode.EXTERNAL_FIELD_CONSTRUCTOR_INITIALIZER", mensagem: "External fields can't have initializers.", correcao: Some("Try removing the field initializer or the 'external' keyword from the field declaration."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "external_with_initializer", unico: "CompileTimeErrorCode.EXTERNAL_FIELD_INITIALIZER", mensagem: "External fields can't have initializers.", correcao: Some("Try removing the initializer or the 'external' keyword."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "external_with_initializer", unico: "CompileTimeErrorCode.EXTERNAL_VARIABLE_INITIALIZER", mensagem: "External variables can't have initializers.", correcao: Some("Try removing the initializer or the 'external' keyword."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extra_positional_arguments", unico: "CompileTimeErrorCode.EXTRA_POSITIONAL_ARGUMENTS", mensagem: "Too many positional arguments: {0} expected, but {1} found.", correcao: Some("Try removing the extra arguments."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extra_positional_arguments_could_be_named", unico: "CompileTimeErrorCode.EXTRA_POSITIONAL_ARGUMENTS_COULD_BE_NAMED", mensagem: "Too many positional arguments: {0} expected, but {1} found.", correcao: Some("Try removing the extra positional arguments, or specifying the name for named arguments."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "field_initialized_by_multiple_initializers", unico: "CompileTimeErrorCode.FIELD_INITIALIZED_BY_MULTIPLE_INITIALIZERS", mensagem: "The field '{0}' can't be initialized twice in the same constructor.", correcao: Some("Try removing one of the initializations."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "field_initialized_in_initializer_and_declaration", unico: "CompileTimeErrorCode.FIELD_INITIALIZED_IN_INITIALIZER_AND_DECLARATION", mensagem: "Fields can't be initialized in the constructor if they are final and were already initialized at their declaration.", correcao: Some("Try removing one of the initializations."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "field_initialized_in_parameter_and_initializer", unico: "CompileTimeErrorCode.FIELD_INITIALIZED_IN_PARAMETER_AND_INITIALIZER", mensagem: "Fields can't be initialized in both the parameter list and the initializers.", correcao: Some("Try removing one of the initializations."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "field_initializer_factory_constructor", unico: "CompileTimeErrorCode.FIELD_INITIALIZER_FACTORY_CONSTRUCTOR", mensagem: "Initializing formal parameters can't be used in factory constructors.", correcao: Some("Try using a normal parameter."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "field_initializer_not_assignable", unico: "CompileTimeErrorCode.FIELD_INITIALIZER_NOT_ASSIGNABLE", mensagem: "The initializer type '{0}' can't be assigned to the field type '{1}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "field_initializer_outside_constructor", unico: "CompileTimeErrorCode.FIELD_INITIALIZER_OUTSIDE_CONSTRUCTOR", mensagem: "Initializing formal parameters can only be used in constructors.", correcao: Some("Try using a normal parameter."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "field_initializer_redirecting_constructor", unico: "CompileTimeErrorCode.FIELD_INITIALIZER_REDIRECTING_CONSTRUCTOR", mensagem: "The redirecting constructor can't have a field initializer.", correcao: Some("Try initializing the field in the constructor being redirected to."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "field_initializing_formal_not_assignable", unico: "CompileTimeErrorCode.FIELD_INITIALIZING_FORMAL_NOT_ASSIGNABLE", mensagem: "The parameter type '{0}' is incompatible with the field type '{1}'.", correcao: Some("Try changing or removing the parameter's type, or changing the field's type."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_use_of_type_outside_library", unico: "CompileTimeErrorCode.FINAL_CLASS_EXTENDED_OUTSIDE_OF_LIBRARY", mensagem: "The class '{0}' can't be extended outside of its library because it's a final class.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_use_of_type_outside_library", unico: "CompileTimeErrorCode.FINAL_CLASS_IMPLEMENTED_OUTSIDE_OF_LIBRARY", mensagem: "The class '{0}' can't be implemented outside of its library because it's a final class.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_use_of_type_outside_library", unico: "CompileTimeErrorCode.FINAL_CLASS_USED_AS_MIXIN_CONSTRAINT_OUTSIDE_OF_LIBRARY", mensagem: "The class '{0}' can't be used as a mixin superclass constraint outside of its library because it's a final class.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "final_initialized_in_declaration_and_constructor", unico: "CompileTimeErrorCode.FINAL_INITIALIZED_IN_DECLARATION_AND_CONSTRUCTOR", mensagem: "'{0}' is final and was given a value when it was declared, so it can't be set to a new value.", correcao: Some("Try removing one of the initializations."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "final_not_initialized", unico: "CompileTimeErrorCode.FINAL_NOT_INITIALIZED", mensagem: "The final variable '{0}' must be initialized.", correcao: Some("Try initializing the variable."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "final_not_initialized_constructor", unico: "CompileTimeErrorCode.FINAL_NOT_INITIALIZED_CONSTRUCTOR_1", mensagem: "All final variables must be initialized, but '{0}' isn't.", correcao: Some("Try adding an initializer for the field."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "final_not_initialized_constructor", unico: "CompileTimeErrorCode.FINAL_NOT_INITIALIZED_CONSTRUCTOR_2", mensagem: "All final variables must be initialized, but '{0}' and '{1}' aren't.", correcao: Some("Try adding initializers for the fields."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "final_not_initialized_constructor", unico: "CompileTimeErrorCode.FINAL_NOT_INITIALIZED_CONSTRUCTOR_3_PLUS", mensagem: "All final variables must be initialized, but '{0}', '{1}', and {2} others aren't.", correcao: Some("Try adding initializers for the fields."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "for_in_of_invalid_element_type", unico: "CompileTimeErrorCode.FOR_IN_OF_INVALID_ELEMENT_TYPE", mensagem: "The type '{0}' used in the 'for' loop must implement '{1}' with a type argument that can be assigned to '{2}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "for_in_of_invalid_type", unico: "CompileTimeErrorCode.FOR_IN_OF_INVALID_TYPE", mensagem: "The type '{0}' used in the 'for' loop must implement '{1}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "for_in_with_const_variable", unico: "CompileTimeErrorCode.FOR_IN_WITH_CONST_VARIABLE", mensagem: "A for-in loop variable can't be a 'const'.", correcao: Some("Try removing the 'const' modifier from the variable, or use a different variable."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "generic_function_type_cannot_be_bound", unico: "CompileTimeErrorCode.GENERIC_FUNCTION_TYPE_CANNOT_BE_BOUND", mensagem: "Generic function types can't be used as type parameter bounds.", correcao: Some("Try making the free variable in the function type part of the larger declaration signature."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "generic_function_type_cannot_be_type_argument", unico: "CompileTimeErrorCode.GENERIC_FUNCTION_TYPE_CANNOT_BE_TYPE_ARGUMENT", mensagem: "A generic function type can't be a type argument.", correcao: Some("Try removing type parameters from the generic function type, or using 'dynamic' as the type argument here."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "generic_method_type_instantiation_on_dynamic", unico: "CompileTimeErrorCode.GENERIC_METHOD_TYPE_INSTANTIATION_ON_DYNAMIC", mensagem: "A method tear-off on a receiver whose type is 'dynamic' can't have type arguments.", correcao: Some("Specify the type of the receiver, or remove the type arguments from the method tear-off."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "getter_not_assignable_setter_types", unico: "CompileTimeErrorCode.GETTER_NOT_ASSIGNABLE_SETTER_TYPES", mensagem: "The return type of getter '{0}' is '{1}' which isn't assignable to the type '{2}' of its setter '{3}'.", correcao: Some("Try changing the types so that they are compatible."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "getter_not_subtype_setter_types", unico: "CompileTimeErrorCode.GETTER_NOT_SUBTYPE_SETTER_TYPES", mensagem: "The return type of getter '{0}' is '{1}' which isn't a subtype of the type '{2}' of its setter '{3}'.", correcao: Some("Try changing the types so that they are compatible."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "if_element_condition_from_deferred_library", unico: "CompileTimeErrorCode.IF_ELEMENT_CONDITION_FROM_DEFERRED_LIBRARY", mensagem: "Constant values from a deferred library can't be used as values in an if condition inside a const collection literal.", correcao: Some("Try making the deferred import non-deferred."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "illegal_async_generator_return_type", unico: "CompileTimeErrorCode.ILLEGAL_ASYNC_GENERATOR_RETURN_TYPE", mensagem: "Functions marked 'async*' must have a return type that is a supertype of 'Stream<T>' for some type 'T'.", correcao: Some("Try fixing the return type of the function, or removing the modifier 'async*' from the function body."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "illegal_async_return_type", unico: "CompileTimeErrorCode.ILLEGAL_ASYNC_RETURN_TYPE", mensagem: "Functions marked 'async' must have a return type which is a supertype of 'Future'.", correcao: Some("Try fixing the return type of the function, or removing the modifier 'async' from the function body."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "illegal_concrete_enum_member", unico: "CompileTimeErrorCode.ILLEGAL_CONCRETE_ENUM_MEMBER_DECLARATION", mensagem: "A concrete instance member named '{0}' can't be declared in a class that implements 'Enum'.", correcao: Some("Try using a different name."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "illegal_concrete_enum_member", unico: "CompileTimeErrorCode.ILLEGAL_CONCRETE_ENUM_MEMBER_INHERITANCE", mensagem: "A concrete instance member named '{0}' can't be inherited from '{1}' in a class that implements 'Enum'.", correcao: Some("Try using a different name."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "illegal_enum_values", unico: "CompileTimeErrorCode.ILLEGAL_ENUM_VALUES_DECLARATION", mensagem: "An instance member named 'values' can't be declared in a class that implements 'Enum'.", correcao: Some("Try using a different name."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "illegal_enum_values", unico: "CompileTimeErrorCode.ILLEGAL_ENUM_VALUES_INHERITANCE", mensagem: "An instance member named 'values' can't be inherited from '{0}' in a class that implements 'Enum'.", correcao: Some("Try using a different name."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "illegal_language_version_override", unico: "CompileTimeErrorCode.ILLEGAL_LANGUAGE_VERSION_OVERRIDE", mensagem: "The language version must be {0}.", correcao: Some("Try removing the language version override and migrating the code."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "illegal_sync_generator_return_type", unico: "CompileTimeErrorCode.ILLEGAL_SYNC_GENERATOR_RETURN_TYPE", mensagem: "Functions marked 'sync*' must have a return type that is a supertype of 'Iterable<T>' for some type 'T'.", correcao: Some("Try fixing the return type of the function, or removing the modifier 'sync*' from the function body."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "subtype_of_deferred_class", unico: "CompileTimeErrorCode.IMPLEMENTS_DEFERRED_CLASS", mensagem: "Classes and mixins can't implement deferred classes.", correcao: Some("Try specifying a different interface, removing the class from the list, or changing the import to not be deferred."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "subtype_of_disallowed_type", unico: "CompileTimeErrorCode.IMPLEMENTS_DISALLOWED_CLASS", mensagem: "Classes and mixins can't implement '{0}'.", correcao: Some("Try specifying a different interface, or remove the class from the list."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "implements_non_class", unico: "CompileTimeErrorCode.IMPLEMENTS_NON_CLASS", mensagem: "Classes and mixins can only implement other classes and mixins.", correcao: Some("Try specifying a class or mixin, or remove the name from the list."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "implements_repeated", unico: "CompileTimeErrorCode.IMPLEMENTS_REPEATED", mensagem: "'{0}' can only be implemented once.", correcao: Some("Try removing all but one occurrence of the class name."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "implements_super_class", unico: "CompileTimeErrorCode.IMPLEMENTS_SUPER_CLASS", mensagem: "'{0}' can't be used in both the 'extends' and 'implements' clauses.", correcao: Some("Try removing one of the occurrences."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "supertype_expands_to_type_parameter", unico: "CompileTimeErrorCode.IMPLEMENTS_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER", mensagem: "A type alias that expands to a type parameter can't be implemented.", correcao: Some("Try specifying a class or mixin, or removing the list."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "implicit_super_initializer_missing_arguments", unico: "CompileTimeErrorCode.IMPLICIT_SUPER_INITIALIZER_MISSING_ARGUMENTS", mensagem: "The implicitly invoked unnamed constructor from '{0}' has required parameters.", correcao: Some("Try adding an explicit super parameter with the required arguments."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "implicit_this_reference_in_initializer", unico: "CompileTimeErrorCode.IMPLICIT_THIS_REFERENCE_IN_INITIALIZER", mensagem: "The instance member '{0}' can't be accessed in an initializer.", correcao: Some("Try replacing the reference to the instance member with a different expression"), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "import_internal_library", unico: "CompileTimeErrorCode.IMPORT_INTERNAL_LIBRARY", mensagem: "The library '{0}' is internal and can't be imported.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "import_of_non_library", unico: "CompileTimeErrorCode.IMPORT_OF_NON_LIBRARY", mensagem: "The imported library '{0}' can't have a part-of directive.", correcao: Some("Try importing the library that the part is a part of."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "inconsistent_case_expression_types", unico: "CompileTimeErrorCode.INCONSISTENT_CASE_EXPRESSION_TYPES", mensagem: "Case expressions must have the same types, '{0}' isn't a '{1}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "inconsistent_inheritance", unico: "CompileTimeErrorCode.INCONSISTENT_INHERITANCE", mensagem: "Superinterfaces don't have a valid override for '{0}': {1}.", correcao: Some("Try adding an explicit override that is consistent with all of the inherited members."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "inconsistent_inheritance_getter_and_method", unico: "CompileTimeErrorCode.INCONSISTENT_INHERITANCE_GETTER_AND_METHOD", mensagem: "'{0}' is inherited as a getter (from '{1}') and also a method (from '{2}').", correcao: Some("Try adjusting the supertypes of this class to remove the inconsistency."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "inconsistent_language_version_override", unico: "CompileTimeErrorCode.INCONSISTENT_LANGUAGE_VERSION_OVERRIDE", mensagem: "Parts must have exactly the same language version override as the library.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "inconsistent_pattern_variable_logical_or", unico: "CompileTimeErrorCode.INCONSISTENT_PATTERN_VARIABLE_LOGICAL_OR", mensagem: "The variable '{0}' has a different type and/or finality in this branch of the logical-or pattern.", correcao: Some("Try declaring the variable pattern with the same type and finality in both branches."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "initializer_for_non_existent_field", unico: "CompileTimeErrorCode.INITIALIZER_FOR_NON_EXISTENT_FIELD", mensagem: "'{0}' isn't a field in the enclosing class.", correcao: Some("Try correcting the name to match an existing field, or defining a field named '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "initializer_for_static_field", unico: "CompileTimeErrorCode.INITIALIZER_FOR_STATIC_FIELD", mensagem: "'{0}' is a static field in the enclosing class. Fields initialized in a constructor can't be static.", correcao: Some("Try removing the initialization."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "initializing_formal_for_non_existent_field", unico: "CompileTimeErrorCode.INITIALIZING_FORMAL_FOR_NON_EXISTENT_FIELD", mensagem: "'{0}' isn't a field in the enclosing class.", correcao: Some("Try correcting the name to match an existing field, or defining a field named '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "instance_access_to_static_member", unico: "CompileTimeErrorCode.INSTANCE_ACCESS_TO_STATIC_MEMBER", mensagem: "The static {1} '{0}' can't be accessed through an instance.", correcao: Some("Try using the {3} '{2}' to access the {1}."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "instance_access_to_static_member", unico: "CompileTimeErrorCode.INSTANCE_ACCESS_TO_STATIC_MEMBER_OF_UNNAMED_EXTENSION", mensagem: "The static {1} '{0}' can't be accessed through an instance.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "instance_member_access_from_factory", unico: "CompileTimeErrorCode.INSTANCE_MEMBER_ACCESS_FROM_FACTORY", mensagem: "Instance members can't be accessed from a factory constructor.", correcao: Some("Try removing the reference to the instance member."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "instance_member_access_from_static", unico: "CompileTimeErrorCode.INSTANCE_MEMBER_ACCESS_FROM_STATIC", mensagem: "Instance members can't be accessed from a static method.", correcao: Some("Try removing the reference to the instance member, or removing the keyword 'static' from the method."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "instantiate_abstract_class", unico: "CompileTimeErrorCode.INSTANTIATE_ABSTRACT_CLASS", mensagem: "Abstract classes can't be instantiated.", correcao: Some("Try creating an instance of a concrete subtype."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "instantiate_enum", unico: "CompileTimeErrorCode.INSTANTIATE_ENUM", mensagem: "Enums can't be instantiated.", correcao: Some("Try using one of the defined constants."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "instantiate_type_alias_expands_to_type_parameter", unico: "CompileTimeErrorCode.INSTANTIATE_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER", mensagem: "Type aliases that expand to a type parameter can't be instantiated.", correcao: Some("Try replacing it with a class."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "integer_literal_imprecise_as_double", unico: "CompileTimeErrorCode.INTEGER_LITERAL_IMPRECISE_AS_DOUBLE", mensagem: "The integer literal is being used as a double, but can't be represented as a 64-bit double without overflow or loss of precision: '{0}'.", correcao: Some("Try using the class 'BigInt', or switch to the closest valid double: '{1}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "integer_literal_out_of_range", unico: "CompileTimeErrorCode.INTEGER_LITERAL_OUT_OF_RANGE", mensagem: "The integer literal {0} can't be represented in 64 bits.", correcao: Some("Try using the 'BigInt' class if you need an integer larger than 9,223,372,036,854,775,807 or less than -9,223,372,036,854,775,808."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_use_of_type_outside_library", unico: "CompileTimeErrorCode.INTERFACE_CLASS_EXTENDED_OUTSIDE_OF_LIBRARY", mensagem: "The class '{0}' can't be extended outside of its library because it's an interface class.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_annotation", unico: "CompileTimeErrorCode.INVALID_ANNOTATION", mensagem: "Annotation must be either a const variable reference or const constructor invocation.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_annotation_constant_value_from_deferred_library", unico: "CompileTimeErrorCode.INVALID_ANNOTATION_CONSTANT_VALUE_FROM_DEFERRED_LIBRARY", mensagem: "Constant values from a deferred library can't be used in annotations.", correcao: Some("Try moving the constant from the deferred library, or removing 'deferred' from the import."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_annotation_from_deferred_library", unico: "CompileTimeErrorCode.INVALID_ANNOTATION_FROM_DEFERRED_LIBRARY", mensagem: "Constant values from a deferred library can't be used as annotations.", correcao: Some("Try removing the annotation, or changing the import to not be deferred."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_assignment", unico: "CompileTimeErrorCode.INVALID_ASSIGNMENT", mensagem: "A value of type '{0}' can't be assigned to a variable of type '{1}'.", correcao: Some("Try changing the type of the variable, or casting the right-hand type to '{1}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_cast_function", unico: "CompileTimeErrorCode.INVALID_CAST_FUNCTION", mensagem: "The function '{0}' has type '{1}' that isn't of expected type '{2}'. This means its parameter or return type doesn't match what is expected.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_cast_function_expr", unico: "CompileTimeErrorCode.INVALID_CAST_FUNCTION_EXPR", mensagem: "The function expression type '{0}' isn't of type '{1}'. This means its parameter or return type doesn't match what is expected. Consider changing parameter type(s) or the returned type(s).", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_cast_literal", unico: "CompileTimeErrorCode.INVALID_CAST_LITERAL", mensagem: "The literal '{0}' with type '{1}' isn't of expected type '{2}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_cast_literal_list", unico: "CompileTimeErrorCode.INVALID_CAST_LITERAL_LIST", mensagem: "The list literal type '{0}' isn't of expected type '{1}'. The list's type can be changed with an explicit generic type argument or by changing the element types.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_cast_literal_map", unico: "CompileTimeErrorCode.INVALID_CAST_LITERAL_MAP", mensagem: "The map literal type '{0}' isn't of expected type '{1}'. The map's type can be changed with an explicit generic type arguments or by changing the key and value types.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_cast_literal_set", unico: "CompileTimeErrorCode.INVALID_CAST_LITERAL_SET", mensagem: "The set literal type '{0}' isn't of expected type '{1}'. The set's type can be changed with an explicit generic type argument or by changing the element types.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_cast_method", unico: "CompileTimeErrorCode.INVALID_CAST_METHOD", mensagem: "The method tear-off '{0}' has type '{1}' that isn't of expected type '{2}'. This means its parameter or return type doesn't match what is expected.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_cast_new_expr", unico: "CompileTimeErrorCode.INVALID_CAST_NEW_EXPR", mensagem: "The constructor returns type '{0}' that isn't of expected type '{1}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_constant", unico: "CompileTimeErrorCode.INVALID_CONSTANT", mensagem: "Invalid constant value.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_extension_argument_count", unico: "CompileTimeErrorCode.INVALID_EXTENSION_ARGUMENT_COUNT", mensagem: "Extension overrides must have exactly one argument: the value of 'this' in the extension method.", correcao: Some("Try specifying exactly one argument."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_factory_name_not_a_class", unico: "CompileTimeErrorCode.INVALID_FACTORY_NAME_NOT_A_CLASS", mensagem: "The name of a factory constructor must be the same as the name of the immediately enclosing class.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_field_name", unico: "CompileTimeErrorCode.INVALID_FIELD_NAME_FROM_OBJECT", mensagem: "Record field names can't be the same as a member from 'Object'.", correcao: Some("Try using a different name for the field."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_field_name", unico: "CompileTimeErrorCode.INVALID_FIELD_NAME_POSITIONAL", mensagem: "Record field names can't be a dollar sign followed by an integer when the integer is the index of a positional field.", correcao: Some("Try using a different name for the field."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_field_name", unico: "CompileTimeErrorCode.INVALID_FIELD_NAME_PRIVATE", mensagem: "Record field names can't be private.", correcao: Some("Try removing the leading underscore."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_implementation_override", unico: "CompileTimeErrorCode.INVALID_IMPLEMENTATION_OVERRIDE", mensagem: "'{1}.{0}' ('{2}') isn't a valid concrete implementation of '{3}.{0}' ('{4}').", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_implementation_override", unico: "CompileTimeErrorCode.INVALID_IMPLEMENTATION_OVERRIDE_SETTER", mensagem: "The setter '{1}.{0}' ('{2}') isn't a valid concrete implementation of '{3}.{0}' ('{4}').", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_inline_function_type", unico: "CompileTimeErrorCode.INVALID_INLINE_FUNCTION_TYPE", mensagem: "Inline function types can't be used for parameters in a generic function type.", correcao: Some("Try using a generic function type (returnType 'Function(' parameters ')')."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_macro_application_target", unico: "CompileTimeErrorCode.INVALID_MACRO_APPLICATION_TARGET", mensagem: "The macro can be applied only to a {0}.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_modifier_on_constructor", unico: "CompileTimeErrorCode.INVALID_MODIFIER_ON_CONSTRUCTOR", mensagem: "The modifier '{0}' can't be applied to the body of a constructor.", correcao: Some("Try removing the modifier."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_modifier_on_setter", unico: "CompileTimeErrorCode.INVALID_MODIFIER_ON_SETTER", mensagem: "Setters can't use 'async', 'async*', or 'sync*'.", correcao: Some("Try removing the modifier."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_override", unico: "CompileTimeErrorCode.INVALID_OVERRIDE", mensagem: "'{1}.{0}' ('{2}') isn't a valid override of '{3}.{0}' ('{4}').", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_override", unico: "CompileTimeErrorCode.INVALID_OVERRIDE_SETTER", mensagem: "The setter '{1}.{0}' ('{2}') isn't a valid override of '{3}.{0}' ('{4}').", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_reference_to_generative_enum_constructor", unico: "CompileTimeErrorCode.INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR", mensagem: "Generative enum constructors can only be used as targets of redirection.", correcao: Some("Try using an enum value, or a factory constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_reference_to_this", unico: "CompileTimeErrorCode.INVALID_REFERENCE_TO_THIS", mensagem: "Invalid reference to 'this' expression.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_super_formal_parameter_location", unico: "CompileTimeErrorCode.INVALID_SUPER_FORMAL_PARAMETER_LOCATION", mensagem: "Super parameters can only be used in non-redirecting generative constructors.", correcao: Some("Try removing the 'super' modifier, or changing the constructor to be non-redirecting and generative."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_type_argument_in_const_literal", unico: "CompileTimeErrorCode.INVALID_TYPE_ARGUMENT_IN_CONST_LIST", mensagem: "Constant list literals can't use a type parameter in a type argument, such as '{0}'.", correcao: Some("Try replacing the type parameter with a different type."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_type_argument_in_const_literal", unico: "CompileTimeErrorCode.INVALID_TYPE_ARGUMENT_IN_CONST_MAP", mensagem: "Constant map literals can't use a type parameter in a type argument, such as '{0}'.", correcao: Some("Try replacing the type parameter with a different type."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_type_argument_in_const_literal", unico: "CompileTimeErrorCode.INVALID_TYPE_ARGUMENT_IN_CONST_SET", mensagem: "Constant set literals can't use a type parameter in a type argument, such as '{0}'.", correcao: Some("Try replacing the type parameter with a different type."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_uri", unico: "CompileTimeErrorCode.INVALID_URI", mensagem: "Invalid URI syntax: '{0}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_use_of_covariant", unico: "CompileTimeErrorCode.INVALID_USE_OF_COVARIANT", mensagem: "The 'covariant' keyword can only be used for parameters in instance methods or before non-final instance fields.", correcao: Some("Try removing the 'covariant' keyword."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_use_of_null_value", unico: "CompileTimeErrorCode.INVALID_USE_OF_NULL_VALUE", mensagem: "An expression whose value is always 'null' can't be dereferenced.", correcao: Some("Try changing the type of the expression."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invocation_of_extension_without_call", unico: "CompileTimeErrorCode.INVOCATION_OF_EXTENSION_WITHOUT_CALL", mensagem: "The extension '{0}' doesn't define a 'call' method so the override can't be used in an invocation.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invocation_of_non_function", unico: "CompileTimeErrorCode.INVOCATION_OF_NON_FUNCTION", mensagem: "'{0}' isn't a function.", correcao: Some("Try correcting the name to match an existing function, or define a method or function named '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invocation_of_non_function_expression", unico: "CompileTimeErrorCode.INVOCATION_OF_NON_FUNCTION_EXPRESSION", mensagem: "The expression doesn't evaluate to a function, so it can't be invoked.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "label_in_outer_scope", unico: "CompileTimeErrorCode.LABEL_IN_OUTER_SCOPE", mensagem: "Can't reference label '{0}' declared in an outer method.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "label_undefined", unico: "CompileTimeErrorCode.LABEL_UNDEFINED", mensagem: "Can't reference an undefined label '{0}'.", correcao: Some("Try defining the label, or correcting the name to match an existing label."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "late_final_field_with_const_constructor", unico: "CompileTimeErrorCode.LATE_FINAL_FIELD_WITH_CONST_CONSTRUCTOR", mensagem: "Can't have a late final field in a class with a generative const constructor.", correcao: Some("Try removing the 'late' modifier, or don't declare 'const' constructors."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "late_final_local_already_assigned", unico: "CompileTimeErrorCode.LATE_FINAL_LOCAL_ALREADY_ASSIGNED", mensagem: "The late final local variable is already assigned.", correcao: Some("Try removing the 'final' modifier, or don't reassign the value."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "list_element_type_not_assignable", unico: "CompileTimeErrorCode.LIST_ELEMENT_TYPE_NOT_ASSIGNABLE", mensagem: "The element type '{0}' can't be assigned to the list type '{1}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "macro_application_argument_error", unico: "CompileTimeErrorCode.MACRO_APPLICATION_ARGUMENT_ERROR", mensagem: "{0}", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "macro_declarations_phase_introspection_cycle", unico: "CompileTimeErrorCode.MACRO_DECLARATIONS_PHASE_INTROSPECTION_CYCLE", mensagem: "The declaration '{0}' can't be introspected because there is a cycle of macro applications.", correcao: Some("Try removing one or more macro applications to break the cycle."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "macro_definition_application_same_library_cycle", unico: "CompileTimeErrorCode.MACRO_DEFINITION_APPLICATION_SAME_LIBRARY_CYCLE", mensagem: "The macro '{0}' can't be applied in the same library cycle where it is defined.", correcao: Some("Try moving it to a different library that does not import the one where it is applied."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "macro_error", unico: "CompileTimeErrorCode.MACRO_ERROR", mensagem: "{0}", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "macro_internal_exception", unico: "CompileTimeErrorCode.MACRO_INTERNAL_EXCEPTION", mensagem: "{0} {1}", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "macro_not_allowed_declaration", unico: "CompileTimeErrorCode.MACRO_NOT_ALLOWED_DECLARATION", mensagem: "The macro attempted to add declaration(s) not allowed during the {0} phase.\nLocations: {1}\n---\n{2}\n---", correcao: Some("Try adding these declaration during an earlier phase."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "main_first_positional_parameter_type", unico: "CompileTimeErrorCode.MAIN_FIRST_POSITIONAL_PARAMETER_TYPE", mensagem: "The type of the first positional parameter of the 'main' function must be a supertype of 'List<String>'.", correcao: Some("Try changing the type of the parameter."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "main_has_required_named_parameters", unico: "CompileTimeErrorCode.MAIN_HAS_REQUIRED_NAMED_PARAMETERS", mensagem: "The function 'main' can't have any required named parameters.", correcao: Some("Try using a different name for the function, or removing the 'required' modifier."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "main_has_too_many_required_positional_parameters", unico: "CompileTimeErrorCode.MAIN_HAS_TOO_MANY_REQUIRED_POSITIONAL_PARAMETERS", mensagem: "The function 'main' can't have more than two required positional parameters.", correcao: Some("Try using a different name for the function, or removing extra parameters."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "main_is_not_function", unico: "CompileTimeErrorCode.MAIN_IS_NOT_FUNCTION", mensagem: "The declaration named 'main' must be a function.", correcao: Some("Try using a different name for this declaration."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "map_entry_not_in_map", unico: "CompileTimeErrorCode.MAP_ENTRY_NOT_IN_MAP", mensagem: "Map entries can only be used in a map literal.", correcao: Some("Try converting the collection to a map or removing the map entry."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "map_key_type_not_assignable", unico: "CompileTimeErrorCode.MAP_KEY_TYPE_NOT_ASSIGNABLE", mensagem: "The element type '{0}' can't be assigned to the map key type '{1}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "map_value_type_not_assignable", unico: "CompileTimeErrorCode.MAP_VALUE_TYPE_NOT_ASSIGNABLE", mensagem: "The element type '{0}' can't be assigned to the map value type '{1}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "missing_const_in_list_literal", unico: "CompileTimeErrorCode.MISSING_CONST_IN_LIST_LITERAL", mensagem: "Seeing this message constitutes a bug. Please report it.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_const_in_map_literal", unico: "CompileTimeErrorCode.MISSING_CONST_IN_MAP_LITERAL", mensagem: "Seeing this message constitutes a bug. Please report it.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_const_in_set_literal", unico: "CompileTimeErrorCode.MISSING_CONST_IN_SET_LITERAL", mensagem: "Seeing this message constitutes a bug. Please report it.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_dart_library", unico: "CompileTimeErrorCode.MISSING_DART_LIBRARY", mensagem: "Required library '{0}' is missing.", correcao: Some("Re-install the Dart or Flutter SDK."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "missing_default_value_for_parameter", unico: "CompileTimeErrorCode.MISSING_DEFAULT_VALUE_FOR_PARAMETER", mensagem: "The parameter '{0}' can't have a value of 'null' because of its type, but the implicit default value is 'null'.", correcao: Some("Try adding either an explicit non-'null' default value or the 'required' modifier."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "missing_default_value_for_parameter", unico: "CompileTimeErrorCode.MISSING_DEFAULT_VALUE_FOR_PARAMETER_POSITIONAL", mensagem: "The parameter '{0}' can't have a value of 'null' because of its type, but the implicit default value is 'null'.", correcao: Some("Try adding an explicit non-'null' default value."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "missing_default_value_for_parameter", unico: "CompileTimeErrorCode.MISSING_DEFAULT_VALUE_FOR_PARAMETER_WITH_ANNOTATION", mensagem: "With null safety, use the 'required' keyword, not the '@required' annotation.", correcao: Some("Try removing the '@'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "missing_named_pattern_field_name", unico: "CompileTimeErrorCode.MISSING_NAMED_PATTERN_FIELD_NAME", mensagem: "The getter name is not specified explicitly, and the pattern is not a variable.", correcao: Some("Try specifying the getter name explicitly, or using a variable pattern."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "missing_required_argument", unico: "CompileTimeErrorCode.MISSING_REQUIRED_ARGUMENT", mensagem: "The named parameter '{0}' is required, but there's no corresponding argument.", correcao: Some("Try adding the required argument."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "missing_variable_pattern", unico: "CompileTimeErrorCode.MISSING_VARIABLE_PATTERN", mensagem: "Variable pattern '{0}' is missing in this branch of the logical-or pattern.", correcao: Some("Try declaring this variable pattern in the branch."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "implements_super_class", unico: "CompileTimeErrorCode.MIXINS_SUPER_CLASS", mensagem: "'{0}' can't be used in both the 'extends' and 'with' clauses.", correcao: Some("Try removing one of the occurrences."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "mixin_application_concrete_super_invoked_member_type", unico: "CompileTimeErrorCode.MIXIN_APPLICATION_CONCRETE_SUPER_INVOKED_MEMBER_TYPE", mensagem: "The super-invoked member '{0}' has the type '{1}', and the concrete member in the class has the type '{2}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "mixin_application_not_implemented_interface", unico: "CompileTimeErrorCode.MIXIN_APPLICATION_NOT_IMPLEMENTED_INTERFACE", mensagem: "'{0}' can't be mixed onto '{1}' because '{1}' doesn't implement '{2}'.", correcao: Some("Try extending the class '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "mixin_application_no_concrete_super_invoked_member", unico: "CompileTimeErrorCode.MIXIN_APPLICATION_NO_CONCRETE_SUPER_INVOKED_MEMBER", mensagem: "The class doesn't have a concrete implementation of the super-invoked member '{0}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "mixin_application_no_concrete_super_invoked_member", unico: "CompileTimeErrorCode.MIXIN_APPLICATION_NO_CONCRETE_SUPER_INVOKED_SETTER", mensagem: "The class doesn't have a concrete implementation of the super-invoked setter '{0}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "mixin_class_declaration_extends_not_object", unico: "CompileTimeErrorCode.MIXIN_CLASS_DECLARATION_EXTENDS_NOT_OBJECT", mensagem: "The class '{0}' can't be declared a mixin because it extends a class other than 'Object'.", correcao: Some("Try removing the 'mixin' modifier or changing the superclass to 'Object'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "mixin_class_declares_constructor", unico: "CompileTimeErrorCode.MIXIN_CLASS_DECLARES_CONSTRUCTOR", mensagem: "The class '{0}' can't be used as a mixin because it declares a constructor.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "subtype_of_deferred_class", unico: "CompileTimeErrorCode.MIXIN_DEFERRED_CLASS", mensagem: "Classes can't mixin deferred classes.", correcao: Some("Try changing the import to not be deferred."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "mixin_inherits_from_not_object", unico: "CompileTimeErrorCode.MIXIN_INHERITS_FROM_NOT_OBJECT", mensagem: "The class '{0}' can't be used as a mixin because it extends a class other than 'Object'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "mixin_instantiate", unico: "CompileTimeErrorCode.MIXIN_INSTANTIATE", mensagem: "Mixins can't be instantiated.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "subtype_of_disallowed_type", unico: "CompileTimeErrorCode.MIXIN_OF_DISALLOWED_CLASS", mensagem: "Classes can't mixin '{0}'.", correcao: Some("Try specifying a different class or mixin, or remove the class or mixin from the list."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "mixin_of_non_class", unico: "CompileTimeErrorCode.MIXIN_OF_NON_CLASS", mensagem: "Classes can only mix in mixins and classes.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "supertype_expands_to_type_parameter", unico: "CompileTimeErrorCode.MIXIN_OF_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER", mensagem: "A type alias that expands to a type parameter can't be mixed in.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "supertype_expands_to_type_parameter", unico: "CompileTimeErrorCode.MIXIN_ON_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER", mensagem: "A type alias that expands to a type parameter can't be used as a superclass constraint.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "subtype_of_base_or_final_is_not_base_final_or_sealed", unico: "CompileTimeErrorCode.MIXIN_SUBTYPE_OF_BASE_IS_NOT_BASE", mensagem: "The mixin '{0}' must be 'base' because the supertype '{1}' is 'base'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "subtype_of_base_or_final_is_not_base_final_or_sealed", unico: "CompileTimeErrorCode.MIXIN_SUBTYPE_OF_FINAL_IS_NOT_BASE", mensagem: "The mixin '{0}' must be 'base' because the supertype '{1}' is 'final'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "mixin_super_class_constraint_deferred_class", unico: "CompileTimeErrorCode.MIXIN_SUPER_CLASS_CONSTRAINT_DEFERRED_CLASS", mensagem: "Deferred classes can't be used as superclass constraints.", correcao: Some("Try changing the import to not be deferred."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "subtype_of_disallowed_type", unico: "CompileTimeErrorCode.MIXIN_SUPER_CLASS_CONSTRAINT_DISALLOWED_CLASS", mensagem: "'{0}' can't be used as a superclass constraint.", correcao: Some("Try specifying a different super-class constraint, or remove the 'on' clause."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "mixin_super_class_constraint_non_interface", unico: "CompileTimeErrorCode.MIXIN_SUPER_CLASS_CONSTRAINT_NON_INTERFACE", mensagem: "Only classes and mixins can be used as superclass constraints.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "mixin_with_non_class_superclass", unico: "CompileTimeErrorCode.MIXIN_WITH_NON_CLASS_SUPERCLASS", mensagem: "Mixin can only be applied to class.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "multiple_redirecting_constructor_invocations", unico: "CompileTimeErrorCode.MULTIPLE_REDIRECTING_CONSTRUCTOR_INVOCATIONS", mensagem: "Constructors can have only one 'this' redirection, at most.", correcao: Some("Try removing all but one of the redirections."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "multiple_super_initializers", unico: "CompileTimeErrorCode.MULTIPLE_SUPER_INITIALIZERS", mensagem: "A constructor can have at most one 'super' initializer.", correcao: Some("Try removing all but one of the 'super' initializers."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "creation_with_non_type", unico: "CompileTimeErrorCode.NEW_WITH_NON_TYPE", mensagem: "The name '{0}' isn't a class.", correcao: Some("Try correcting the name to match an existing class."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "new_with_undefined_constructor", unico: "CompileTimeErrorCode.NEW_WITH_UNDEFINED_CONSTRUCTOR", mensagem: "The class '{0}' doesn't have a constructor named '{1}'.", correcao: Some("Try invoking a different constructor, or define a constructor named '{1}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "new_with_undefined_constructor_default", unico: "CompileTimeErrorCode.NEW_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT", mensagem: "The class '{0}' doesn't have an unnamed constructor.", correcao: Some("Try using one of the named constructors defined in '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_abstract_class_inherits_abstract_member", unico: "CompileTimeErrorCode.NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_FIVE_PLUS", mensagem: "Missing concrete implementations of '{0}', '{1}', '{2}', '{3}', and {4} more.", correcao: Some("Try implementing the missing methods, or make the class abstract."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_abstract_class_inherits_abstract_member", unico: "CompileTimeErrorCode.NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_FOUR", mensagem: "Missing concrete implementations of '{0}', '{1}', '{2}', and '{3}'.", correcao: Some("Try implementing the missing methods, or make the class abstract."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_abstract_class_inherits_abstract_member", unico: "CompileTimeErrorCode.NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_ONE", mensagem: "Missing concrete implementation of '{0}'.", correcao: Some("Try implementing the missing method, or make the class abstract."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_abstract_class_inherits_abstract_member", unico: "CompileTimeErrorCode.NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_THREE", mensagem: "Missing concrete implementations of '{0}', '{1}', and '{2}'.", correcao: Some("Try implementing the missing methods, or make the class abstract."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_abstract_class_inherits_abstract_member", unico: "CompileTimeErrorCode.NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_TWO", mensagem: "Missing concrete implementations of '{0}' and '{1}'.", correcao: Some("Try implementing the missing methods, or make the class abstract."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_bool_condition", unico: "CompileTimeErrorCode.NON_BOOL_CONDITION", mensagem: "Conditions must have a static type of 'bool'.", correcao: Some("Try changing the condition."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_bool_expression", unico: "CompileTimeErrorCode.NON_BOOL_EXPRESSION", mensagem: "The expression in an assert must be of type 'bool'.", correcao: Some("Try changing the expression."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_bool_negation_expression", unico: "CompileTimeErrorCode.NON_BOOL_NEGATION_EXPRESSION", mensagem: "A negation operand must have a static type of 'bool'.", correcao: Some("Try changing the operand to the '!' operator."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_bool_operand", unico: "CompileTimeErrorCode.NON_BOOL_OPERAND", mensagem: "The operands of the operator '{0}' must be assignable to 'bool'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_constant_annotation_constructor", unico: "CompileTimeErrorCode.NON_CONSTANT_ANNOTATION_CONSTRUCTOR", mensagem: "Annotation creation can only call a const constructor.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_constant_case_expression", unico: "CompileTimeErrorCode.NON_CONSTANT_CASE_EXPRESSION", mensagem: "Case expressions must be constant.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_constant_case_expression_from_deferred_library", unico: "CompileTimeErrorCode.NON_CONSTANT_CASE_EXPRESSION_FROM_DEFERRED_LIBRARY", mensagem: "Constant values from a deferred library can't be used as a case expression.", correcao: Some("Try re-writing the switch as a series of if statements, or changing the import to not be deferred."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_constant_default_value", unico: "CompileTimeErrorCode.NON_CONSTANT_DEFAULT_VALUE", mensagem: "The default value of an optional parameter must be constant.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_constant_default_value_from_deferred_library", unico: "CompileTimeErrorCode.NON_CONSTANT_DEFAULT_VALUE_FROM_DEFERRED_LIBRARY", mensagem: "Constant values from a deferred library can't be used as a default parameter value.", correcao: Some("Try leaving the default as 'null' and initializing the parameter inside the function body."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_constant_list_element", unico: "CompileTimeErrorCode.NON_CONSTANT_LIST_ELEMENT", mensagem: "The values in a const list literal must be constants.", correcao: Some("Try removing the keyword 'const' from the list literal."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "collection_element_from_deferred_library", unico: "CompileTimeErrorCode.NON_CONSTANT_LIST_ELEMENT_FROM_DEFERRED_LIBRARY", mensagem: "Constant values from a deferred library can't be used as values in a 'const' list literal.", correcao: Some("Try removing the keyword 'const' from the list literal or removing the keyword 'deferred' from the import."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_constant_map_element", unico: "CompileTimeErrorCode.NON_CONSTANT_MAP_ELEMENT", mensagem: "The elements in a const map literal must be constant.", correcao: Some("Try removing the keyword 'const' from the map literal."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_constant_map_key", unico: "CompileTimeErrorCode.NON_CONSTANT_MAP_KEY", mensagem: "The keys in a const map literal must be constant.", correcao: Some("Try removing the keyword 'const' from the map literal."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "collection_element_from_deferred_library", unico: "CompileTimeErrorCode.NON_CONSTANT_MAP_KEY_FROM_DEFERRED_LIBRARY", mensagem: "Constant values from a deferred library can't be used as keys in a 'const' map literal.", correcao: Some("Try removing the keyword 'const' from the map literal or removing the keyword 'deferred' from the import."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_constant_map_pattern_key", unico: "CompileTimeErrorCode.NON_CONSTANT_MAP_PATTERN_KEY", mensagem: "Key expressions in map patterns must be constants.", correcao: Some("Try using constants instead."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_constant_map_value", unico: "CompileTimeErrorCode.NON_CONSTANT_MAP_VALUE", mensagem: "The values in a const map literal must be constant.", correcao: Some("Try removing the keyword 'const' from the map literal."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "collection_element_from_deferred_library", unico: "CompileTimeErrorCode.NON_CONSTANT_MAP_VALUE_FROM_DEFERRED_LIBRARY", mensagem: "Constant values from a deferred library can't be used as values in a 'const' map literal.", correcao: Some("Try removing the keyword 'const' from the map literal or removing the keyword 'deferred' from the import."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_constant_record_field", unico: "CompileTimeErrorCode.NON_CONSTANT_RECORD_FIELD", mensagem: "The fields in a const record literal must be constants.", correcao: Some("Try removing the keyword 'const' from the record literal."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "non_constant_record_field_from_deferred_library", unico: "CompileTimeErrorCode.NON_CONSTANT_RECORD_FIELD_FROM_DEFERRED_LIBRARY", mensagem: "Constant values from a deferred library can't be used as fields in a 'const' record literal.", correcao: Some("Try removing the keyword 'const' from the record literal or removing the keyword 'deferred' from the import."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "non_constant_relational_pattern_expression", unico: "CompileTimeErrorCode.NON_CONSTANT_RELATIONAL_PATTERN_EXPRESSION", mensagem: "The relational pattern expression must be a constant.", correcao: Some("Try using a constant instead."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_constant_set_element", unico: "CompileTimeErrorCode.NON_CONSTANT_SET_ELEMENT", mensagem: "The values in a const set literal must be constants.", correcao: Some("Try removing the keyword 'const' from the set literal."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_const_generative_enum_constructor", unico: "CompileTimeErrorCode.NON_CONST_GENERATIVE_ENUM_CONSTRUCTOR", mensagem: "Generative enum constructors must be 'const'.", correcao: Some("Try adding the keyword 'const'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_const_map_as_expression_statement", unico: "CompileTimeErrorCode.NON_CONST_MAP_AS_EXPRESSION_STATEMENT", mensagem: "A non-constant map or set literal without type arguments can't be used as an expression statement.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "non_covariant_type_parameter_position_in_representation_type", unico: "CompileTimeErrorCode.NON_COVARIANT_TYPE_PARAMETER_POSITION_IN_REPRESENTATION_TYPE", mensagem: "An extension type parameter can't be used in a non-covariant position of its representation type.", correcao: Some("Try removing the type parameters from function parameter types and type parameter bounds."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_exhaustive_switch_expression", unico: "CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_EXPRESSION", mensagem: "The type '{0}' is not exhaustively matched by the switch cases since it doesn't match '{1}'.", correcao: Some("Try adding a wildcard pattern or cases that match '{2}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_exhaustive_switch_statement", unico: "CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_STATEMENT", mensagem: "The type '{0}' is not exhaustively matched by the switch cases since it doesn't match '{1}'.", correcao: Some("Try adding a default case or cases that match '{2}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_final_field_in_enum", unico: "CompileTimeErrorCode.NON_FINAL_FIELD_IN_ENUM", mensagem: "Enums can only declare final fields.", correcao: Some("Try making the field final."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_generative_constructor", unico: "CompileTimeErrorCode.NON_GENERATIVE_CONSTRUCTOR", mensagem: "The generative constructor '{0}' is expected, but a factory was found.", correcao: Some("Try calling a different constructor of the superclass, or making the called constructor not be a factory constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_generative_implicit_constructor", unico: "CompileTimeErrorCode.NON_GENERATIVE_IMPLICIT_CONSTRUCTOR", mensagem: "The unnamed constructor of superclass '{0}' (called by the default constructor of '{1}') must be a generative constructor, but factory found.", correcao: Some("Try adding an explicit constructor that has a different superinitializer or changing the superclass constructor '{2}' to not be a factory constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_sync_factory", unico: "CompileTimeErrorCode.NON_SYNC_FACTORY", mensagem: "Factory bodies can't use 'async', 'async*', or 'sync*'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_type_as_type_argument", unico: "CompileTimeErrorCode.NON_TYPE_AS_TYPE_ARGUMENT", mensagem: "The name '{0}' isn't a type, so it can't be used as a type argument.", correcao: Some("Try correcting the name to an existing type, or defining a type named '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_type_in_catch_clause", unico: "CompileTimeErrorCode.NON_TYPE_IN_CATCH_CLAUSE", mensagem: "The name '{0}' isn't a type and can't be used in an on-catch clause.", correcao: Some("Try correcting the name to match an existing class."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_void_return_for_operator", unico: "CompileTimeErrorCode.NON_VOID_RETURN_FOR_OPERATOR", mensagem: "The return type of the operator []= must be 'void'.", correcao: Some("Try changing the return type to 'void'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_void_return_for_setter", unico: "CompileTimeErrorCode.NON_VOID_RETURN_FOR_SETTER", mensagem: "The return type of the setter must be 'void' or absent.", correcao: Some("Try removing the return type, or define a method rather than a setter."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "not_assigned_potentially_non_nullable_local_variable", unico: "CompileTimeErrorCode.NOT_ASSIGNED_POTENTIALLY_NON_NULLABLE_LOCAL_VARIABLE", mensagem: "The non-nullable local variable '{0}' must be assigned before it can be used.", correcao: Some("Try giving it an initializer expression, or ensure that it's assigned on every execution path."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "not_a_type", unico: "CompileTimeErrorCode.NOT_A_TYPE", mensagem: "{0} isn't a type.", correcao: Some("Try correcting the name to match an existing type."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "not_binary_operator", unico: "CompileTimeErrorCode.NOT_BINARY_OPERATOR", mensagem: "'{0}' isn't a binary operator.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "not_enough_positional_arguments", unico: "CompileTimeErrorCode.NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_PLURAL", mensagem: "{0} positional arguments expected by '{2}', but {1} found.", correcao: Some("Try adding the missing arguments."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "not_enough_positional_arguments", unico: "CompileTimeErrorCode.NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_SINGULAR", mensagem: "1 positional argument expected by '{0}', but 0 found.", correcao: Some("Try adding the missing argument."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "not_enough_positional_arguments", unico: "CompileTimeErrorCode.NOT_ENOUGH_POSITIONAL_ARGUMENTS_PLURAL", mensagem: "{0} positional arguments expected, but {1} found.", correcao: Some("Try adding the missing arguments."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "not_enough_positional_arguments", unico: "CompileTimeErrorCode.NOT_ENOUGH_POSITIONAL_ARGUMENTS_SINGULAR", mensagem: "1 positional argument expected, but 0 found.", correcao: Some("Try adding the missing argument."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "not_initialized_non_nullable_instance_field", unico: "CompileTimeErrorCode.NOT_INITIALIZED_NON_NULLABLE_INSTANCE_FIELD", mensagem: "Non-nullable instance field '{0}' must be initialized.", correcao: Some("Try adding an initializer expression, or a generative constructor that initializes it, or mark it 'late'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "not_initialized_non_nullable_instance_field", unico: "CompileTimeErrorCode.NOT_INITIALIZED_NON_NULLABLE_INSTANCE_FIELD_CONSTRUCTOR", mensagem: "Non-nullable instance field '{0}' must be initialized.", correcao: Some("Try adding an initializer expression, or add a field initializer in this constructor, or mark it 'late'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "not_initialized_non_nullable_variable", unico: "CompileTimeErrorCode.NOT_INITIALIZED_NON_NULLABLE_VARIABLE", mensagem: "The non-nullable variable '{0}' must be initialized.", correcao: Some("Try adding an initializer expression."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "not_instantiated_bound", unico: "CompileTimeErrorCode.NOT_INSTANTIATED_BOUND", mensagem: "Type parameter bound types must be instantiated.", correcao: Some("Try adding type arguments to the type parameter bound."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "not_iterable_spread", unico: "CompileTimeErrorCode.NOT_ITERABLE_SPREAD", mensagem: "Spread elements in list or set literals must implement 'Iterable'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "not_map_spread", unico: "CompileTimeErrorCode.NOT_MAP_SPREAD", mensagem: "Spread elements in map literals must implement 'Map'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "not_null_aware_null_spread", unico: "CompileTimeErrorCode.NOT_NULL_AWARE_NULL_SPREAD", mensagem: "The Null-typed expression can't be used with a non-null-aware spread.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "no_annotation_constructor_arguments", unico: "CompileTimeErrorCode.NO_ANNOTATION_CONSTRUCTOR_ARGUMENTS", mensagem: "Annotation creation must have arguments.", correcao: Some("Try adding an empty argument list."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "no_combined_super_signature", unico: "CompileTimeErrorCode.NO_COMBINED_SUPER_SIGNATURE", mensagem: "Can't infer missing types in '{0}' from overridden methods: {1}.", correcao: Some("Try providing explicit types for this method's parameters and return type."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "no_default_super_constructor", unico: "CompileTimeErrorCode.NO_DEFAULT_SUPER_CONSTRUCTOR_EXPLICIT", mensagem: "The superclass '{0}' doesn't have a zero argument constructor.", correcao: Some("Try declaring a zero argument constructor in '{0}', or explicitly invoking a different constructor in '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "no_default_super_constructor", unico: "CompileTimeErrorCode.NO_DEFAULT_SUPER_CONSTRUCTOR_IMPLICIT", mensagem: "The superclass '{0}' doesn't have a zero argument constructor.", correcao: Some("Try declaring a zero argument constructor in '{0}', or declaring a constructor in {1} that explicitly invokes a constructor in '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "no_generative_constructors_in_superclass", unico: "CompileTimeErrorCode.NO_GENERATIVE_CONSTRUCTORS_IN_SUPERCLASS", mensagem: "The class '{0}' can't extend '{1}' because '{1}' only has factory constructors (no generative constructors), and '{0}' has at least one generative constructor.", correcao: Some("Try implementing the class instead, adding a generative (not factory) constructor to the superclass '{1}', or a factory constructor to the subclass."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "nullable_type_in_extends_clause", unico: "CompileTimeErrorCode.NULLABLE_TYPE_IN_EXTENDS_CLAUSE", mensagem: "A class can't extend a nullable type.", correcao: Some("Try removing the question mark."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "nullable_type_in_implements_clause", unico: "CompileTimeErrorCode.NULLABLE_TYPE_IN_IMPLEMENTS_CLAUSE", mensagem: "A class, mixin, or extension type can't implement a nullable type.", correcao: Some("Try removing the question mark."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "nullable_type_in_on_clause", unico: "CompileTimeErrorCode.NULLABLE_TYPE_IN_ON_CLAUSE", mensagem: "A mixin can't have a nullable type as a superclass constraint.", correcao: Some("Try removing the question mark."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "nullable_type_in_with_clause", unico: "CompileTimeErrorCode.NULLABLE_TYPE_IN_WITH_CLAUSE", mensagem: "A class or mixin can't mix in a nullable type.", correcao: Some("Try removing the question mark."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "object_cannot_extend_another_class", unico: "CompileTimeErrorCode.OBJECT_CANNOT_EXTEND_ANOTHER_CLASS", mensagem: "The class 'Object' can't extend any other class.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "obsolete_colon_for_default_value", unico: "CompileTimeErrorCode.OBSOLETE_COLON_FOR_DEFAULT_VALUE", mensagem: "Using a colon as the separator before a default value is no longer supported.", correcao: Some("Try replacing the colon with an equal sign."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "on_repeated", unico: "CompileTimeErrorCode.ON_REPEATED", mensagem: "The type '{0}' can be included in the superclass constraints only once.", correcao: Some("Try removing all except one occurrence of the type name."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "optional_parameter_in_operator", unico: "CompileTimeErrorCode.OPTIONAL_PARAMETER_IN_OPERATOR", mensagem: "Optional parameters aren't allowed when defining an operator.", correcao: Some("Try removing the optional parameters."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "part_of_different_library", unico: "CompileTimeErrorCode.PART_OF_DIFFERENT_LIBRARY", mensagem: "Expected this library to be part of '{0}', not '{1}'.", correcao: Some("Try including a different part, or changing the name of the library in the part's part-of directive."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "part_of_non_part", unico: "CompileTimeErrorCode.PART_OF_NON_PART", mensagem: "The included part '{0}' must have a part-of directive.", correcao: Some("Try adding a part-of directive to '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "part_of_unnamed_library", unico: "CompileTimeErrorCode.PART_OF_UNNAMED_LIBRARY", mensagem: "The library is unnamed. A URI is expected, not a library name '{0}', in the part-of directive.", correcao: Some("Try changing the part-of directive to a URI, or try including a different part."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "pattern_assignment_not_local_variable", unico: "CompileTimeErrorCode.PATTERN_ASSIGNMENT_NOT_LOCAL_VARIABLE", mensagem: "Only local variables can be assigned in pattern assignments.", correcao: Some("Try assigning to a local variable."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "pattern_constant_from_deferred_library", unico: "CompileTimeErrorCode.PATTERN_CONSTANT_FROM_DEFERRED_LIBRARY", mensagem: "Constant values from a deferred library can't be used in patterns.", correcao: Some("Try removing the keyword 'deferred' from the import."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "pattern_type_mismatch_in_irrefutable_context", unico: "CompileTimeErrorCode.PATTERN_TYPE_MISMATCH_IN_IRREFUTABLE_CONTEXT", mensagem: "The matched value of type '{0}' isn't assignable to the required type '{1}'.", correcao: Some("Try changing the required type of the pattern, or the matched value type."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "pattern_variable_assignment_inside_guard", unico: "CompileTimeErrorCode.PATTERN_VARIABLE_ASSIGNMENT_INSIDE_GUARD", mensagem: "Pattern variables can't be assigned inside the guard of the enclosing guarded pattern.", correcao: Some("Try assigning to a different variable."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_pattern_variable_in_shared_case_scope", unico: "CompileTimeErrorCode.PATTERN_VARIABLE_SHARED_CASE_SCOPE_DIFFERENT_FINALITY_OR_TYPE", mensagem: "The variable '{0}' doesn't have the same type and/or finality in all cases that share this body.", correcao: Some("Try declaring the variable pattern with the same type and finality in all cases."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_pattern_variable_in_shared_case_scope", unico: "CompileTimeErrorCode.PATTERN_VARIABLE_SHARED_CASE_SCOPE_HAS_LABEL", mensagem: "The variable '{0}' is not available because there is a label or 'default' case.", correcao: Some("Try removing the label, or providing the 'default' case with its own body."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_pattern_variable_in_shared_case_scope", unico: "CompileTimeErrorCode.PATTERN_VARIABLE_SHARED_CASE_SCOPE_NOT_ALL_CASES", mensagem: "The variable '{0}' is available in some, but not all cases that share this body.", correcao: Some("Try declaring the variable pattern with the same type and finality in all cases."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "positional_field_in_object_pattern", unico: "CompileTimeErrorCode.POSITIONAL_FIELD_IN_OBJECT_PATTERN", mensagem: "Object patterns can only use named fields.", correcao: Some("Try specifying the field name."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "positional_super_formal_parameter_with_positional_argument", unico: "CompileTimeErrorCode.POSITIONAL_SUPER_FORMAL_PARAMETER_WITH_POSITIONAL_ARGUMENT", mensagem: "Positional super parameters can't be used when the super constructor invocation has a positional argument.", correcao: Some("Try making all the positional parameters passed to the super constructor be either all super parameters or all normal parameters."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "prefix_collides_with_top_level_member", unico: "CompileTimeErrorCode.PREFIX_COLLIDES_WITH_TOP_LEVEL_MEMBER", mensagem: "The name '{0}' is already used as an import prefix and can't be used to name a top-level element.", correcao: Some("Try renaming either the top-level element or the prefix."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "prefix_identifier_not_followed_by_dot", unico: "CompileTimeErrorCode.PREFIX_IDENTIFIER_NOT_FOLLOWED_BY_DOT", mensagem: "The name '{0}' refers to an import prefix, so it must be followed by '.'.", correcao: Some("Try correcting the name to refer to something other than a prefix, or renaming the prefix."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "prefix_shadowed_by_local_declaration", unico: "CompileTimeErrorCode.PREFIX_SHADOWED_BY_LOCAL_DECLARATION", mensagem: "The prefix '{0}' can't be used here because it's shadowed by a local declaration.", correcao: Some("Try renaming either the prefix or the local declaration."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "private_collision_in_mixin_application", unico: "CompileTimeErrorCode.PRIVATE_COLLISION_IN_MIXIN_APPLICATION", mensagem: "The private name '{0}', defined by '{1}', conflicts with the same name defined by '{2}'.", correcao: Some("Try removing '{1}' from the 'with' clause."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "private_optional_parameter", unico: "CompileTimeErrorCode.PRIVATE_OPTIONAL_PARAMETER", mensagem: "Named parameters can't start with an underscore.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "private_setter", unico: "CompileTimeErrorCode.PRIVATE_SETTER", mensagem: "The setter '{0}' is private and can't be accessed outside the library that declares it.", correcao: Some("Try making it public."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "read_potentially_unassigned_final", unico: "CompileTimeErrorCode.READ_POTENTIALLY_UNASSIGNED_FINAL", mensagem: "The final variable '{0}' can't be read because it's potentially unassigned at this point.", correcao: Some("Ensure that it is assigned on necessary execution paths."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "record_literal_one_positional_no_trailing_comma", unico: "CompileTimeErrorCode.RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA", mensagem: "A record literal with exactly one positional field requires a trailing comma.", correcao: Some("Try adding a trailing comma."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "recursive_compile_time_constant", unico: "CompileTimeErrorCode.RECURSIVE_COMPILE_TIME_CONSTANT", mensagem: "The compile-time constant expression depends on itself.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "recursive_constant_constructor", unico: "CompileTimeErrorCode.RECURSIVE_CONSTANT_CONSTRUCTOR", mensagem: "The constant constructor depends on itself.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "recursive_constructor_redirect", unico: "CompileTimeErrorCode.RECURSIVE_CONSTRUCTOR_REDIRECT", mensagem: "Constructors can't redirect to themselves either directly or indirectly.", correcao: Some("Try changing one of the constructors in the loop to not redirect."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "recursive_constructor_redirect", unico: "CompileTimeErrorCode.RECURSIVE_FACTORY_REDIRECT", mensagem: "Constructors can't redirect to themselves either directly or indirectly.", correcao: Some("Try changing one of the constructors in the loop to not redirect."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "recursive_interface_inheritance", unico: "CompileTimeErrorCode.RECURSIVE_INTERFACE_INHERITANCE", mensagem: "'{0}' can't be a superinterface of itself: {1}.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "recursive_interface_inheritance", unico: "CompileTimeErrorCode.RECURSIVE_INTERFACE_INHERITANCE_EXTENDS", mensagem: "'{0}' can't extend itself.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "recursive_interface_inheritance", unico: "CompileTimeErrorCode.RECURSIVE_INTERFACE_INHERITANCE_IMPLEMENTS", mensagem: "'{0}' can't implement itself.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "recursive_interface_inheritance", unico: "CompileTimeErrorCode.RECURSIVE_INTERFACE_INHERITANCE_ON", mensagem: "'{0}' can't use itself as a superclass constraint.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "recursive_interface_inheritance", unico: "CompileTimeErrorCode.RECURSIVE_INTERFACE_INHERITANCE_WITH", mensagem: "'{0}' can't use itself as a mixin.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "redirect_generative_to_missing_constructor", unico: "CompileTimeErrorCode.REDIRECT_GENERATIVE_TO_MISSING_CONSTRUCTOR", mensagem: "The constructor '{0}' couldn't be found in '{1}'.", correcao: Some("Try redirecting to a different constructor, or defining the constructor named '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "redirect_generative_to_non_generative_constructor", unico: "CompileTimeErrorCode.REDIRECT_GENERATIVE_TO_NON_GENERATIVE_CONSTRUCTOR", mensagem: "Generative constructors can't redirect to a factory constructor.", correcao: Some("Try redirecting to a different constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "redirect_to_abstract_class_constructor", unico: "CompileTimeErrorCode.REDIRECT_TO_ABSTRACT_CLASS_CONSTRUCTOR", mensagem: "The redirecting constructor '{0}' can't redirect to a constructor of the abstract class '{1}'.", correcao: Some("Try redirecting to a constructor of a different class."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "redirect_to_invalid_function_type", unico: "CompileTimeErrorCode.REDIRECT_TO_INVALID_FUNCTION_TYPE", mensagem: "The redirected constructor '{0}' has incompatible parameters with '{1}'.", correcao: Some("Try redirecting to a different constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "redirect_to_invalid_return_type", unico: "CompileTimeErrorCode.REDIRECT_TO_INVALID_RETURN_TYPE", mensagem: "The return type '{0}' of the redirected constructor isn't a subtype of '{1}'.", correcao: Some("Try redirecting to a different constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "redirect_to_missing_constructor", unico: "CompileTimeErrorCode.REDIRECT_TO_MISSING_CONSTRUCTOR", mensagem: "The constructor '{0}' couldn't be found in '{1}'.", correcao: Some("Try redirecting to a different constructor, or define the constructor named '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "redirect_to_non_class", unico: "CompileTimeErrorCode.REDIRECT_TO_NON_CLASS", mensagem: "The name '{0}' isn't a type and can't be used in a redirected constructor.", correcao: Some("Try redirecting to a different constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "redirect_to_non_const_constructor", unico: "CompileTimeErrorCode.REDIRECT_TO_NON_CONST_CONSTRUCTOR", mensagem: "A constant redirecting constructor can't redirect to a non-constant constructor.", correcao: Some("Try redirecting to a different constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "redirect_to_type_alias_expands_to_type_parameter", unico: "CompileTimeErrorCode.REDIRECT_TO_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER", mensagem: "A redirecting constructor can't redirect to a type alias that expands to a type parameter.", correcao: Some("Try replacing it with a class."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "referenced_before_declaration", unico: "CompileTimeErrorCode.REFERENCED_BEFORE_DECLARATION", mensagem: "Local variable '{0}' can't be referenced before it is declared.", correcao: Some("Try moving the declaration to before the first use, or renaming the local variable so that it doesn't hide a name from an enclosing scope."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "refutable_pattern_in_irrefutable_context", unico: "CompileTimeErrorCode.REFUTABLE_PATTERN_IN_IRREFUTABLE_CONTEXT", mensagem: "Refutable patterns can't be used in an irrefutable context.", correcao: Some("Try using an if-case, a 'switch' statement, or a 'switch' expression instead."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "relational_pattern_operand_type_not_assignable", unico: "CompileTimeErrorCode.RELATIONAL_PATTERN_OPERAND_TYPE_NOT_ASSIGNABLE", mensagem: "The constant expression type '{0}' is not assignable to the parameter type '{1}' of the '{2}' operator.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "relational_pattern_operator_return_type_not_assignable_to_bool", unico: "CompileTimeErrorCode.RELATIONAL_PATTERN_OPERATOR_RETURN_TYPE_NOT_ASSIGNABLE_TO_BOOL", mensagem: "The return type of operators used in relational patterns must be assignable to 'bool'.", correcao: Some("Try updating the operator declaration to return 'bool'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "rest_element_in_map_pattern", unico: "CompileTimeErrorCode.REST_ELEMENT_IN_MAP_PATTERN", mensagem: "A map pattern can't contain a rest pattern.", correcao: Some("Try removing the rest pattern."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "rethrow_outside_catch", unico: "CompileTimeErrorCode.RETHROW_OUTSIDE_CATCH", mensagem: "A rethrow must be inside of a catch clause.", correcao: Some("Try moving the expression into a catch clause, or using a 'throw' expression."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "return_in_generative_constructor", unico: "CompileTimeErrorCode.RETURN_IN_GENERATIVE_CONSTRUCTOR", mensagem: "Constructors can't return values.", correcao: Some("Try removing the return statement or using a factory constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "return_in_generator", unico: "CompileTimeErrorCode.RETURN_IN_GENERATOR", mensagem: "Can't return a value from a generator function that uses the 'async*' or 'sync*' modifier.", correcao: Some("Try replacing 'return' with 'yield', using a block function body, or changing the method body modifier."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "return_of_invalid_type_from_closure", unico: "CompileTimeErrorCode.RETURN_OF_INVALID_TYPE_FROM_CLOSURE", mensagem: "The returned type '{0}' isn't returnable from a '{1}' function, as required by the closure's context.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "return_of_invalid_type", unico: "CompileTimeErrorCode.RETURN_OF_INVALID_TYPE_FROM_CONSTRUCTOR", mensagem: "A value of type '{0}' can't be returned from the constructor '{2}' because it has a return type of '{1}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "return_of_invalid_type", unico: "CompileTimeErrorCode.RETURN_OF_INVALID_TYPE_FROM_FUNCTION", mensagem: "A value of type '{0}' can't be returned from the function '{2}' because it has a return type of '{1}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "return_of_invalid_type", unico: "CompileTimeErrorCode.RETURN_OF_INVALID_TYPE_FROM_METHOD", mensagem: "A value of type '{0}' can't be returned from the method '{2}' because it has a return type of '{1}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "return_without_value", unico: "CompileTimeErrorCode.RETURN_WITHOUT_VALUE", mensagem: "The return value is missing after 'return'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_use_of_type_outside_library", unico: "CompileTimeErrorCode.SEALED_CLASS_SUBTYPE_OUTSIDE_OF_LIBRARY", mensagem: "The class '{0}' can't be extended, implemented, or mixed in outside of its library because it's a sealed class.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "collection_element_from_deferred_library", unico: "CompileTimeErrorCode.SET_ELEMENT_FROM_DEFERRED_LIBRARY", mensagem: "Constant values from a deferred library can't be used as values in a 'const' set literal.", correcao: Some("Try removing the keyword 'const' from the set literal or removing the keyword 'deferred' from the import."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "set_element_type_not_assignable", unico: "CompileTimeErrorCode.SET_ELEMENT_TYPE_NOT_ASSIGNABLE", mensagem: "The element type '{0}' can't be assigned to the set type '{1}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "shared_deferred_prefix", unico: "CompileTimeErrorCode.SHARED_DEFERRED_PREFIX", mensagem: "The prefix of a deferred import can't be used in other import directives.", correcao: Some("Try renaming one of the prefixes."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "spread_expression_from_deferred_library", unico: "CompileTimeErrorCode.SPREAD_EXPRESSION_FROM_DEFERRED_LIBRARY", mensagem: "Constant values from a deferred library can't be spread into a const literal.", correcao: Some("Try making the deferred import non-deferred."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "static_access_to_instance_member", unico: "CompileTimeErrorCode.STATIC_ACCESS_TO_INSTANCE_MEMBER", mensagem: "Instance member '{0}' can't be accessed using static access.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "subtype_of_base_or_final_is_not_base_final_or_sealed", unico: "CompileTimeErrorCode.SUBTYPE_OF_BASE_IS_NOT_BASE_FINAL_OR_SEALED", mensagem: "The type '{0}' must be 'base', 'final' or 'sealed' because the supertype '{1}' is 'base'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "subtype_of_base_or_final_is_not_base_final_or_sealed", unico: "CompileTimeErrorCode.SUBTYPE_OF_FINAL_IS_NOT_BASE_FINAL_OR_SEALED", mensagem: "The type '{0}' must be 'base', 'final' or 'sealed' because the supertype '{1}' is 'final'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "super_formal_parameter_type_is_not_subtype_of_associated", unico: "CompileTimeErrorCode.SUPER_FORMAL_PARAMETER_TYPE_IS_NOT_SUBTYPE_OF_ASSOCIATED", mensagem: "The type '{0}' of this parameter isn't a subtype of the type '{1}' of the associated super constructor parameter.", correcao: Some("Try removing the explicit type annotation from the parameter."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "super_formal_parameter_without_associated_named", unico: "CompileTimeErrorCode.SUPER_FORMAL_PARAMETER_WITHOUT_ASSOCIATED_NAMED", mensagem: "No associated named super constructor parameter.", correcao: Some("Try changing the name to the name of an existing named super constructor parameter, or creating such named parameter."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "super_formal_parameter_without_associated_positional", unico: "CompileTimeErrorCode.SUPER_FORMAL_PARAMETER_WITHOUT_ASSOCIATED_POSITIONAL", mensagem: "No associated positional super constructor parameter.", correcao: Some("Try using a normal parameter, or adding more positional parameters to the super constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "super_initializer_in_object", unico: "CompileTimeErrorCode.SUPER_INITIALIZER_IN_OBJECT", mensagem: "The class 'Object' can't invoke a constructor from a superclass.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "super_invocation_not_last", unico: "CompileTimeErrorCode.SUPER_INVOCATION_NOT_LAST", mensagem: "The superconstructor call must be last in an initializer list: '{0}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "super_in_enum_constructor", unico: "CompileTimeErrorCode.SUPER_IN_ENUM_CONSTRUCTOR", mensagem: "The enum constructor can't have a 'super' initializer.", correcao: Some("Try removing the 'super' invocation."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "super_in_extension", unico: "CompileTimeErrorCode.SUPER_IN_EXTENSION", mensagem: "The 'super' keyword can't be used in an extension because an extension doesn't have a superclass.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "super_in_extension_type", unico: "CompileTimeErrorCode.SUPER_IN_EXTENSION_TYPE", mensagem: "The 'super' keyword can't be used in an extension type because an extension type doesn't have a superclass.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "super_in_invalid_context", unico: "CompileTimeErrorCode.SUPER_IN_INVALID_CONTEXT", mensagem: "Invalid context for 'super' invocation.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "super_in_redirecting_constructor", unico: "CompileTimeErrorCode.SUPER_IN_REDIRECTING_CONSTRUCTOR", mensagem: "The redirecting constructor can't have a 'super' initializer.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "switch_case_completes_normally", unico: "CompileTimeErrorCode.SWITCH_CASE_COMPLETES_NORMALLY", mensagem: "The 'case' shouldn't complete normally.", correcao: Some("Try adding 'break', 'return', or 'throw'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "tearoff_of_generative_constructor_of_abstract_class", unico: "CompileTimeErrorCode.TEAROFF_OF_GENERATIVE_CONSTRUCTOR_OF_ABSTRACT_CLASS", mensagem: "A generative constructor of an abstract class can't be torn off.", correcao: Some("Try tearing off a constructor of a concrete class, or a non-generative constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "throw_of_invalid_type", unico: "CompileTimeErrorCode.THROW_OF_INVALID_TYPE", mensagem: "The type '{0}' of the thrown expression must be assignable to 'Object'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "top_level_cycle", unico: "CompileTimeErrorCode.TOP_LEVEL_CYCLE", mensagem: "The type of '{0}' can't be inferred because it depends on itself through the cycle: {1}.", correcao: Some("Try adding an explicit type to one or more of the variables in the cycle in order to break the cycle."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "type_alias_cannot_reference_itself", unico: "CompileTimeErrorCode.TYPE_ALIAS_CANNOT_REFERENCE_ITSELF", mensagem: "Typedefs can't reference themselves directly or recursively via another typedef.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "type_annotation_deferred_class", unico: "CompileTimeErrorCode.TYPE_ANNOTATION_DEFERRED_CLASS", mensagem: "The deferred type '{0}' can't be used in a declaration, cast, or type test.", correcao: Some("Try using a different type, or changing the import to not be deferred."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "type_argument_not_matching_bounds", unico: "CompileTimeErrorCode.TYPE_ARGUMENT_NOT_MATCHING_BOUNDS", mensagem: "'{0}' doesn't conform to the bound '{2}' of the type parameter '{1}'.", correcao: Some("Try using a type that is or is a subclass of '{2}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "type_parameter_referenced_by_static", unico: "CompileTimeErrorCode.TYPE_PARAMETER_REFERENCED_BY_STATIC", mensagem: "Static members can't reference type parameters of the class.", correcao: Some("Try removing the reference to the type parameter, or making the member an instance member."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "type_parameter_supertype_of_its_bound", unico: "CompileTimeErrorCode.TYPE_PARAMETER_SUPERTYPE_OF_ITS_BOUND", mensagem: "'{0}' can't be a supertype of its upper bound.", correcao: Some("Try using a type that is the same as or a subclass of '{1}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "type_test_with_non_type", unico: "CompileTimeErrorCode.TYPE_TEST_WITH_NON_TYPE", mensagem: "The name '{0}' isn't a type and can't be used in an 'is' expression.", correcao: Some("Try correcting the name to match an existing type."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "type_test_with_undefined_name", unico: "CompileTimeErrorCode.TYPE_TEST_WITH_UNDEFINED_NAME", mensagem: "The name '{0}' isn't defined, so it can't be used in an 'is' expression.", correcao: Some("Try changing the name to the name of an existing type, or creating a type with the name '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "unchecked_use_of_nullable_value", unico: "CompileTimeErrorCode.UNCHECKED_INVOCATION_OF_NULLABLE_VALUE", mensagem: "The function can't be unconditionally invoked because it can be 'null'.", correcao: Some("Try adding a null check ('!')."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "unchecked_use_of_nullable_value", unico: "CompileTimeErrorCode.UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE", mensagem: "The method '{0}' can't be unconditionally invoked because the receiver can be 'null'.", correcao: Some("Try making the call conditional (using '?.') or adding a null check to the target ('!')."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "unchecked_use_of_nullable_value", unico: "CompileTimeErrorCode.UNCHECKED_OPERATOR_INVOCATION_OF_NULLABLE_VALUE", mensagem: "The operator '{0}' can't be unconditionally invoked because the receiver can be 'null'.", correcao: Some("Try adding a null check to the target ('!')."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "unchecked_use_of_nullable_value", unico: "CompileTimeErrorCode.UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE", mensagem: "The property '{0}' can't be unconditionally accessed because the receiver can be 'null'.", correcao: Some("Try making the access conditional (using '?.') or adding a null check to the target ('!')."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "unchecked_use_of_nullable_value", unico: "CompileTimeErrorCode.UNCHECKED_USE_OF_NULLABLE_VALUE_AS_CONDITION", mensagem: "A nullable expression can't be used as a condition.", correcao: Some("Try checking that the value isn't 'null' before using it as a condition."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "unchecked_use_of_nullable_value", unico: "CompileTimeErrorCode.UNCHECKED_USE_OF_NULLABLE_VALUE_AS_ITERATOR", mensagem: "A nullable expression can't be used as an iterator in a for-in loop.", correcao: Some("Try checking that the value isn't 'null' before using it as an iterator."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "unchecked_use_of_nullable_value", unico: "CompileTimeErrorCode.UNCHECKED_USE_OF_NULLABLE_VALUE_IN_SPREAD", mensagem: "A nullable expression can't be used in a spread.", correcao: Some("Try checking that the value isn't 'null' before using it in a spread, or use a null-aware spread."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "unchecked_use_of_nullable_value", unico: "CompileTimeErrorCode.UNCHECKED_USE_OF_NULLABLE_VALUE_IN_YIELD_EACH", mensagem: "A nullable expression can't be used in a yield-each statement.", correcao: Some("Try checking that the value isn't 'null' before using it in a yield-each statement."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_annotation", unico: "CompileTimeErrorCode.UNDEFINED_ANNOTATION", mensagem: "Undefined name '{0}' used as an annotation.", correcao: Some("Try defining the name or importing it from another library."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_class", unico: "CompileTimeErrorCode.UNDEFINED_CLASS", mensagem: "Undefined class '{0}'.", correcao: Some("Try changing the name to the name of an existing class, or creating a class with the name '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_class", unico: "CompileTimeErrorCode.UNDEFINED_CLASS_BOOLEAN", mensagem: "Undefined class '{0}'.", correcao: Some("Try using the type 'bool'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_constructor_in_initializer", unico: "CompileTimeErrorCode.UNDEFINED_CONSTRUCTOR_IN_INITIALIZER", mensagem: "The class '{0}' doesn't have a constructor named '{1}'.", correcao: Some("Try defining a constructor named '{1}' in '{0}', or invoking a different constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_constructor_in_initializer", unico: "CompileTimeErrorCode.UNDEFINED_CONSTRUCTOR_IN_INITIALIZER_DEFAULT", mensagem: "The class '{0}' doesn't have an unnamed constructor.", correcao: Some("Try defining an unnamed constructor in '{0}', or invoking a different constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_enum_constant", unico: "CompileTimeErrorCode.UNDEFINED_ENUM_CONSTANT", mensagem: "There's no constant named '{0}' in '{1}'.", correcao: Some("Try correcting the name to the name of an existing constant, or defining a constant named '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_enum_constructor", unico: "CompileTimeErrorCode.UNDEFINED_ENUM_CONSTRUCTOR_NAMED", mensagem: "The enum doesn't have a constructor named '{0}'.", correcao: Some("Try correcting the name to the name of an existing constructor, or defining constructor with the name '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_enum_constructor", unico: "CompileTimeErrorCode.UNDEFINED_ENUM_CONSTRUCTOR_UNNAMED", mensagem: "The enum doesn't have an unnamed constructor.", correcao: Some("Try adding the name of an existing constructor, or defining an unnamed constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_extension_getter", unico: "CompileTimeErrorCode.UNDEFINED_EXTENSION_GETTER", mensagem: "The getter '{0}' isn't defined for the extension '{1}'.", correcao: Some("Try correcting the name to the name of an existing getter, or defining a getter named '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_extension_method", unico: "CompileTimeErrorCode.UNDEFINED_EXTENSION_METHOD", mensagem: "The method '{0}' isn't defined for the extension '{1}'.", correcao: Some("Try correcting the name to the name of an existing method, or defining a method named '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_extension_operator", unico: "CompileTimeErrorCode.UNDEFINED_EXTENSION_OPERATOR", mensagem: "The operator '{0}' isn't defined for the extension '{1}'.", correcao: Some("Try defining the operator '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_extension_setter", unico: "CompileTimeErrorCode.UNDEFINED_EXTENSION_SETTER", mensagem: "The setter '{0}' isn't defined for the extension '{1}'.", correcao: Some("Try correcting the name to the name of an existing setter, or defining a setter named '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_function", unico: "CompileTimeErrorCode.UNDEFINED_FUNCTION", mensagem: "The function '{0}' isn't defined.", correcao: Some("Try importing the library that defines '{0}', correcting the name to the name of an existing function, or defining a function named '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_getter", unico: "CompileTimeErrorCode.UNDEFINED_GETTER", mensagem: "The getter '{0}' isn't defined for the type '{1}'.", correcao: Some("Try importing the library that defines '{0}', correcting the name to the name of an existing getter, or defining a getter or field named '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_getter", unico: "CompileTimeErrorCode.UNDEFINED_GETTER_ON_FUNCTION_TYPE", mensagem: "The getter '{0}' isn't defined for the '{1}' function type.", correcao: Some("Try wrapping the function type alias in parentheses in order to access '{0}' as an extension getter on 'Type'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_identifier", unico: "CompileTimeErrorCode.UNDEFINED_IDENTIFIER", mensagem: "Undefined name '{0}'.", correcao: Some("Try correcting the name to one that is defined, or defining the name."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_identifier_await", unico: "CompileTimeErrorCode.UNDEFINED_IDENTIFIER_AWAIT", mensagem: "Undefined name 'await' in function body not marked with 'async'.", correcao: Some("Try correcting the name to one that is defined, defining the name, or adding 'async' to the enclosing function body."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_method", unico: "CompileTimeErrorCode.UNDEFINED_METHOD", mensagem: "The method '{0}' isn't defined for the type '{1}'.", correcao: Some("Try correcting the name to the name of an existing method, or defining a method named '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_method", unico: "CompileTimeErrorCode.UNDEFINED_METHOD_ON_FUNCTION_TYPE", mensagem: "The method '{0}' isn't defined for the '{1}' function type.", correcao: Some("Try wrapping the function type alias in parentheses in order to access '{0}' as an extension method on 'Type'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_named_parameter", unico: "CompileTimeErrorCode.UNDEFINED_NAMED_PARAMETER", mensagem: "The named parameter '{0}' isn't defined.", correcao: Some("Try correcting the name to an existing named parameter's name, or defining a named parameter with the name '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_operator", unico: "CompileTimeErrorCode.UNDEFINED_OPERATOR", mensagem: "The operator '{0}' isn't defined for the type '{1}'.", correcao: Some("Try defining the operator '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_prefixed_name", unico: "CompileTimeErrorCode.UNDEFINED_PREFIXED_NAME", mensagem: "The name '{0}' is being referenced through the prefix '{1}', but it isn't defined in any of the libraries imported using that prefix.", correcao: Some("Try correcting the prefix or importing the library that defines '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_setter", unico: "CompileTimeErrorCode.UNDEFINED_SETTER", mensagem: "The setter '{0}' isn't defined for the type '{1}'.", correcao: Some("Try importing the library that defines '{0}', correcting the name to the name of an existing setter, or defining a setter or field named '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_setter", unico: "CompileTimeErrorCode.UNDEFINED_SETTER_ON_FUNCTION_TYPE", mensagem: "The setter '{0}' isn't defined for the '{1}' function type.", correcao: Some("Try wrapping the function type alias in parentheses in order to access '{0}' as an extension getter on 'Type'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_super_member", unico: "CompileTimeErrorCode.UNDEFINED_SUPER_GETTER", mensagem: "The getter '{0}' isn't defined in a superclass of '{1}'.", correcao: Some("Try correcting the name to the name of an existing getter, or defining a getter or field named '{0}' in a superclass."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_super_member", unico: "CompileTimeErrorCode.UNDEFINED_SUPER_METHOD", mensagem: "The method '{0}' isn't defined in a superclass of '{1}'.", correcao: Some("Try correcting the name to the name of an existing method, or defining a method named '{0}' in a superclass."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_super_member", unico: "CompileTimeErrorCode.UNDEFINED_SUPER_OPERATOR", mensagem: "The operator '{0}' isn't defined in a superclass of '{1}'.", correcao: Some("Try defining the operator '{0}' in a superclass."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "undefined_super_member", unico: "CompileTimeErrorCode.UNDEFINED_SUPER_SETTER", mensagem: "The setter '{0}' isn't defined in a superclass of '{1}'.", correcao: Some("Try correcting the name to the name of an existing setter, or defining a setter or field named '{0}' in a superclass."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "unqualified_reference_to_non_local_static_member", unico: "CompileTimeErrorCode.UNQUALIFIED_REFERENCE_TO_NON_LOCAL_STATIC_MEMBER", mensagem: "Static members from supertypes must be qualified by the name of the defining type.", correcao: Some("Try adding '{0}.' before the name."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "unqualified_reference_to_static_member_of_extended_type", unico: "CompileTimeErrorCode.UNQUALIFIED_REFERENCE_TO_STATIC_MEMBER_OF_EXTENDED_TYPE", mensagem: "Static members from the extended type or one of its superclasses must be qualified by the name of the defining type.", correcao: Some("Try adding '{0}.' before the name."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "uri_does_not_exist", unico: "CompileTimeErrorCode.URI_DOES_NOT_EXIST", mensagem: "Target of URI doesn't exist: '{0}'.", correcao: Some("Try creating the file referenced by the URI, or try using a URI for a file that does exist."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "uri_has_not_been_generated", unico: "CompileTimeErrorCode.URI_HAS_NOT_BEEN_GENERATED", mensagem: "Target of URI hasn't been generated: '{0}'.", correcao: Some("Try running the generator that will generate the file referenced by the URI."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "uri_with_interpolation", unico: "CompileTimeErrorCode.URI_WITH_INTERPOLATION", mensagem: "URIs can't use string interpolation.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "use_of_native_extension", unico: "CompileTimeErrorCode.USE_OF_NATIVE_EXTENSION", mensagem: "Dart native extensions are deprecated and aren't available in Dart 2.15.", correcao: Some("Try using dart:ffi for C interop."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "use_of_void_result", unico: "CompileTimeErrorCode.USE_OF_VOID_RESULT", mensagem: "This expression has a type of 'void' so its value can't be used.", correcao: Some("Try checking to see if you're using the correct API; there might be a function or call that returns void you didn't expect. Also check type parameters and variables which might also be void."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "values_declaration_in_enum", unico: "CompileTimeErrorCode.VALUES_DECLARATION_IN_ENUM", mensagem: "A member named 'values' can't be declared in an enum.", correcao: Some("Try using a different name."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "variable_type_mismatch", unico: "CompileTimeErrorCode.VARIABLE_TYPE_MISMATCH", mensagem: "A value of type '{0}' can't be assigned to a const variable of type '{1}'.", correcao: Some("Try using a subtype, or removing the 'const' keyword"), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "wrong_explicit_type_parameter_variance_in_superinterface", unico: "CompileTimeErrorCode.WRONG_EXPLICIT_TYPE_PARAMETER_VARIANCE_IN_SUPERINTERFACE", mensagem: "'{0}' is an '{1}' type parameter and can't be used in an '{2}' position in '{3}'.", correcao: Some("Try using 'in' type parameters in 'in' positions and 'out' type parameters in 'out' positions in the superinterface."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "wrong_number_of_parameters_for_operator", unico: "CompileTimeErrorCode.WRONG_NUMBER_OF_PARAMETERS_FOR_OPERATOR", mensagem: "Operator '{0}' should declare exactly {1} parameters, but {2} found.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "wrong_number_of_parameters_for_operator", unico: "CompileTimeErrorCode.WRONG_NUMBER_OF_PARAMETERS_FOR_OPERATOR_MINUS", mensagem: "Operator '-' should declare 0 or 1 parameter, but {0} found.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "wrong_number_of_parameters_for_setter", unico: "CompileTimeErrorCode.WRONG_NUMBER_OF_PARAMETERS_FOR_SETTER", mensagem: "Setters must declare exactly one required positional parameter.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "wrong_number_of_type_arguments", unico: "CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS", mensagem: "The type '{0}' is declared with {1} type parameters, but {2} type arguments were given.", correcao: Some("Try adjusting the number of type arguments to match the number of type parameters."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "wrong_number_of_type_arguments_function", unico: "CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_ANONYMOUS_FUNCTION", mensagem: "This function is declared with {0} type parameters, but {1} type arguments were given.", correcao: Some("Try adjusting the number of type arguments to match the number of type parameters."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "wrong_number_of_type_arguments_constructor", unico: "CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_CONSTRUCTOR", mensagem: "The constructor '{0}.{1}' doesn't have type parameters.", correcao: Some("Try moving type arguments to after the type name."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "wrong_number_of_type_arguments_enum", unico: "CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_ENUM", mensagem: "The enum is declared with {0} type parameters, but {1} type arguments were given.", correcao: Some("Try adjusting the number of type arguments."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "wrong_number_of_type_arguments_extension", unico: "CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_EXTENSION", mensagem: "The extension '{0}' is declared with {1} type parameters, but {2} type arguments were given.", correcao: Some("Try adjusting the number of type arguments."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "wrong_number_of_type_arguments_function", unico: "CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION", mensagem: "The function '{0}' is declared with {1} type parameters, but {2} type arguments were given.", correcao: Some("Try adjusting the number of type arguments to match the number of type parameters."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "wrong_number_of_type_arguments_method", unico: "CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_METHOD", mensagem: "The method '{0}' is declared with {1} type parameters, but {2} type arguments are given.", correcao: Some("Try adjusting the number of type arguments."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "wrong_type_parameter_variance_in_superinterface", unico: "CompileTimeErrorCode.WRONG_TYPE_PARAMETER_VARIANCE_IN_SUPERINTERFACE", mensagem: "'{0}' can't be used contravariantly or invariantly in '{1}'.", correcao: Some("Try not using class type parameters in types of formal parameters of function types, nor in explicitly contravariant or invariant superinterfaces."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "wrong_type_parameter_variance_position", unico: "CompileTimeErrorCode.WRONG_TYPE_PARAMETER_VARIANCE_POSITION", mensagem: "The '{0}' type parameter '{1}' can't be used in an '{2}' position.", correcao: Some("Try removing the type parameter or change the explicit variance modifier declaration for the type parameter to another one of 'in', 'out', or 'inout'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "yield_in_non_generator", unico: "CompileTimeErrorCode.YIELD_EACH_IN_NON_GENERATOR", mensagem: "Yield-each statements must be in a generator function (one marked with either 'async*' or 'sync*').", correcao: Some("Try adding 'async*' or 'sync*' to the enclosing function."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "yield_of_invalid_type", unico: "CompileTimeErrorCode.YIELD_EACH_OF_INVALID_TYPE", mensagem: "The type '{0}' implied by the 'yield*' expression must be assignable to '{1}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "yield_in_non_generator", unico: "CompileTimeErrorCode.YIELD_IN_NON_GENERATOR", mensagem: "Yield statements must be in a generator function (one marked with either 'async*' or 'sync*').", correcao: Some("Try adding 'async*' or 'sync*' to the enclosing function."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "yield_of_invalid_type", unico: "CompileTimeErrorCode.YIELD_OF_INVALID_TYPE", mensagem: "A yielded value of type '{0}' must be assignable to '{1}'.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "dead_null_aware_expression", unico: "StaticWarningCode.DEAD_NULL_AWARE_EXPRESSION", mensagem: "The left operand can't be null, so the right operand is never executed.", correcao: Some("Try removing the operator and the right operand."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_null_aware_operator", unico: "StaticWarningCode.INVALID_NULL_AWARE_OPERATOR", mensagem: "The receiver can't be null, so the null-aware operator '{0}' is unnecessary.", correcao: Some("Try replacing the operator '{0}' with '{1}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_null_aware_operator", unico: "StaticWarningCode.INVALID_NULL_AWARE_OPERATOR_AFTER_SHORT_CIRCUIT", mensagem: "The receiver can't be 'null' because of short-circuiting, so the null-aware operator '{0}' can't be used.", correcao: Some("Try replacing the operator '{0}' with '{1}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "missing_enum_constant_in_switch", unico: "StaticWarningCode.MISSING_ENUM_CONSTANT_IN_SWITCH", mensagem: "Missing case clause for '{0}'.", correcao: Some("Try adding a case clause for the missing constant, or adding a default clause."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unnecessary_non_null_assertion", unico: "StaticWarningCode.UNNECESSARY_NON_NULL_ASSERTION", mensagem: "The '!' will have no effect because the receiver can't be null.", correcao: Some("Try removing the '!' operator."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unnecessary_null_assert_pattern", unico: "StaticWarningCode.UNNECESSARY_NULL_ASSERT_PATTERN", mensagem: "The null-assert pattern will have no effect because the matched type isn't nullable.", correcao: Some("Try replacing the null-assert pattern with its nested pattern."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unnecessary_null_check_pattern", unico: "StaticWarningCode.UNNECESSARY_NULL_CHECK_PATTERN", mensagem: "The null-check pattern will have no effect because the matched type isn't nullable.", correcao: Some("Try replacing the null-check pattern with its nested pattern."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "argument_type_not_assignable_to_error_handler", unico: "WarningCode.ARGUMENT_TYPE_NOT_ASSIGNABLE_TO_ERROR_HANDLER", mensagem: "The argument type '{0}' can't be assigned to the parameter type '{1} Function(Object)' or '{1} Function(Object, StackTrace)'.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "assignment_of_do_not_store", unico: "WarningCode.ASSIGNMENT_OF_DO_NOT_STORE", mensagem: "'{0}' is marked 'doNotStore' and shouldn't be assigned to a field or top-level variable.", correcao: Some("Try removing the assignment."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "body_might_complete_normally_catch_error", unico: "WarningCode.BODY_MIGHT_COMPLETE_NORMALLY_CATCH_ERROR", mensagem: "This 'onError' handler must return a value assignable to '{0}', but ends without returning a value.", correcao: Some("Try adding a return statement."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "body_might_complete_normally_nullable", unico: "WarningCode.BODY_MIGHT_COMPLETE_NORMALLY_NULLABLE", mensagem: "This function has a nullable return type of '{0}', but ends without returning a value.", correcao: Some("Try adding a return statement, or if no value is ever returned, try changing the return type to 'void'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "cast_from_nullable_always_fails", unico: "WarningCode.CAST_FROM_NULLABLE_ALWAYS_FAILS", mensagem: "This cast will always throw an exception because the nullable local variable '{0}' is not assigned.", correcao: Some("Try giving it an initializer expression, or ensure that it's assigned on every execution path."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "cast_from_null_always_fails", unico: "WarningCode.CAST_FROM_NULL_ALWAYS_FAILS", mensagem: "This cast always throws an exception because the expression always evaluates to 'null'.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "constant_pattern_never_matches_value_type", unico: "WarningCode.CONSTANT_PATTERN_NEVER_MATCHES_VALUE_TYPE", mensagem: "The matched value type '{0}' can never be equal to this constant of type '{1}'.", correcao: Some("Try a constant of the same type as the matched value type."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "dead_code", unico: "WarningCode.DEAD_CODE", mensagem: "Dead code.", correcao: Some("Try removing the code, or fixing the code before it so that it can be reached."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "dead_code_catch_following_catch", unico: "WarningCode.DEAD_CODE_CATCH_FOLLOWING_CATCH", mensagem: "Dead code: Catch clauses after a 'catch (e)' or an 'on Object catch (e)' are never reached.", correcao: Some("Try reordering the catch clauses so that they can be reached, or removing the unreachable catch clauses."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "dead_code", unico: "WarningCode.DEAD_CODE_LATE_WILDCARD_VARIABLE_INITIALIZER", mensagem: "Dead code: The assigned-to wildcard variable is marked late and can never be referenced so this initializer will never be evaluated.", correcao: Some("Try removing the code, removing the late modifier or changing the variable to a non-wildcard."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "dead_code_on_catch_subtype", unico: "WarningCode.DEAD_CODE_ON_CATCH_SUBTYPE", mensagem: "Dead code: This on-catch block won't be executed because '{0}' is a subtype of '{1}' and hence will have been caught already.", correcao: Some("Try reordering the catch clauses so that this block can be reached, or removing the unreachable catch clause."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "deprecated_export_use", unico: "WarningCode.DEPRECATED_EXPORT_USE", mensagem: "The ability to import '{0}' indirectly is deprecated.", correcao: Some("Try importing '{0}' directly."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "deprecated_subtype_of_function", unico: "WarningCode.DEPRECATED_EXTENDS_FUNCTION", mensagem: "Extending 'Function' is deprecated.", correcao: Some("Try removing 'Function' from the 'extends' clause."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "deprecated_subtype_of_function", unico: "WarningCode.DEPRECATED_IMPLEMENTS_FUNCTION", mensagem: "Implementing 'Function' has no effect.", correcao: Some("Try removing 'Function' from the 'implements' clause."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "deprecated_subtype_of_function", unico: "WarningCode.DEPRECATED_MIXIN_FUNCTION", mensagem: "Mixing in 'Function' is deprecated.", correcao: Some("Try removing 'Function' from the 'with' clause."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "deprecated_new_in_comment_reference", unico: "WarningCode.DEPRECATED_NEW_IN_COMMENT_REFERENCE", mensagem: "Using the 'new' keyword in a comment reference is deprecated.", correcao: Some("Try referring to a constructor by its name."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "doc_directive_argument_wrong_format", unico: "WarningCode.DOC_DIRECTIVE_ARGUMENT_WRONG_FORMAT", mensagem: "The '{0}' argument must be formatted as {1}.", correcao: Some("Try formatting '{0}' as {1}."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "doc_directive_has_extra_arguments", unico: "WarningCode.DOC_DIRECTIVE_HAS_EXTRA_ARGUMENTS", mensagem: "The '{0}' directive has '{1}' arguments, but only '{2}' are expected.", correcao: Some("Try removing the extra arguments."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "doc_directive_has_unexpected_named_argument", unico: "WarningCode.DOC_DIRECTIVE_HAS_UNEXPECTED_NAMED_ARGUMENT", mensagem: "The '{0}' directive has an unexpected named argument, '{1}'.", correcao: Some("Try removing the unexpected argument."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "doc_directive_missing_closing_brace", unico: "WarningCode.DOC_DIRECTIVE_MISSING_CLOSING_BRACE", mensagem: "Doc directive is missing a closing curly brace ('}').", correcao: Some("Try closing the directive with a curly brace."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "doc_directive_missing_closing_tag", unico: "WarningCode.DOC_DIRECTIVE_MISSING_CLOSING_TAG", mensagem: "Doc directive is missing a closing tag.", correcao: Some("Try closing the directive with the appropriate closing tag, '{0}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "doc_directive_missing_argument", unico: "WarningCode.DOC_DIRECTIVE_MISSING_ONE_ARGUMENT", mensagem: "The '{0}' directive is missing a '{1}' argument.", correcao: Some("Try adding a '{1}' argument before the closing '}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "doc_directive_missing_opening_tag", unico: "WarningCode.DOC_DIRECTIVE_MISSING_OPENING_TAG", mensagem: "Doc directive is missing an opening tag.", correcao: Some("Try opening the directive with the appropriate opening tag, '{0}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "doc_directive_missing_argument", unico: "WarningCode.DOC_DIRECTIVE_MISSING_THREE_ARGUMENTS", mensagem: "The '{0}' directive is missing a '{1}', a '{2}', and a '{3}' argument.", correcao: Some("Try adding the missing arguments before the closing '}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "doc_directive_missing_argument", unico: "WarningCode.DOC_DIRECTIVE_MISSING_TWO_ARGUMENTS", mensagem: "The '{0}' directive is missing a '{1}' and a '{2}' argument.", correcao: Some("Try adding the missing arguments before the closing '}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "doc_directive_unknown", unico: "WarningCode.DOC_DIRECTIVE_UNKNOWN", mensagem: "Doc directive '{0}' is unknown.", correcao: Some("Try using one of the supported doc directives."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "doc_import_cannot_be_deferred", unico: "WarningCode.DOC_IMPORT_CANNOT_BE_DEFERRED", mensagem: "Doc imports can't be deferred.", correcao: Some("Try removing the 'deferred' keyword."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "doc_import_cannot_have_configurations", unico: "WarningCode.DOC_IMPORT_CANNOT_HAVE_CONFIGURATIONS", mensagem: "Doc imports can't have configurations.", correcao: Some("Try removing the configurations."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "duplicate_export", unico: "WarningCode.DUPLICATE_EXPORT", mensagem: "Duplicate export.", correcao: Some("Try removing all but one export of the library."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "duplicate_hidden_name", unico: "WarningCode.DUPLICATE_HIDDEN_NAME", mensagem: "Duplicate hidden name.", correcao: Some("Try removing the repeated name from the list of hidden members."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "duplicate_ignore", unico: "WarningCode.DUPLICATE_IGNORE", mensagem: "The diagnostic '{0}' doesn't need to be ignored here because it's already being ignored.", correcao: Some("Try removing the name from the list, or removing the whole comment if this is the only name in the list."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "duplicate_import", unico: "WarningCode.DUPLICATE_IMPORT", mensagem: "Duplicate import.", correcao: Some("Try removing all but one import of the library."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "duplicate_shown_name", unico: "WarningCode.DUPLICATE_SHOWN_NAME", mensagem: "Duplicate shown name.", correcao: Some("Try removing the repeated name from the list of shown members."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "equal_elements_in_set", unico: "WarningCode.EQUAL_ELEMENTS_IN_SET", mensagem: "Two elements in a set literal shouldn't be equal.", correcao: Some("Change or remove the duplicate element."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "equal_keys_in_map", unico: "WarningCode.EQUAL_KEYS_IN_MAP", mensagem: "Two keys in a map literal shouldn't be equal.", correcao: Some("Change or remove the duplicate key."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "inference_failure_on_collection_literal", unico: "WarningCode.INFERENCE_FAILURE_ON_COLLECTION_LITERAL", mensagem: "The type argument(s) of '{0}' can't be inferred.", correcao: Some("Use explicit type argument(s) for '{0}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "inference_failure_on_function_invocation", unico: "WarningCode.INFERENCE_FAILURE_ON_FUNCTION_INVOCATION", mensagem: "The type argument(s) of the function '{0}' can't be inferred.", correcao: Some("Use explicit type argument(s) for '{0}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "inference_failure_on_function_return_type", unico: "WarningCode.INFERENCE_FAILURE_ON_FUNCTION_RETURN_TYPE", mensagem: "The return type of '{0}' cannot be inferred.", correcao: Some("Declare the return type of '{0}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "inference_failure_on_generic_invocation", unico: "WarningCode.INFERENCE_FAILURE_ON_GENERIC_INVOCATION", mensagem: "The type argument(s) of the generic function type '{0}' can't be inferred.", correcao: Some("Use explicit type argument(s) for '{0}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "inference_failure_on_instance_creation", unico: "WarningCode.INFERENCE_FAILURE_ON_INSTANCE_CREATION", mensagem: "The type argument(s) of the constructor '{0}' can't be inferred.", correcao: Some("Use explicit type argument(s) for '{0}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "inference_failure_on_uninitialized_variable", unico: "WarningCode.INFERENCE_FAILURE_ON_UNINITIALIZED_VARIABLE", mensagem: "The type of {0} can't be inferred without either a type or initializer.", correcao: Some("Try specifying the type of the variable."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "inference_failure_on_untyped_parameter", unico: "WarningCode.INFERENCE_FAILURE_ON_UNTYPED_PARAMETER", mensagem: "The type of {0} can't be inferred; a type must be explicitly provided.", correcao: Some("Try specifying the type of the parameter."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "invalid_annotation_target", unico: "WarningCode.INVALID_ANNOTATION_TARGET", mensagem: "The annotation '{0}' can only be used on {1}.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_export_of_internal_element", unico: "WarningCode.INVALID_EXPORT_OF_INTERNAL_ELEMENT", mensagem: "The member '{0}' can't be exported as a part of a package's public API.", correcao: Some("Try using a hide clause to hide '{0}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_export_of_internal_element_indirectly", unico: "WarningCode.INVALID_EXPORT_OF_INTERNAL_ELEMENT_INDIRECTLY", mensagem: "The member '{0}' can't be exported as a part of a package's public API, but is indirectly exported as part of the signature of '{1}'.", correcao: Some("Try using a hide clause to hide '{0}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_factory_method_decl", unico: "WarningCode.INVALID_FACTORY_METHOD_DECL", mensagem: "Factory method '{0}' must have a return type.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_factory_method_impl", unico: "WarningCode.INVALID_FACTORY_METHOD_IMPL", mensagem: "Factory method '{0}' doesn't return a newly allocated object.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_internal_annotation", unico: "WarningCode.INVALID_INTERNAL_ANNOTATION", mensagem: "Only public elements in a package's private API can be annotated as being internal.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_language_version_override", unico: "WarningCode.INVALID_LANGUAGE_VERSION_OVERRIDE_AT_SIGN", mensagem: "The Dart language version override number must begin with '@dart'.", correcao: Some("Specify a Dart language version override with a comment like '// @dart = 2.0'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_language_version_override", unico: "WarningCode.INVALID_LANGUAGE_VERSION_OVERRIDE_EQUALS", mensagem: "The Dart language version override comment must be specified with an '=' character.", correcao: Some("Specify a Dart language version override with a comment like '// @dart = 2.0'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_language_version_override", unico: "WarningCode.INVALID_LANGUAGE_VERSION_OVERRIDE_GREATER", mensagem: "The language version override can't specify a version greater than the latest known language version: {0}.{1}.", correcao: Some("Try removing the language version override."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_language_version_override", unico: "WarningCode.INVALID_LANGUAGE_VERSION_OVERRIDE_LOCATION", mensagem: "The language version override must be specified before any declaration or directive.", correcao: Some("Try moving the language version override to the top of the file."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_language_version_override", unico: "WarningCode.INVALID_LANGUAGE_VERSION_OVERRIDE_LOWER_CASE", mensagem: "The Dart language version override comment must be specified with the word 'dart' in all lower case.", correcao: Some("Specify a Dart language version override with a comment like '// @dart = 2.0'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_language_version_override", unico: "WarningCode.INVALID_LANGUAGE_VERSION_OVERRIDE_NUMBER", mensagem: "The Dart language version override comment must be specified with a version number, like '2.0', after the '=' character.", correcao: Some("Specify a Dart language version override with a comment like '// @dart = 2.0'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_language_version_override", unico: "WarningCode.INVALID_LANGUAGE_VERSION_OVERRIDE_PREFIX", mensagem: "The Dart language version override number can't be prefixed with a letter.", correcao: Some("Specify a Dart language version override with a comment like '// @dart = 2.0'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_language_version_override", unico: "WarningCode.INVALID_LANGUAGE_VERSION_OVERRIDE_TRAILING_CHARACTERS", mensagem: "The Dart language version override comment can't be followed by any non-whitespace characters.", correcao: Some("Specify a Dart language version override with a comment like '// @dart = 2.0'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_language_version_override", unico: "WarningCode.INVALID_LANGUAGE_VERSION_OVERRIDE_TWO_SLASHES", mensagem: "The Dart language version override comment must be specified with exactly two slashes.", correcao: Some("Specify a Dart language version override with a comment like '// @dart = 2.0'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_literal_annotation", unico: "WarningCode.INVALID_LITERAL_ANNOTATION", mensagem: "Only const constructors can have the `@literal` annotation.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_non_virtual_annotation", unico: "WarningCode.INVALID_NON_VIRTUAL_ANNOTATION", mensagem: "The annotation '@nonVirtual' can only be applied to a concrete instance member.", correcao: Some("Try removing '@nonVirtual'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_override_of_non_virtual_member", unico: "WarningCode.INVALID_OVERRIDE_OF_NON_VIRTUAL_MEMBER", mensagem: "The member '{0}' is declared non-virtual in '{1}' and can't be overridden in subclasses.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_reopen_annotation", unico: "WarningCode.INVALID_REOPEN_ANNOTATION", mensagem: "The annotation '@reopen' can only be applied to a class that opens capabilities that the supertype intentionally disallows.", correcao: Some("Try removing the '@reopen' annotation."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "invalid_required_named_param", unico: "WarningCode.INVALID_REQUIRED_NAMED_PARAM", mensagem: "The type parameter '{0}' is annotated with @required but only named parameters without a default value can be annotated with it.", correcao: Some("Remove @required."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "invalid_required_optional_positional_param", unico: "WarningCode.INVALID_REQUIRED_OPTIONAL_POSITIONAL_PARAM", mensagem: "Incorrect use of the annotation @required on the optional positional parameter '{0}'. Optional positional parameters cannot be required.", correcao: Some("Remove @required."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "invalid_required_positional_param", unico: "WarningCode.INVALID_REQUIRED_POSITIONAL_PARAM", mensagem: "Redundant use of the annotation @required on the required positional parameter '{0}'.", correcao: Some("Remove @required."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "invalid_use_of_internal_member", unico: "WarningCode.INVALID_USE_OF_INTERNAL_MEMBER", mensagem: "The member '{0}' can only be used within its package.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_use_of_protected_member", unico: "WarningCode.INVALID_USE_OF_PROTECTED_MEMBER", mensagem: "The member '{0}' can only be used within instance members of subclasses of '{1}'.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "invalid_use_of_visible_for_overriding_member", unico: "WarningCode.INVALID_USE_OF_VISIBLE_FOR_OVERRIDING_MEMBER", mensagem: "The member '{0}' can only be used for overriding.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_use_of_visible_for_template_member", unico: "WarningCode.INVALID_USE_OF_VISIBLE_FOR_TEMPLATE_MEMBER", mensagem: "The member '{0}' can only be used within '{1}' or a template library.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "invalid_use_of_visible_for_testing_member", unico: "WarningCode.INVALID_USE_OF_VISIBLE_FOR_TESTING_MEMBER", mensagem: "The member '{0}' can only be used within '{1}' or a test.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_visibility_annotation", unico: "WarningCode.INVALID_VISIBILITY_ANNOTATION", mensagem: "The member '{0}' is annotated with '{1}', but this annotation is only meaningful on declarations of public members.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_visible_for_overriding_annotation", unico: "WarningCode.INVALID_VISIBLE_FOR_OVERRIDING_ANNOTATION", mensagem: "The annotation 'visibleForOverriding' can only be applied to a public instance member that can be overridden.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_visible_outside_template_annotation", unico: "WarningCode.INVALID_VISIBLE_OUTSIDE_TEMPLATE_ANNOTATION", mensagem: "The annotation 'visibleOutsideTemplate' can only be applied to a member of a class, enum, or mixin that is annotated with 'visibleForTemplate'.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "macro_warning", unico: "WarningCode.MACRO_WARNING", mensagem: "{0}", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "missing_override_of_must_be_overridden", unico: "WarningCode.MISSING_OVERRIDE_OF_MUST_BE_OVERRIDDEN_ONE", mensagem: "Missing concrete implementation of '{0}'.", correcao: Some("Try overriding the missing member."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "missing_override_of_must_be_overridden", unico: "WarningCode.MISSING_OVERRIDE_OF_MUST_BE_OVERRIDDEN_THREE_PLUS", mensagem: "Missing concrete implementations of '{0}', '{1}', and {2} more.", correcao: Some("Try overriding the missing members."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "missing_override_of_must_be_overridden", unico: "WarningCode.MISSING_OVERRIDE_OF_MUST_BE_OVERRIDDEN_TWO", mensagem: "Missing concrete implementations of '{0}' and '{1}'.", correcao: Some("Try overriding the missing members."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "missing_required_param", unico: "WarningCode.MISSING_REQUIRED_PARAM", mensagem: "The parameter '{0}' is required.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "missing_required_param", unico: "WarningCode.MISSING_REQUIRED_PARAM_WITH_DETAILS", mensagem: "The parameter '{0}' is required. {1}.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "mixin_on_sealed_class", unico: "WarningCode.MIXIN_ON_SEALED_CLASS", mensagem: "The class '{0}' shouldn't be used as a mixin constraint because it is sealed, and any class mixing in this mixin must have '{0}' as a superclass.", correcao: Some("Try composing with this class, or refer to its documentation for more information."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "must_be_immutable", unico: "WarningCode.MUST_BE_IMMUTABLE", mensagem: "This class (or a class that this class inherits from) is marked as '@immutable', but one or more of its instance fields aren't final: {0}", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "must_call_super", unico: "WarningCode.MUST_CALL_SUPER", mensagem: "This method overrides a method annotated as '@mustCallSuper' in '{0}', but doesn't invoke the overridden method.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "non_const_argument_for_const_parameter", unico: "WarningCode.NON_CONST_ARGUMENT_FOR_CONST_PARAMETER", mensagem: "Argument '{0}' must be a constant.", correcao: Some("Try replacing the argument with a constant."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "non_const_call_to_literal_constructor", unico: "WarningCode.NON_CONST_CALL_TO_LITERAL_CONSTRUCTOR", mensagem: "This instance creation must be 'const', because the {0} constructor is marked as '@literal'.", correcao: Some("Try adding a 'const' keyword."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "non_const_call_to_literal_constructor", unico: "WarningCode.NON_CONST_CALL_TO_LITERAL_CONSTRUCTOR_USING_NEW", mensagem: "This instance creation must be 'const', because the {0} constructor is marked as '@literal'.", correcao: Some("Try replacing the 'new' keyword with 'const'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "non_nullable_equals_parameter", unico: "WarningCode.NON_NULLABLE_EQUALS_PARAMETER", mensagem: "The parameter type of '==' operators should be non-nullable.", correcao: Some("Try using a non-nullable type."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "nullable_type_in_catch_clause", unico: "WarningCode.NULLABLE_TYPE_IN_CATCH_CLAUSE", mensagem: "A potentially nullable type can't be used in an 'on' clause because it isn't valid to throw a nullable expression.", correcao: Some("Try using a non-nullable type."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "null_argument_to_non_null_type", unico: "WarningCode.NULL_ARGUMENT_TO_NON_NULL_TYPE", mensagem: "'{0}' shouldn't be called with a 'null' argument for the non-nullable type argument '{1}'.", correcao: Some("Try adding a non-null argument."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "null_check_always_fails", unico: "WarningCode.NULL_CHECK_ALWAYS_FAILS", mensagem: "This null-check will always throw an exception because the expression will always evaluate to 'null'.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "override_on_non_overriding_member", unico: "WarningCode.OVERRIDE_ON_NON_OVERRIDING_FIELD", mensagem: "The field doesn't override an inherited getter or setter.", correcao: Some("Try updating this class to match the superclass, or removing the override annotation."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "override_on_non_overriding_member", unico: "WarningCode.OVERRIDE_ON_NON_OVERRIDING_GETTER", mensagem: "The getter doesn't override an inherited getter.", correcao: Some("Try updating this class to match the superclass, or removing the override annotation."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "override_on_non_overriding_member", unico: "WarningCode.OVERRIDE_ON_NON_OVERRIDING_METHOD", mensagem: "The method doesn't override an inherited method.", correcao: Some("Try updating this class to match the superclass, or removing the override annotation."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "override_on_non_overriding_member", unico: "WarningCode.OVERRIDE_ON_NON_OVERRIDING_SETTER", mensagem: "The setter doesn't override an inherited setter.", correcao: Some("Try updating this class to match the superclass, or removing the override annotation."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "pattern_never_matches_value_type", unico: "WarningCode.PATTERN_NEVER_MATCHES_VALUE_TYPE", mensagem: "The matched value type '{0}' can never match the required type '{1}'.", correcao: Some("Try using a different pattern."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "receiver_of_type_never", unico: "WarningCode.RECEIVER_OF_TYPE_NEVER", mensagem: "The receiver is of type 'Never', and will never complete with a value.", correcao: Some("Try checking for throw expressions or type errors in the receiver"), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "redeclare_on_non_redeclaring_member", unico: "WarningCode.REDECLARE_ON_NON_REDECLARING_MEMBER", mensagem: "The {0} doesn't redeclare a {0} declared in a superinterface.", correcao: Some("Try updating this member to match a declaration in a superinterface, or removing the redeclare annotation."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "removed_lint_use", unico: "WarningCode.REMOVED_LINT_USE", mensagem: "'{0}' was removed in Dart '{1}'", correcao: Some("Remove the reference to '{0}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "replaced_lint_use", unico: "WarningCode.REPLACED_LINT_USE", mensagem: "'{0}' was replaced by '{2}' in Dart '{1}'.", correcao: Some("Replace '{0}' with '{1}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "return_of_do_not_store", unico: "WarningCode.RETURN_OF_DO_NOT_STORE", mensagem: "'{0}' is annotated with 'doNotStore' and shouldn't be returned unless '{1}' is also annotated.", correcao: Some("Annotate '{1}' with 'doNotStore'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_return_type_for_catch_error", unico: "WarningCode.RETURN_OF_INVALID_TYPE_FROM_CATCH_ERROR", mensagem: "A value of type '{0}' can't be returned by the 'onError' handler because it must be assignable to '{1}'.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_return_type_for_catch_error", unico: "WarningCode.RETURN_TYPE_INVALID_FOR_CATCH_ERROR", mensagem: "The return type '{0}' isn't assignable to '{1}', as required by 'Future.catchError'.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "sdk_version_constructor_tearoffs", unico: "WarningCode.SDK_VERSION_CONSTRUCTOR_TEAROFFS", mensagem: "Tearing off a constructor requires the 'constructor-tearoffs' language feature.", correcao: Some("Try updating your pubspec.yaml to set the minimum SDK constraint to 2.15 or higher, and running 'pub get'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "sdk_version_gt_gt_gt_operator", unico: "WarningCode.SDK_VERSION_GT_GT_GT_OPERATOR", mensagem: "The operator '>>>' wasn't supported until version 2.14.0, but this code is required to be able to run on earlier versions.", correcao: Some("Try updating the SDK constraints."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "sdk_version_since", unico: "WarningCode.SDK_VERSION_SINCE", mensagem: "This API is available since SDK {0}, but constraints '{1}' don't guarantee it.", correcao: Some("Try updating the SDK constraints."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "strict_raw_type", unico: "WarningCode.STRICT_RAW_TYPE", mensagem: "The generic type '{0}' should have explicit type arguments but doesn't.", correcao: Some("Use explicit type arguments for '{0}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "subtype_of_sealed_class", unico: "WarningCode.SUBTYPE_OF_SEALED_CLASS", mensagem: "The class '{0}' shouldn't be extended, mixed in, or implemented because it's sealed.", correcao: Some("Try composing instead of inheriting, or refer to the documentation of '{0}' for more information."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "text_direction_code_point_in_comment", unico: "WarningCode.TEXT_DIRECTION_CODE_POINT_IN_COMMENT", mensagem: "The Unicode code point 'U+{0}' changes the appearance of text from how it's interpreted by the compiler.", correcao: Some("Try removing the code point or using the Unicode escape sequence '\\u{0}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "text_direction_code_point_in_literal", unico: "WarningCode.TEXT_DIRECTION_CODE_POINT_IN_LITERAL", mensagem: "The Unicode code point 'U+{0}' changes the appearance of text from how it's interpreted by the compiler.", correcao: Some("Try removing the code point or using the Unicode escape sequence '\\u{0}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "type_check_with_null", unico: "WarningCode.TYPE_CHECK_IS_NOT_NULL", mensagem: "Tests for non-null should be done with '!= null'.", correcao: Some("Try replacing the 'is! Null' check with '!= null'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "type_check_with_null", unico: "WarningCode.TYPE_CHECK_IS_NULL", mensagem: "Tests for null should be done with '== null'.", correcao: Some("Try replacing the 'is Null' check with '== null'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "undefined_hidden_name", unico: "WarningCode.UNDEFINED_HIDDEN_NAME", mensagem: "The library '{0}' doesn't export a member with the hidden name '{1}'.", correcao: Some("Try removing the name from the list of hidden members."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "undefined_referenced_parameter", unico: "WarningCode.UNDEFINED_REFERENCED_PARAMETER", mensagem: "The parameter '{0}' isn't defined by '{1}'.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "undefined_shown_name", unico: "WarningCode.UNDEFINED_SHOWN_NAME", mensagem: "The library '{0}' doesn't export a member with the shown name '{1}'.", correcao: Some("Try removing the name from the list of shown members."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unignorable_ignore", unico: "WarningCode.UNIGNORABLE_IGNORE", mensagem: "The diagnostic '{0}' can't be ignored.", correcao: Some("Try removing the name from the list, or removing the whole comment if this is the only name in the list."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "unnecessary_cast", unico: "WarningCode.UNNECESSARY_CAST", mensagem: "Unnecessary cast.", correcao: Some("Try removing the cast."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unnecessary_cast_pattern", unico: "WarningCode.UNNECESSARY_CAST_PATTERN", mensagem: "Unnecessary cast pattern.", correcao: Some("Try removing the cast pattern."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "unnecessary_final", unico: "WarningCode.UNNECESSARY_FINAL", mensagem: "The keyword 'final' isn't necessary because the parameter is implicitly 'final'.", correcao: Some("Try removing the 'final'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unnecessary_ignore", unico: "WarningCode.UNNECESSARY_IGNORE", mensagem: "The diagnostic '{0}' isn't produced at this location so it doesn't need to be ignored.", correcao: Some("Try removing the name from the list, or removing the whole comment if this is the only name in the list."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "unnecessary_nan_comparison", unico: "WarningCode.UNNECESSARY_NAN_COMPARISON_FALSE", mensagem: "A double can't equal 'double.nan', so the condition is always 'false'.", correcao: Some("Try using 'double.isNan', or removing the condition."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unnecessary_nan_comparison", unico: "WarningCode.UNNECESSARY_NAN_COMPARISON_TRUE", mensagem: "A double can't equal 'double.nan', so the condition is always 'true'.", correcao: Some("Try using 'double.isNan', or removing the condition."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unnecessary_no_such_method", unico: "WarningCode.UNNECESSARY_NO_SUCH_METHOD", mensagem: "Unnecessary 'noSuchMethod' declaration.", correcao: Some("Try removing the declaration of 'noSuchMethod'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unnecessary_null_comparison", unico: "WarningCode.UNNECESSARY_NULL_COMPARISON_ALWAYS_NULL_FALSE", mensagem: "The operand must be 'null', so the condition is always 'false'.", correcao: Some("Remove the condition."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unnecessary_null_comparison", unico: "WarningCode.UNNECESSARY_NULL_COMPARISON_ALWAYS_NULL_TRUE", mensagem: "The operand must be 'null', so the condition is always 'true'.", correcao: Some("Remove the condition."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unnecessary_null_comparison", unico: "WarningCode.UNNECESSARY_NULL_COMPARISON_NEVER_NULL_FALSE", mensagem: "The operand can't be 'null', so the condition is always 'false'.", correcao: Some("Try removing the condition, an enclosing condition, or the whole conditional statement."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unnecessary_null_comparison", unico: "WarningCode.UNNECESSARY_NULL_COMPARISON_NEVER_NULL_TRUE", mensagem: "The operand can't be 'null', so the condition is always 'true'.", correcao: Some("Remove the condition."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unnecessary_question_mark", unico: "WarningCode.UNNECESSARY_QUESTION_MARK", mensagem: "The '?' is unnecessary because '{0}' is nullable without it.", correcao: None, tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unnecessary_set_literal", unico: "WarningCode.UNNECESSARY_SET_LITERAL", mensagem: "Braces unnecessarily wrap this expression in a set literal.", correcao: Some("Try removing the set literal around the expression."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unnecessary_type_check", unico: "WarningCode.UNNECESSARY_TYPE_CHECK_FALSE", mensagem: "Unnecessary type check; the result is always 'false'.", correcao: Some("Try correcting the type check, or removing the type check."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unnecessary_type_check", unico: "WarningCode.UNNECESSARY_TYPE_CHECK_TRUE", mensagem: "Unnecessary type check; the result is always 'true'.", correcao: Some("Try correcting the type check, or removing the type check."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unnecessary_wildcard_pattern", unico: "WarningCode.UNNECESSARY_WILDCARD_PATTERN", mensagem: "Unnecessary wildcard pattern.", correcao: Some("Try removing the wildcard pattern."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: false },
    InfoCodigo { nome: "unreachable_switch_case", unico: "WarningCode.UNREACHABLE_SWITCH_CASE", mensagem: "This case is covered by the previous cases.", correcao: Some("Try removing the case clause, or restructuring the preceding patterns."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unreachable_switch_default", unico: "WarningCode.UNREACHABLE_SWITCH_DEFAULT", mensagem: "This default clause is covered by the previous cases.", correcao: Some("Try removing the default clause, or restructuring the preceding patterns."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unused_catch_clause", unico: "WarningCode.UNUSED_CATCH_CLAUSE", mensagem: "The exception variable '{0}' isn't used, so the 'catch' clause can be removed.", correcao: Some("Try removing the catch clause."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unused_catch_stack", unico: "WarningCode.UNUSED_CATCH_STACK", mensagem: "The stack trace variable '{0}' isn't used and can be removed.", correcao: Some("Try removing the stack trace variable, or using it."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unused_element", unico: "WarningCode.UNUSED_ELEMENT", mensagem: "The declaration '{0}' isn't referenced.", correcao: Some("Try removing the declaration of '{0}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unused_element", unico: "WarningCode.UNUSED_ELEMENT_PARAMETER", mensagem: "A value for optional parameter '{0}' isn't ever given.", correcao: Some("Try removing the unused parameter."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unused_field", unico: "WarningCode.UNUSED_FIELD", mensagem: "The value of the field '{0}' isn't used.", correcao: Some("Try removing the field, or using it."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unused_import", unico: "WarningCode.UNUSED_IMPORT", mensagem: "Unused import: '{0}'.", correcao: Some("Try removing the import directive."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unused_label", unico: "WarningCode.UNUSED_LABEL", mensagem: "The label '{0}' isn't used.", correcao: Some("Try removing the label, or using it in either a 'break' or 'continue' statement."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unused_local_variable", unico: "WarningCode.UNUSED_LOCAL_VARIABLE", mensagem: "The value of the local variable '{0}' isn't used.", correcao: Some("Try removing the variable or using it."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unused_result", unico: "WarningCode.UNUSED_RESULT", mensagem: "The value of '{0}' should be used.", correcao: Some("Try using the result by invoking a member, passing it to a function, or returning it from this function."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unused_result", unico: "WarningCode.UNUSED_RESULT_WITH_MESSAGE", mensagem: "'{0}' should be used. {1}.", correcao: Some("Try using the result by invoking a member, passing it to a function, or returning it from this function."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "unused_shown_name", unico: "WarningCode.UNUSED_SHOWN_NAME", mensagem: "The name {0} is shown, but isn't used.", correcao: Some("Try removing the name from the list of shown members."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "uri_does_not_exist_in_doc_import", unico: "WarningCode.URI_DOES_NOT_EXIST_IN_DOC_IMPORT", mensagem: "Target of URI doesn't exist: '{0}'.", correcao: Some("Try creating the file referenced by the URI, or try using a URI for a file that does exist."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "invalid_use_of_do_not_submit_member", unico: "WarningCode.invalid_use_of_do_not_submit_member", mensagem: "Uses of '{0}' should not be submitted to source control.", correcao: Some("Try removing the reference to '{0}'."), tipo: TipoErro::StaticWarning, severidade: Severidade::Warning, documentado: true },
    InfoCodigo { nome: "deprecated_colon_for_default_value", unico: "HintCode.DEPRECATED_COLON_FOR_DEFAULT_VALUE", mensagem: "Using a colon as the separator before a default value is deprecated and will not be supported in language version 3.0 and later.", correcao: Some("Try replacing the colon with an equal sign."), tipo: TipoErro::Hint, severidade: Severidade::Info, documentado: true },
    InfoCodigo { nome: "deprecated_member_use", unico: "HintCode.DEPRECATED_MEMBER_USE", mensagem: "'{0}' is deprecated and shouldn't be used.", correcao: Some("Try replacing the use of the deprecated member with the replacement."), tipo: TipoErro::Hint, severidade: Severidade::Info, documentado: true },
    InfoCodigo { nome: "deprecated_member_use_from_same_package", unico: "HintCode.DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE", mensagem: "'{0}' is deprecated and shouldn't be used.", correcao: Some("Try replacing the use of the deprecated member with the replacement."), tipo: TipoErro::Hint, severidade: Severidade::Info, documentado: true },
    InfoCodigo { nome: "deprecated_member_use_from_same_package", unico: "HintCode.DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE_WITH_MESSAGE", mensagem: "'{0}' is deprecated and shouldn't be used. {1}", correcao: Some("Try replacing the use of the deprecated member with the replacement."), tipo: TipoErro::Hint, severidade: Severidade::Info, documentado: true },
    InfoCodigo { nome: "deprecated_member_use", unico: "HintCode.DEPRECATED_MEMBER_USE_WITH_MESSAGE", mensagem: "'{0}' is deprecated and shouldn't be used. {1}", correcao: Some("Try replacing the use of the deprecated member with the replacement."), tipo: TipoErro::Hint, severidade: Severidade::Info, documentado: true },
    InfoCodigo { nome: "import_deferred_library_with_load_function", unico: "HintCode.IMPORT_DEFERRED_LIBRARY_WITH_LOAD_FUNCTION", mensagem: "The imported library defines a top-level function named 'loadLibrary' that is hidden by deferring this library.", correcao: Some("Try changing the import to not be deferred, or rename the function in the imported library."), tipo: TipoErro::Hint, severidade: Severidade::Info, documentado: true },
    InfoCodigo { nome: "macro_info", unico: "HintCode.MACRO_INFO", mensagem: "{0}", correcao: None, tipo: TipoErro::Hint, severidade: Severidade::Info, documentado: false },
    InfoCodigo { nome: "unnecessary_import", unico: "HintCode.UNNECESSARY_IMPORT", mensagem: "The import of '{0}' is unnecessary because all of the used elements are also provided by the import of '{1}'.", correcao: Some("Try removing the import directive."), tipo: TipoErro::Hint, severidade: Severidade::Info, documentado: true },
    InfoCodigo { nome: "abi_specific_integer_invalid", unico: "FfiCode.ABI_SPECIFIC_INTEGER_INVALID", mensagem: "Classes extending 'AbiSpecificInteger' must have exactly one const constructor, no other members, and no type parameters.", correcao: Some("Try removing all type parameters, removing all members, and adding one const constructor."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "abi_specific_integer_mapping_extra", unico: "FfiCode.ABI_SPECIFIC_INTEGER_MAPPING_EXTRA", mensagem: "Classes extending 'AbiSpecificInteger' must have exactly one 'AbiSpecificIntegerMapping' annotation specifying the mapping from ABI to a 'NativeType' integer with a fixed size.", correcao: Some("Try removing the extra annotation."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "abi_specific_integer_mapping_missing", unico: "FfiCode.ABI_SPECIFIC_INTEGER_MAPPING_MISSING", mensagem: "Classes extending 'AbiSpecificInteger' must have exactly one 'AbiSpecificIntegerMapping' annotation specifying the mapping from ABI to a 'NativeType' integer with a fixed size.", correcao: Some("Try adding an annotation."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "abi_specific_integer_mapping_unsupported", unico: "FfiCode.ABI_SPECIFIC_INTEGER_MAPPING_UNSUPPORTED", mensagem: "Invalid mapping to '{0}'; only mappings to 'Int8', 'Int16', 'Int32', 'Int64', 'Uint8', 'Uint16', 'UInt32', and 'Uint64' are supported.", correcao: Some("Try changing the value to 'Int8', 'Int16', 'Int32', 'Int64', 'Uint8', 'Uint16', 'UInt32', or 'Uint64'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "address_position", unico: "FfiCode.ADDRESS_POSITION", mensagem: "The '.address' expression can only be used as argument to a leaf native external call.", correcao: None, tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "address_receiver", unico: "FfiCode.ADDRESS_RECEIVER", mensagem: "The receiver of '.address' must be a concrete 'TypedData', a concrete 'TypedData' '[]', an 'Array', an 'Array' '[]', a Struct field, or a Union field.", correcao: Some("Change the receiver of '.address' to one of the allowed kinds."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "annotation_on_pointer_field", unico: "FfiCode.ANNOTATION_ON_POINTER_FIELD", mensagem: "Fields in a struct class whose type is 'Pointer' shouldn't have any annotations.", correcao: Some("Try removing the annotation."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "argument_must_be_a_constant", unico: "FfiCode.ARGUMENT_MUST_BE_A_CONSTANT", mensagem: "Argument '{0}' must be a constant.", correcao: Some("Try replacing the value with a literal or const."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "argument_must_be_native", unico: "FfiCode.ARGUMENT_MUST_BE_NATIVE", mensagem: "Argument to 'Native.addressOf' must be annotated with @Native", correcao: Some("Try passing a static function or field annotated with '@Native'"), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "compound_implements_finalizable", unico: "FfiCode.COMPOUND_IMPLEMENTS_FINALIZABLE", mensagem: "The class '{0}' can't implement Finalizable.", correcao: Some("Try removing the implements clause from '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "creation_of_struct_or_union", unico: "FfiCode.CREATION_OF_STRUCT_OR_UNION", mensagem: "Subclasses of 'Struct' and 'Union' are backed by native memory, and can't be instantiated by a generative constructor.", correcao: Some("Try allocating it via allocation, or load from a 'Pointer'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "empty_struct", unico: "FfiCode.EMPTY_STRUCT", mensagem: "The class '{0}' can't be empty because it's a subclass of '{1}'.", correcao: Some("Try adding a field to '{0}' or use a different superclass."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extra_annotation_on_struct_field", unico: "FfiCode.EXTRA_ANNOTATION_ON_STRUCT_FIELD", mensagem: "Fields in a struct class must have exactly one annotation indicating the native type.", correcao: Some("Try removing the extra annotation."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extra_size_annotation_carray", unico: "FfiCode.EXTRA_SIZE_ANNOTATION_CARRAY", mensagem: "'Array's must have exactly one 'Array' annotation.", correcao: Some("Try removing the extra annotation."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "ffi_native_invalid_duplicate_default_asset", unico: "FfiCode.FFI_NATIVE_INVALID_DUPLICATE_DEFAULT_ASSET", mensagem: "There may be at most one @DefaultAsset annotation on a library.", correcao: Some("Try removing the extra annotation."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "ffi_native_invalid_multiple_annotations", unico: "FfiCode.FFI_NATIVE_INVALID_MULTIPLE_ANNOTATIONS", mensagem: "Native functions and fields must have exactly one `@Native` annotation.", correcao: Some("Try removing the extra annotation."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "ffi_native_must_be_external", unico: "FfiCode.FFI_NATIVE_MUST_BE_EXTERNAL", mensagem: "Native functions must be declared external.", correcao: Some("Add the `external` keyword to the function."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "ffi_native_only_classes_extending_nativefieldwrapperclass1_can_be_pointer", unico: "FfiCode.FFI_NATIVE_ONLY_CLASSES_EXTENDING_NATIVEFIELDWRAPPERCLASS1_CAN_BE_POINTER", mensagem: "Only classes extending NativeFieldWrapperClass1 can be passed as Pointer.", correcao: Some("Pass as Handle instead."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "ffi_native_unexpected_number_of_parameters", unico: "FfiCode.FFI_NATIVE_UNEXPECTED_NUMBER_OF_PARAMETERS", mensagem: "Unexpected number of Native annotation parameters. Expected {0} but has {1}.", correcao: Some("Make sure parameters match the function annotated."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "ffi_native_unexpected_number_of_parameters_with_receiver", unico: "FfiCode.FFI_NATIVE_UNEXPECTED_NUMBER_OF_PARAMETERS_WITH_RECEIVER", mensagem: "Unexpected number of Native annotation parameters. Expected {0} but has {1}. Native instance method annotation must have receiver as first argument.", correcao: Some("Make sure parameters match the function annotated, including an extra first parameter for the receiver."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "field_must_be_external_in_struct", unico: "FfiCode.FIELD_MUST_BE_EXTERNAL_IN_STRUCT", mensagem: "Fields of 'Struct' and 'Union' subclasses must be marked external.", correcao: Some("Try adding the 'external' modifier."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "generic_struct_subclass", unico: "FfiCode.GENERIC_STRUCT_SUBCLASS", mensagem: "The class '{0}' can't extend 'Struct' or 'Union' because '{0}' is generic.", correcao: Some("Try removing the type parameters from '{0}'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_exception_value", unico: "FfiCode.INVALID_EXCEPTION_VALUE", mensagem: "The method {0} can't have an exceptional return value (the second argument) when the return type of the function is either 'void', 'Handle' or 'Pointer'.", correcao: Some("Try removing the exceptional return value."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_field_type_in_struct", unico: "FfiCode.INVALID_FIELD_TYPE_IN_STRUCT", mensagem: "Fields in struct classes can't have the type '{0}'. They can only be declared as 'int', 'double', 'Array', 'Pointer', or subtype of 'Struct' or 'Union'.", correcao: Some("Try using 'int', 'double', 'Array', 'Pointer', or subtype of 'Struct' or 'Union'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "leaf_call_must_not_return_handle", unico: "FfiCode.LEAF_CALL_MUST_NOT_RETURN_HANDLE", mensagem: "FFI leaf call can't return a 'Handle'.", correcao: Some("Try changing the return type to primitive or struct."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "leaf_call_must_not_take_handle", unico: "FfiCode.LEAF_CALL_MUST_NOT_TAKE_HANDLE", mensagem: "FFI leaf call can't take arguments of type 'Handle'.", correcao: Some("Try changing the argument type to primitive or struct."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "mismatched_annotation_on_struct_field", unico: "FfiCode.MISMATCHED_ANNOTATION_ON_STRUCT_FIELD", mensagem: "The annotation doesn't match the declared type of the field.", correcao: Some("Try using a different annotation or changing the declared type to match."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "missing_annotation_on_struct_field", unico: "FfiCode.MISSING_ANNOTATION_ON_STRUCT_FIELD", mensagem: "Fields of type '{0}' in a subclass of '{1}' must have an annotation indicating the native type.", correcao: Some("Try adding an annotation."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "missing_exception_value", unico: "FfiCode.MISSING_EXCEPTION_VALUE", mensagem: "The method {0} must have an exceptional return value (the second argument) when the return type of the function is neither 'void', 'Handle', nor 'Pointer'.", correcao: Some("Try adding an exceptional return value."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "missing_field_type_in_struct", unico: "FfiCode.MISSING_FIELD_TYPE_IN_STRUCT", mensagem: "Fields in struct classes must have an explicitly declared type of 'int', 'double' or 'Pointer'.", correcao: Some("Try using 'int', 'double' or 'Pointer'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "missing_size_annotation_carray", unico: "FfiCode.MISSING_SIZE_ANNOTATION_CARRAY", mensagem: "Fields of type 'Array' must have exactly one 'Array' annotation.", correcao: Some("Try adding an 'Array' annotation, or removing all but one of the annotations."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "must_be_a_native_function_type", unico: "FfiCode.MUST_BE_A_NATIVE_FUNCTION_TYPE", mensagem: "The type '{0}' given to '{1}' must be a valid 'dart:ffi' native function type.", correcao: Some("Try changing the type to only use members for 'dart:ffi'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "must_be_a_subtype", unico: "FfiCode.MUST_BE_A_SUBTYPE", mensagem: "The type '{0}' must be a subtype of '{1}' for '{2}'.", correcao: Some("Try changing one or both of the type arguments."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "must_return_void", unico: "FfiCode.MUST_RETURN_VOID", mensagem: "The return type of the function passed to 'NativeCallable.listener' must be 'void' rather than '{0}'.", correcao: Some("Try changing the return type to 'void'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "native_field_invalid_type", unico: "FfiCode.NATIVE_FIELD_INVALID_TYPE", mensagem: "'{0}' is an unsupported type for native fields. Native fields only support pointers, arrays or numeric and compound types.", correcao: Some("Try changing the type in the `@Native` annotation to a numeric FFI type, a pointer, array, or a compound class."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "native_field_missing_type", unico: "FfiCode.NATIVE_FIELD_MISSING_TYPE", mensagem: "The native type of this field could not be inferred and must be specified in the annotation.", correcao: Some("Try adding a type parameter extending `NativeType` to the `@Native` annotation."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "native_field_not_static", unico: "FfiCode.NATIVE_FIELD_NOT_STATIC", mensagem: "Native fields must be static.", correcao: Some("Try adding the modifier 'static' to this field."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_constant_type_argument", unico: "FfiCode.NON_CONSTANT_TYPE_ARGUMENT", mensagem: "The type arguments to '{0}' must be known at compile time, so they can't be type parameters.", correcao: Some("Try changing the type argument to be a constant type."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_native_function_type_argument_to_pointer", unico: "FfiCode.NON_NATIVE_FUNCTION_TYPE_ARGUMENT_TO_POINTER", mensagem: "Can't invoke 'asFunction' because the function signature '{0}' for the pointer isn't a valid C function signature.", correcao: Some("Try changing the function argument in 'NativeFunction' to only use NativeTypes."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_positive_array_dimension", unico: "FfiCode.NON_POSITIVE_ARRAY_DIMENSION", mensagem: "Array dimensions must be positive numbers.", correcao: Some("Try changing the input to a positive number."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "non_sized_type_argument", unico: "FfiCode.NON_SIZED_TYPE_ARGUMENT", mensagem: "The type '{1}' isn't a valid type argument for '{0}'. The type argument must be a native integer, 'Float', 'Double', 'Pointer', or subtype of 'Struct', 'Union', or 'AbiSpecificInteger'.", correcao: Some("Try using a native integer, 'Float', 'Double', 'Pointer', or subtype of 'Struct', 'Union', or 'AbiSpecificInteger'."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "packed_annotation", unico: "FfiCode.PACKED_ANNOTATION", mensagem: "Structs must have at most one 'Packed' annotation.", correcao: Some("Try removing extra 'Packed' annotations."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "packed_annotation_alignment", unico: "FfiCode.PACKED_ANNOTATION_ALIGNMENT", mensagem: "Only packing to 1, 2, 4, 8, and 16 bytes is supported.", correcao: Some("Try changing the 'Packed' annotation alignment to 1, 2, 4, 8, or 16."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "size_annotation_dimensions", unico: "FfiCode.SIZE_ANNOTATION_DIMENSIONS", mensagem: "'Array's must have an 'Array' annotation that matches the dimensions.", correcao: Some("Try adjusting the arguments in the 'Array' annotation."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "subtype_of_struct_class", unico: "FfiCode.SUBTYPE_OF_STRUCT_CLASS_IN_EXTENDS", mensagem: "The class '{0}' can't extend '{1}' because '{1}' is a subtype of 'Struct', 'Union', or 'AbiSpecificInteger'.", correcao: Some("Try extending 'Struct', 'Union', or 'AbiSpecificInteger' directly."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "subtype_of_struct_class", unico: "FfiCode.SUBTYPE_OF_STRUCT_CLASS_IN_IMPLEMENTS", mensagem: "The class '{0}' can't implement '{1}' because '{1}' is a subtype of 'Struct', 'Union', or 'AbiSpecificInteger'.", correcao: Some("Try extending 'Struct', 'Union', or 'AbiSpecificInteger' directly."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "subtype_of_struct_class", unico: "FfiCode.SUBTYPE_OF_STRUCT_CLASS_IN_WITH", mensagem: "The class '{0}' can't mix in '{1}' because '{1}' is a subtype of 'Struct', 'Union', or 'AbiSpecificInteger'.", correcao: Some("Try extending 'Struct', 'Union', or 'AbiSpecificInteger' directly."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "variable_length_array_not_last", unico: "FfiCode.VARIABLE_LENGTH_ARRAY_NOT_LAST", mensagem: "Variable length 'Array's must only occur as the last field of Structs.", correcao: Some("Try adjusting the arguments in the 'Array' annotation."), tipo: TipoErro::CompileTimeError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "abstract_class_member", unico: "ParserErrorCode.ABSTRACT_CLASS_MEMBER", mensagem: "Members of classes can't be declared to be 'abstract'.", correcao: Some("Try removing the 'abstract' keyword. You can add the 'abstract' keyword before the class declaration."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "abstract_external_field", unico: "ParserErrorCode.ABSTRACT_EXTERNAL_FIELD", mensagem: "Fields can't be declared both 'abstract' and 'external'.", correcao: Some("Try removing the 'abstract' or 'external' keyword."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "abstract_final_base_class", unico: "ParserErrorCode.ABSTRACT_FINAL_BASE_CLASS", mensagem: "An 'abstract' class can't be declared as both 'final' and 'base'.", correcao: Some("Try removing either the 'final' or 'base' keyword."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "abstract_final_interface_class", unico: "ParserErrorCode.ABSTRACT_FINAL_INTERFACE_CLASS", mensagem: "An 'abstract' class can't be declared as both 'final' and 'interface'.", correcao: Some("Try removing either the 'final' or 'interface' keyword."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "abstract_late_field", unico: "ParserErrorCode.ABSTRACT_LATE_FIELD", mensagem: "Abstract fields cannot be late.", correcao: Some("Try removing the 'abstract' or 'late' keyword."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "abstract_sealed_class", unico: "ParserErrorCode.ABSTRACT_SEALED_CLASS", mensagem: "A 'sealed' class can't be marked 'abstract' because it's already implicitly abstract.", correcao: Some("Try removing the 'abstract' keyword."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "abstract_static_field", unico: "ParserErrorCode.ABSTRACT_STATIC_FIELD", mensagem: "Static fields can't be declared 'abstract'.", correcao: Some("Try removing the 'abstract' or 'static' keyword."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "abstract_static_method", unico: "ParserErrorCode.ABSTRACT_STATIC_METHOD", mensagem: "Static methods can't be declared to be 'abstract'.", correcao: Some("Try removing the keyword 'abstract'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "annotation_on_type_argument", unico: "ParserErrorCode.ANNOTATION_ON_TYPE_ARGUMENT", mensagem: "Type arguments can't have annotations because they aren't declarations.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "annotation_space_before_parenthesis", unico: "ParserErrorCode.ANNOTATION_SPACE_BEFORE_PARENTHESIS", mensagem: "Annotations can't have spaces or comments before the parenthesis.", correcao: Some("Remove any spaces or comments before the parenthesis."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "annotation_with_type_arguments", unico: "ParserErrorCode.ANNOTATION_WITH_TYPE_ARGUMENTS", mensagem: "An annotation can't use type arguments.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "annotation_with_type_arguments_uninstantiated", unico: "ParserErrorCode.ANNOTATION_WITH_TYPE_ARGUMENTS_UNINSTANTIATED", mensagem: "An annotation with type arguments must be followed by an argument list.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "async_keyword_used_as_identifier", unico: "ParserErrorCode.ASYNC_KEYWORD_USED_AS_IDENTIFIER", mensagem: "The keywords 'await' and 'yield' can't be used as identifiers in an asynchronous or generator function.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "base_enum", unico: "ParserErrorCode.BASE_ENUM", mensagem: "Enums can't be declared to be 'base'.", correcao: Some("Try removing the keyword 'base'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "binary_operator_written_out", unico: "ParserErrorCode.BINARY_OPERATOR_WRITTEN_OUT", mensagem: "Binary operator '{0}' is written as '{1}' instead of the written out word.", correcao: Some("Try replacing '{0}' with '{1}'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "break_outside_of_loop", unico: "ParserErrorCode.BREAK_OUTSIDE_OF_LOOP", mensagem: "A break statement can't be used outside of a loop or switch statement.", correcao: Some("Try removing the break statement."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "catch_syntax", unico: "ParserErrorCode.CATCH_SYNTAX", mensagem: "'catch' must be followed by '(identifier)' or '(identifier, identifier)'.", correcao: Some("No types are needed, the first is given by 'on', the second is always 'StackTrace'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "catch_syntax_extra_parameters", unico: "ParserErrorCode.CATCH_SYNTAX_EXTRA_PARAMETERS", mensagem: "'catch' must be followed by '(identifier)' or '(identifier, identifier)'.", correcao: Some("No types are needed, the first is given by 'on', the second is always 'StackTrace'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "class_in_class", unico: "ParserErrorCode.CLASS_IN_CLASS", mensagem: "Classes can't be declared inside other classes.", correcao: Some("Try moving the class to the top-level."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "colon_in_place_of_in", unico: "ParserErrorCode.COLON_IN_PLACE_OF_IN", mensagem: "For-in loops use 'in' rather than a colon.", correcao: Some("Try replacing the colon with the keyword 'in'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "conflicting_modifiers", unico: "ParserErrorCode.CONFLICTING_MODIFIERS", mensagem: "Members can't be declared to be both '{0}' and '{1}'.", correcao: Some("Try removing one of the keywords."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "constructor_with_return_type", unico: "ParserErrorCode.CONSTRUCTOR_WITH_RETURN_TYPE", mensagem: "Constructors can't have a return type.", correcao: Some("Try removing the return type."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "constructor_with_type_arguments", unico: "ParserErrorCode.CONSTRUCTOR_WITH_TYPE_ARGUMENTS", mensagem: "A constructor invocation can't have type arguments after the constructor name.", correcao: Some("Try removing the type arguments or placing them after the class name."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_and_final", unico: "ParserErrorCode.CONST_AND_FINAL", mensagem: "Members can't be declared to be both 'const' and 'final'.", correcao: Some("Try removing either the 'const' or 'final' keyword."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_class", unico: "ParserErrorCode.CONST_CLASS", mensagem: "Classes can't be declared to be 'const'.", correcao: Some("Try removing the 'const' keyword. If you're trying to indicate that instances of the class can be constants, place the 'const' keyword on  the class' constructor(s)."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_constructor_with_body", unico: "ParserErrorCode.CONST_CONSTRUCTOR_WITH_BODY", mensagem: "Const constructors can't have a body.", correcao: Some("Try removing either the 'const' keyword or the body."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_factory", unico: "ParserErrorCode.CONST_FACTORY", mensagem: "Only redirecting factory constructors can be declared to be 'const'.", correcao: Some("Try removing the 'const' keyword, or replacing the body with '=' followed by a valid target."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "const_method", unico: "ParserErrorCode.CONST_METHOD", mensagem: "Getters, setters and methods can't be declared to be 'const'.", correcao: Some("Try removing the 'const' keyword."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "continue_outside_of_loop", unico: "ParserErrorCode.CONTINUE_OUTSIDE_OF_LOOP", mensagem: "A continue statement can't be used outside of a loop or switch statement.", correcao: Some("Try removing the continue statement."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "continue_without_label_in_case", unico: "ParserErrorCode.CONTINUE_WITHOUT_LABEL_IN_CASE", mensagem: "A continue statement in a switch statement must have a label as a target.", correcao: Some("Try adding a label associated with one of the case clauses to the continue statement."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "covariant_and_static", unico: "ParserErrorCode.COVARIANT_AND_STATIC", mensagem: "Members can't be declared to be both 'covariant' and 'static'.", correcao: Some("Try removing either the 'covariant' or 'static' keyword."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "covariant_constructor", unico: "ParserErrorCode.COVARIANT_CONSTRUCTOR", mensagem: "A constructor can't be declared to be 'covariant'.", correcao: Some("Try removing the keyword 'covariant'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "covariant_member", unico: "ParserErrorCode.COVARIANT_MEMBER", mensagem: "Getters, setters and methods can't be declared to be 'covariant'.", correcao: Some("Try removing the 'covariant' keyword."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "declaration_named_augmented_inside_augmentation", unico: "ParserErrorCode.DECLARATION_NAMED_AUGMENTED_INSIDE_AUGMENTATION", mensagem: "The identifier 'augmented' has a special meaning inside augmenting declarations.", correcao: Some("Try using a different name."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "default_in_switch_expression", unico: "ParserErrorCode.DEFAULT_IN_SWITCH_EXPRESSION", mensagem: "A switch expression may not use the `default` keyword.", correcao: Some("Try replacing `default` with `_`."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "default_value_in_function_type", unico: "ParserErrorCode.DEFAULT_VALUE_IN_FUNCTION_TYPE", mensagem: "Parameters in a function type can't have default values.", correcao: Some("Try removing the default value."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "deferred_after_prefix", unico: "ParserErrorCode.DEFERRED_AFTER_PREFIX", mensagem: "The deferred keyword should come immediately before the prefix ('as' clause).", correcao: Some("Try moving the deferred keyword before the prefix."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "directive_after_declaration", unico: "ParserErrorCode.DIRECTIVE_AFTER_DECLARATION", mensagem: "Directives must appear before any declarations.", correcao: Some("Try moving the directive before any declarations."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "duplicated_modifier", unico: "ParserErrorCode.DUPLICATED_MODIFIER", mensagem: "The modifier '{0}' was already specified.", correcao: Some("Try removing all but one occurrence of the modifier."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "duplicate_deferred", unico: "ParserErrorCode.DUPLICATE_DEFERRED", mensagem: "An import directive can only have one 'deferred' keyword.", correcao: Some("Try removing all but one 'deferred' keyword."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "duplicate_label_in_switch_statement", unico: "ParserErrorCode.DUPLICATE_LABEL_IN_SWITCH_STATEMENT", mensagem: "The label '{0}' was already used in this switch statement.", correcao: Some("Try choosing a different name for this label."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "duplicate_prefix", unico: "ParserErrorCode.DUPLICATE_PREFIX", mensagem: "An import directive can only have one prefix ('as' clause).", correcao: Some("Try removing all but one prefix."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "empty_enum_body", unico: "ParserErrorCode.EMPTY_ENUM_BODY", mensagem: "An enum must declare at least one constant name.", correcao: Some("Try declaring a constant."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "empty_record_literal_with_comma", unico: "ParserErrorCode.EMPTY_RECORD_LITERAL_WITH_COMMA", mensagem: "A record literal without fields can't have a trailing comma.", correcao: Some("Try removing the trailing comma."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "empty_record_type_named_fields_list", unico: "ParserErrorCode.EMPTY_RECORD_TYPE_NAMED_FIELDS_LIST", mensagem: "The list of named fields in a record type can't be empty.", correcao: Some("Try adding a named field to the list."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "empty_record_type_with_comma", unico: "ParserErrorCode.EMPTY_RECORD_TYPE_WITH_COMMA", mensagem: "A record type without fields can't have a trailing comma.", correcao: Some("Try removing the trailing comma."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "enum_in_class", unico: "ParserErrorCode.ENUM_IN_CLASS", mensagem: "Enums can't be declared inside classes.", correcao: Some("Try moving the enum to the top-level."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "equality_cannot_be_equality_operand", unico: "ParserErrorCode.EQUALITY_CANNOT_BE_EQUALITY_OPERAND", mensagem: "A comparison expression can't be an operand of another comparison expression.", correcao: Some("Try putting parentheses around one of the comparisons."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_case_or_default", unico: "ParserErrorCode.EXPECTED_CASE_OR_DEFAULT", mensagem: "Expected 'case' or 'default'.", correcao: Some("Try placing this code inside a case clause."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_body", unico: "ParserErrorCode.EXPECTED_CATCH_CLAUSE_BODY", mensagem: "A catch clause must have a body, even if it is empty.", correcao: Some("Try adding an empty body."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_body", unico: "ParserErrorCode.EXPECTED_CLASS_BODY", mensagem: "A class declaration must have a body, even if it is empty.", correcao: Some("Try adding an empty body."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_class_member", unico: "ParserErrorCode.EXPECTED_CLASS_MEMBER", mensagem: "Expected a class member.", correcao: Some("Try placing this code inside a class member."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_else_or_comma", unico: "ParserErrorCode.EXPECTED_ELSE_OR_COMMA", mensagem: "Expected 'else' or comma.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_executable", unico: "ParserErrorCode.EXPECTED_EXECUTABLE", mensagem: "Expected a method, getter, setter or operator declaration.", correcao: Some("This appears to be incomplete code. Try removing it or completing it."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_body", unico: "ParserErrorCode.EXPECTED_EXTENSION_BODY", mensagem: "An extension declaration must have a body, even if it is empty.", correcao: Some("Try adding an empty body."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_body", unico: "ParserErrorCode.EXPECTED_EXTENSION_TYPE_BODY", mensagem: "An extension type declaration must have a body, even if it is empty.", correcao: Some("Try adding an empty body."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_body", unico: "ParserErrorCode.EXPECTED_FINALLY_CLAUSE_BODY", mensagem: "A finally clause must have a body, even if it is empty.", correcao: Some("Try adding an empty body."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_identifier_but_got_keyword", unico: "ParserErrorCode.EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD", mensagem: "'{0}' can't be used as an identifier because it's a keyword.", correcao: Some("Try renaming this to be an identifier that isn't a keyword."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_instead", unico: "ParserErrorCode.EXPECTED_INSTEAD", mensagem: "Expected '{0}' instead of this.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_list_or_map_literal", unico: "ParserErrorCode.EXPECTED_LIST_OR_MAP_LITERAL", mensagem: "Expected a list or map literal.", correcao: Some("Try inserting a list or map literal, or remove the type arguments."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_body", unico: "ParserErrorCode.EXPECTED_MIXIN_BODY", mensagem: "A mixin declaration must have a body, even if it is empty.", correcao: Some("Try adding an empty body."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_named_type", unico: "ParserErrorCode.EXPECTED_NAMED_TYPE_EXTENDS", mensagem: "Expected a class name.", correcao: Some("Try using a class name, possibly with type arguments."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_named_type", unico: "ParserErrorCode.EXPECTED_NAMED_TYPE_IMPLEMENTS", mensagem: "Expected the name of a class or mixin.", correcao: Some("Try using a class or mixin name, possibly with type arguments."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_named_type", unico: "ParserErrorCode.EXPECTED_NAMED_TYPE_ON", mensagem: "Expected the name of a class or mixin.", correcao: Some("Try using a class or mixin name, possibly with type arguments."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_named_type", unico: "ParserErrorCode.EXPECTED_NAMED_TYPE_WITH", mensagem: "Expected a mixin name.", correcao: Some("Try using a mixin name, possibly with type arguments."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_representation_field", unico: "ParserErrorCode.EXPECTED_REPRESENTATION_FIELD", mensagem: "Expected a representation field.", correcao: Some("Try providing the representation field for this extension type."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_representation_type", unico: "ParserErrorCode.EXPECTED_REPRESENTATION_TYPE", mensagem: "Expected a representation type.", correcao: Some("Try providing the representation type for this extension type."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_string_literal", unico: "ParserErrorCode.EXPECTED_STRING_LITERAL", mensagem: "Expected a string literal.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_body", unico: "ParserErrorCode.EXPECTED_SWITCH_EXPRESSION_BODY", mensagem: "A switch expression must have a body, even if it is empty.", correcao: Some("Try adding an empty body."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_body", unico: "ParserErrorCode.EXPECTED_SWITCH_STATEMENT_BODY", mensagem: "A switch statement must have a body, even if it is empty.", correcao: Some("Try adding an empty body."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_token", unico: "ParserErrorCode.EXPECTED_TOKEN", mensagem: "Expected to find '{0}'.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_body", unico: "ParserErrorCode.EXPECTED_TRY_STATEMENT_BODY", mensagem: "A try statement must have a body, even if it is empty.", correcao: Some("Try adding an empty body."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_type_name", unico: "ParserErrorCode.EXPECTED_TYPE_NAME", mensagem: "Expected a type name.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "experiment_not_enabled", unico: "ParserErrorCode.EXPERIMENT_NOT_ENABLED", mensagem: "This requires the '{0}' language feature to be enabled.", correcao: Some("Try updating your pubspec.yaml to set the minimum SDK constraint to {1} or higher, and running 'pub get'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "experiment_not_enabled_off_by_default", unico: "ParserErrorCode.EXPERIMENT_NOT_ENABLED_OFF_BY_DEFAULT", mensagem: "This requires the experimental '{0}' language feature to be enabled.", correcao: Some("Try passing the '--enable-experiment={0}' command line option."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "export_directive_after_part_directive", unico: "ParserErrorCode.EXPORT_DIRECTIVE_AFTER_PART_DIRECTIVE", mensagem: "Export directives must precede part directives.", correcao: Some("Try moving the export directives before the part directives."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "extension_augmentation_has_on_clause", unico: "ParserErrorCode.EXTENSION_AUGMENTATION_HAS_ON_CLAUSE", mensagem: "Extension augmentations can't have 'on' clauses.", correcao: Some("Try removing the 'on' clause."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "extension_declares_abstract_member", unico: "ParserErrorCode.EXTENSION_DECLARES_ABSTRACT_MEMBER", mensagem: "Extensions can't declare abstract members.", correcao: Some("Try providing an implementation for the member."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extension_declares_constructor", unico: "ParserErrorCode.EXTENSION_DECLARES_CONSTRUCTOR", mensagem: "Extensions can't declare constructors.", correcao: Some("Try removing the constructor declaration."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extension_declares_instance_field", unico: "ParserErrorCode.EXTENSION_DECLARES_INSTANCE_FIELD", mensagem: "Extensions can't declare instance fields", correcao: Some("Try removing the field declaration or making it a static field"), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "extension_type_extends", unico: "ParserErrorCode.EXTENSION_TYPE_EXTENDS", mensagem: "An extension type declaration can't have an 'extends' clause.", correcao: Some("Try removing the 'extends' clause or replacing the 'extends' with 'implements'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "extension_type_with", unico: "ParserErrorCode.EXTENSION_TYPE_WITH", mensagem: "An extension type declaration can't have a 'with' clause.", correcao: Some("Try removing the 'with' clause or replacing the 'with' with 'implements'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "external_class", unico: "ParserErrorCode.EXTERNAL_CLASS", mensagem: "Classes can't be declared to be 'external'.", correcao: Some("Try removing the keyword 'external'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "external_constructor_with_body", unico: "ParserErrorCode.EXTERNAL_CONSTRUCTOR_WITH_BODY", mensagem: "External constructors can't have a body.", correcao: Some("Try removing the body of the constructor, or removing the keyword 'external'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "external_constructor_with_field_initializers", unico: "ParserErrorCode.EXTERNAL_CONSTRUCTOR_WITH_FIELD_INITIALIZERS", mensagem: "An external constructor can't initialize fields.", correcao: Some("Try removing the field initializers, or removing the keyword 'external'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "external_constructor_with_initializer", unico: "ParserErrorCode.EXTERNAL_CONSTRUCTOR_WITH_INITIALIZER", mensagem: "An external constructor can't have any initializers.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "external_enum", unico: "ParserErrorCode.EXTERNAL_ENUM", mensagem: "Enums can't be declared to be 'external'.", correcao: Some("Try removing the keyword 'external'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "external_factory_redirection", unico: "ParserErrorCode.EXTERNAL_FACTORY_REDIRECTION", mensagem: "A redirecting factory can't be external.", correcao: Some("Try removing the 'external' modifier."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "external_factory_with_body", unico: "ParserErrorCode.EXTERNAL_FACTORY_WITH_BODY", mensagem: "External factories can't have a body.", correcao: Some("Try removing the body of the factory, or removing the keyword 'external'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "external_field", unico: "ParserErrorCode.EXTERNAL_FIELD", mensagem: "Fields can't be declared to be 'external'.", correcao: Some("Try removing the keyword 'external', or replacing the field by an external getter and/or setter."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "external_getter_with_body", unico: "ParserErrorCode.EXTERNAL_GETTER_WITH_BODY", mensagem: "External getters can't have a body.", correcao: Some("Try removing the body of the getter, or removing the keyword 'external'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "external_late_field", unico: "ParserErrorCode.EXTERNAL_LATE_FIELD", mensagem: "External fields cannot be late.", correcao: Some("Try removing the 'external' or 'late' keyword."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "external_method_with_body", unico: "ParserErrorCode.EXTERNAL_METHOD_WITH_BODY", mensagem: "An external or native method can't have a body.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "external_operator_with_body", unico: "ParserErrorCode.EXTERNAL_OPERATOR_WITH_BODY", mensagem: "External operators can't have a body.", correcao: Some("Try removing the body of the operator, or removing the keyword 'external'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "external_setter_with_body", unico: "ParserErrorCode.EXTERNAL_SETTER_WITH_BODY", mensagem: "External setters can't have a body.", correcao: Some("Try removing the body of the setter, or removing the keyword 'external'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "external_typedef", unico: "ParserErrorCode.EXTERNAL_TYPEDEF", mensagem: "Typedefs can't be declared to be 'external'.", correcao: Some("Try removing the keyword 'external'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "extraneous_modifier", unico: "ParserErrorCode.EXTRANEOUS_MODIFIER", mensagem: "Can't have modifier '{0}' here.", correcao: Some("Try removing '{0}'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "extraneous_modifier_in_extension_type", unico: "ParserErrorCode.EXTRANEOUS_MODIFIER_IN_EXTENSION_TYPE", mensagem: "Can't have modifier '{0}' in an extension type.", correcao: Some("Try removing '{0}'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "extraneous_modifier_in_primary_constructor", unico: "ParserErrorCode.EXTRANEOUS_MODIFIER_IN_PRIMARY_CONSTRUCTOR", mensagem: "Can't have modifier '{0}' in a primary constructor.", correcao: Some("Try removing '{0}'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "factory_top_level_declaration", unico: "ParserErrorCode.FACTORY_TOP_LEVEL_DECLARATION", mensagem: "Top-level declarations can't be declared to be 'factory'.", correcao: Some("Try removing the keyword 'factory'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "factory_without_body", unico: "ParserErrorCode.FACTORY_WITHOUT_BODY", mensagem: "A non-redirecting 'factory' constructor must have a body.", correcao: Some("Try adding a body to the constructor."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "factory_with_initializers", unico: "ParserErrorCode.FACTORY_WITH_INITIALIZERS", mensagem: "A 'factory' constructor can't have initializers.", correcao: Some("Try removing the 'factory' keyword to make this a generative constructor, or removing the initializers."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "field_initialized_outside_declaring_class", unico: "ParserErrorCode.FIELD_INITIALIZED_OUTSIDE_DECLARING_CLASS", mensagem: "A field can only be initialized in its declaring class", correcao: Some("Try passing a value into the superclass constructor, or moving the initialization into the constructor body."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "field_initializer_outside_constructor", unico: "ParserErrorCode.FIELD_INITIALIZER_OUTSIDE_CONSTRUCTOR", mensagem: "Field formal parameters can only be used in a constructor.", correcao: Some("Try removing 'this.'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "final_and_covariant", unico: "ParserErrorCode.FINAL_AND_COVARIANT", mensagem: "Members can't be declared to be both 'final' and 'covariant'.", correcao: Some("Try removing either the 'final' or 'covariant' keyword."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "final_and_covariant_late_with_initializer", unico: "ParserErrorCode.FINAL_AND_COVARIANT_LATE_WITH_INITIALIZER", mensagem: "Members marked 'late' with an initializer can't be declared to be both 'final' and 'covariant'.", correcao: Some("Try removing either the 'final' or 'covariant' keyword, or removing the initializer."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "final_and_var", unico: "ParserErrorCode.FINAL_AND_VAR", mensagem: "Members can't be declared to be both 'final' and 'var'.", correcao: Some("Try removing the keyword 'var'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "final_constructor", unico: "ParserErrorCode.FINAL_CONSTRUCTOR", mensagem: "A constructor can't be declared to be 'final'.", correcao: Some("Try removing the keyword 'final'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "final_enum", unico: "ParserErrorCode.FINAL_ENUM", mensagem: "Enums can't be declared to be 'final'.", correcao: Some("Try removing the keyword 'final'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "final_method", unico: "ParserErrorCode.FINAL_METHOD", mensagem: "Getters, setters and methods can't be declared to be 'final'.", correcao: Some("Try removing the keyword 'final'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "final_mixin", unico: "ParserErrorCode.FINAL_MIXIN", mensagem: "A mixin can't be declared 'final'.", correcao: Some("Try removing the 'final' keyword."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "final_mixin_class", unico: "ParserErrorCode.FINAL_MIXIN_CLASS", mensagem: "A mixin class can't be declared 'final'.", correcao: Some("Try removing the 'final' keyword."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "function_typed_parameter_var", unico: "ParserErrorCode.FUNCTION_TYPED_PARAMETER_VAR", mensagem: "Function-typed parameters can't specify 'const', 'final' or 'var' in place of a return type.", correcao: Some("Try replacing the keyword with a return type."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "getter_constructor", unico: "ParserErrorCode.GETTER_CONSTRUCTOR", mensagem: "Constructors can't be a getter.", correcao: Some("Try removing 'get'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "getter_in_function", unico: "ParserErrorCode.GETTER_IN_FUNCTION", mensagem: "Getters can't be defined within methods or functions.", correcao: Some("Try moving the getter outside the method or function, or converting the getter to a function."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "getter_with_parameters", unico: "ParserErrorCode.GETTER_WITH_PARAMETERS", mensagem: "Getters must be declared without a parameter list.", correcao: Some("Try removing the parameter list, or removing the keyword 'get' to define a method rather than a getter."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "illegal_assignment_to_non_assignable", unico: "ParserErrorCode.ILLEGAL_ASSIGNMENT_TO_NON_ASSIGNABLE", mensagem: "Illegal assignment to non-assignable expression.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "illegal_pattern_assignment_variable_name", unico: "ParserErrorCode.ILLEGAL_PATTERN_ASSIGNMENT_VARIABLE_NAME", mensagem: "A variable assigned by a pattern assignment can't be named '{0}'.", correcao: Some("Choose a different name."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "illegal_pattern_identifier_name", unico: "ParserErrorCode.ILLEGAL_PATTERN_IDENTIFIER_NAME", mensagem: "A pattern can't refer to an identifier named '{0}'.", correcao: Some("Match the identifier using '=="), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "illegal_pattern_variable_name", unico: "ParserErrorCode.ILLEGAL_PATTERN_VARIABLE_NAME", mensagem: "The variable declared by a variable pattern can't be named '{0}'.", correcao: Some("Choose a different name."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "implements_before_extends", unico: "ParserErrorCode.IMPLEMENTS_BEFORE_EXTENDS", mensagem: "The extends clause must be before the implements clause.", correcao: Some("Try moving the extends clause before the implements clause."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "implements_before_on", unico: "ParserErrorCode.IMPLEMENTS_BEFORE_ON", mensagem: "The on clause must be before the implements clause.", correcao: Some("Try moving the on clause before the implements clause."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "implements_before_with", unico: "ParserErrorCode.IMPLEMENTS_BEFORE_WITH", mensagem: "The with clause must be before the implements clause.", correcao: Some("Try moving the with clause before the implements clause."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "import_directive_after_part_directive", unico: "ParserErrorCode.IMPORT_DIRECTIVE_AFTER_PART_DIRECTIVE", mensagem: "Import directives must precede part directives.", correcao: Some("Try moving the import directives before the part directives."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "initialized_variable_in_for_each", unico: "ParserErrorCode.INITIALIZED_VARIABLE_IN_FOR_EACH", mensagem: "The loop variable in a for-each loop can't be initialized.", correcao: Some("Try removing the initializer, or using a different kind of loop."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "interface_enum", unico: "ParserErrorCode.INTERFACE_ENUM", mensagem: "Enums can't be declared to be 'interface'.", correcao: Some("Try removing the keyword 'interface'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "interface_mixin", unico: "ParserErrorCode.INTERFACE_MIXIN", mensagem: "A mixin can't be declared 'interface'.", correcao: Some("Try removing the 'interface' keyword."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "interface_mixin_class", unico: "ParserErrorCode.INTERFACE_MIXIN_CLASS", mensagem: "A mixin class can't be declared 'interface'.", correcao: Some("Try removing the 'interface' keyword."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_await_in_for", unico: "ParserErrorCode.INVALID_AWAIT_IN_FOR", mensagem: "The keyword 'await' isn't allowed for a normal 'for' statement.", correcao: Some("Try removing the keyword, or use a for-each statement."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_code_point", unico: "ParserErrorCode.INVALID_CODE_POINT", mensagem: "The escape sequence '{0}' isn't a valid code point.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_comment_reference", unico: "ParserErrorCode.INVALID_COMMENT_REFERENCE", mensagem: "Comment references should contain a possibly prefixed identifier and can start with 'new', but shouldn't contain anything else.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_constant_const_prefix", unico: "ParserErrorCode.INVALID_CONSTANT_CONST_PREFIX", mensagem: "The expression can't be prefixed by 'const' to form a constant pattern.", correcao: Some("Try wrapping the expression in 'const ( ... )' instead."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_constant_pattern_binary", unico: "ParserErrorCode.INVALID_CONSTANT_PATTERN_BINARY", mensagem: "The binary operator {0} is not supported as a constant pattern.", correcao: Some("Try wrapping the expression in 'const ( ... )'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_constant_pattern_duplicate_const", unico: "ParserErrorCode.INVALID_CONSTANT_PATTERN_DUPLICATE_CONST", mensagem: "Duplicate 'const' keyword in constant expression.", correcao: Some("Try removing one of the 'const' keywords."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_constant_pattern_empty_record_literal", unico: "ParserErrorCode.INVALID_CONSTANT_PATTERN_EMPTY_RECORD_LITERAL", mensagem: "The empty record literal is not supported as a constant pattern.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_constant_pattern_generic", unico: "ParserErrorCode.INVALID_CONSTANT_PATTERN_GENERIC", mensagem: "This expression is not supported as a constant pattern.", correcao: Some("Try wrapping the expression in 'const ( ... )'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_constant_pattern_negation", unico: "ParserErrorCode.INVALID_CONSTANT_PATTERN_NEGATION", mensagem: "Only negation of a numeric literal is supported as a constant pattern.", correcao: Some("Try wrapping the expression in 'const ( ... )'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_constant_pattern_unary", unico: "ParserErrorCode.INVALID_CONSTANT_PATTERN_UNARY", mensagem: "The unary operator {0} is not supported as a constant pattern.", correcao: Some("Try wrapping the expression in 'const ( ... )'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_constructor_name", unico: "ParserErrorCode.INVALID_CONSTRUCTOR_NAME", mensagem: "The name of a constructor must match the name of the enclosing class.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_generic_function_type", unico: "ParserErrorCode.INVALID_GENERIC_FUNCTION_TYPE", mensagem: "Invalid generic function type.", correcao: Some("Try using a generic function type (returnType 'Function(' parameters ')')."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_hex_escape", unico: "ParserErrorCode.INVALID_HEX_ESCAPE", mensagem: "An escape sequence starting with '\\x' must be followed by 2 hexadecimal digits.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_initializer", unico: "ParserErrorCode.INVALID_INITIALIZER", mensagem: "Not a valid initializer.", correcao: Some("To initialize a field, use the syntax 'name = value'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_inside_unary_pattern", unico: "ParserErrorCode.INVALID_INSIDE_UNARY_PATTERN", mensagem: "This pattern cannot appear inside a unary pattern (cast pattern, null check pattern, or null assert pattern) without parentheses.", correcao: Some("Try combining into a single pattern if possible, or enclose the inner pattern in parentheses."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_literal_in_configuration", unico: "ParserErrorCode.INVALID_LITERAL_IN_CONFIGURATION", mensagem: "The literal in a configuration can't contain interpolation.", correcao: Some("Try removing the interpolation expressions."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_operator", unico: "ParserErrorCode.INVALID_OPERATOR", mensagem: "The string '{0}' isn't a user-definable operator.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_operator_for_super", unico: "ParserErrorCode.INVALID_OPERATOR_FOR_SUPER", mensagem: "The operator '{0}' can't be used with 'super'.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_operator_questionmark_period_for_super", unico: "ParserErrorCode.INVALID_OPERATOR_QUESTIONMARK_PERIOD_FOR_SUPER", mensagem: "The operator '?.' cannot be used with 'super' because 'super' cannot be null.", correcao: Some("Try replacing '?.' with '.'"), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_star_after_async", unico: "ParserErrorCode.INVALID_STAR_AFTER_ASYNC", mensagem: "The modifier 'async*' isn't allowed for an expression function body.", correcao: Some("Try converting the body to a block."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_super_in_initializer", unico: "ParserErrorCode.INVALID_SUPER_IN_INITIALIZER", mensagem: "Can only use 'super' in an initializer for calling the superclass constructor (e.g. 'super()' or 'super.namedConstructor()')", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_sync", unico: "ParserErrorCode.INVALID_SYNC", mensagem: "The modifier 'sync' isn't allowed for an expression function body.", correcao: Some("Try converting the body to a block."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_this_in_initializer", unico: "ParserErrorCode.INVALID_THIS_IN_INITIALIZER", mensagem: "Can only use 'this' in an initializer for field initialization (e.g. 'this.x = something') and constructor redirection (e.g. 'this()' or 'this.namedConstructor())", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_unicode_escape_started", unico: "ParserErrorCode.INVALID_UNICODE_ESCAPE_STARTED", mensagem: "The string '\\' can't stand alone.", correcao: Some("Try adding another backslash (\\) to escape the '\\'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_unicode_escape_u_bracket", unico: "ParserErrorCode.INVALID_UNICODE_ESCAPE_U_BRACKET", mensagem: "An escape sequence starting with '\\u{' must be followed by 1 to 6 hexadecimal digits followed by a '}'.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_unicode_escape_u_no_bracket", unico: "ParserErrorCode.INVALID_UNICODE_ESCAPE_U_NO_BRACKET", mensagem: "An escape sequence starting with '\\u' must be followed by 4 hexadecimal digits.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_unicode_escape_u_started", unico: "ParserErrorCode.INVALID_UNICODE_ESCAPE_U_STARTED", mensagem: "An escape sequence starting with '\\u' must be followed by 4 hexadecimal digits or from 1 to 6 digits between '{' and '}'.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "invalid_use_of_covariant_in_extension", unico: "ParserErrorCode.INVALID_USE_OF_COVARIANT_IN_EXTENSION", mensagem: "Can't have modifier '{0}' in an extension.", correcao: Some("Try removing '{0}'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "invalid_use_of_identifier_augmented", unico: "ParserErrorCode.INVALID_USE_OF_IDENTIFIER_AUGMENTED", mensagem: "The identifier 'augmented' can only be used to reference the augmented declaration inside an augmentation.", correcao: Some("Try using a different identifier."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "late_pattern_variable_declaration", unico: "ParserErrorCode.LATE_PATTERN_VARIABLE_DECLARATION", mensagem: "A pattern variable declaration may not use the `late` keyword.", correcao: Some("Try removing the keyword `late`."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "library_directive_not_first", unico: "ParserErrorCode.LIBRARY_DIRECTIVE_NOT_FIRST", mensagem: "The library directive must appear before all other directives.", correcao: Some("Try moving the library directive before any other directives."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "literal_with_class", unico: "ParserErrorCode.LITERAL_WITH_CLASS", mensagem: "A {0} literal can't be prefixed by '{1}'.", correcao: Some("Try removing '{1}'"), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "literal_with_class_and_new", unico: "ParserErrorCode.LITERAL_WITH_CLASS_AND_NEW", mensagem: "A {0} literal can't be prefixed by 'new {1}'.", correcao: Some("Try removing 'new' and '{1}'"), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "literal_with_new", unico: "ParserErrorCode.LITERAL_WITH_NEW", mensagem: "A literal can't be prefixed by 'new'.", correcao: Some("Try removing 'new'"), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "local_function_declaration_modifier", unico: "ParserErrorCode.LOCAL_FUNCTION_DECLARATION_MODIFIER", mensagem: "Local function declarations can't specify any modifiers.", correcao: Some("Try removing the modifier."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "member_with_class_name", unico: "ParserErrorCode.MEMBER_WITH_CLASS_NAME", mensagem: "A class member can't have the same name as the enclosing class.", correcao: Some("Try renaming the member."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_assignable_selector", unico: "ParserErrorCode.MISSING_ASSIGNABLE_SELECTOR", mensagem: "Missing selector such as '.identifier' or '[0]'.", correcao: Some("Try adding a selector."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_assignment_in_initializer", unico: "ParserErrorCode.MISSING_ASSIGNMENT_IN_INITIALIZER", mensagem: "Expected an assignment after the field name.", correcao: Some("To initialize a field, use the syntax 'name = value'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_catch_or_finally", unico: "ParserErrorCode.MISSING_CATCH_OR_FINALLY", mensagem: "A try block must be followed by an 'on', 'catch', or 'finally' clause.", correcao: Some("Try adding either a catch or finally clause, or remove the try statement."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_closing_parenthesis", unico: "ParserErrorCode.MISSING_CLOSING_PARENTHESIS", mensagem: "The closing parenthesis is missing.", correcao: Some("Try adding the closing parenthesis."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_const_final_var_or_type", unico: "ParserErrorCode.MISSING_CONST_FINAL_VAR_OR_TYPE", mensagem: "Variables must be declared using the keywords 'const', 'final', 'var' or a type name.", correcao: Some("Try adding the name of the type of the variable or the keyword 'var'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_enum_body", unico: "ParserErrorCode.MISSING_ENUM_BODY", mensagem: "An enum definition must have a body with at least one constant name.", correcao: Some("Try adding a body and defining at least one constant."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_expression_in_initializer", unico: "ParserErrorCode.MISSING_EXPRESSION_IN_INITIALIZER", mensagem: "Expected an expression after the assignment operator.", correcao: Some("Try adding the value to be assigned, or remove the assignment operator."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_expression_in_throw", unico: "ParserErrorCode.MISSING_EXPRESSION_IN_THROW", mensagem: "Missing expression after 'throw'.", correcao: Some("Add an expression after 'throw' or use 'rethrow' to throw a caught exception"), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_function_body", unico: "ParserErrorCode.MISSING_FUNCTION_BODY", mensagem: "A function body must be provided.", correcao: Some("Try adding a function body."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_function_keyword", unico: "ParserErrorCode.MISSING_FUNCTION_KEYWORD", mensagem: "Function types must have the keyword 'Function' before the parameter list.", correcao: Some("Try adding the keyword 'Function'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_function_parameters", unico: "ParserErrorCode.MISSING_FUNCTION_PARAMETERS", mensagem: "Functions must have an explicit list of parameters.", correcao: Some("Try adding a parameter list."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_get", unico: "ParserErrorCode.MISSING_GET", mensagem: "Getters must have the keyword 'get' before the getter name.", correcao: Some("Try adding the keyword 'get'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_identifier", unico: "ParserErrorCode.MISSING_IDENTIFIER", mensagem: "Expected an identifier.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_initializer", unico: "ParserErrorCode.MISSING_INITIALIZER", mensagem: "Expected an initializer.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_keyword_operator", unico: "ParserErrorCode.MISSING_KEYWORD_OPERATOR", mensagem: "Operator declarations must be preceded by the keyword 'operator'.", correcao: Some("Try adding the keyword 'operator'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_method_parameters", unico: "ParserErrorCode.MISSING_METHOD_PARAMETERS", mensagem: "Methods must have an explicit list of parameters.", correcao: Some("Try adding a parameter list."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_name_for_named_parameter", unico: "ParserErrorCode.MISSING_NAME_FOR_NAMED_PARAMETER", mensagem: "Named parameters in a function type must have a name", correcao: Some("Try providing a name for the parameter or removing the curly braces."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_name_in_library_directive", unico: "ParserErrorCode.MISSING_NAME_IN_LIBRARY_DIRECTIVE", mensagem: "Library directives must include a library name.", correcao: Some("Try adding a library name after the keyword 'library', or remove the library directive if the library doesn't have any parts."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_name_in_part_of_directive", unico: "ParserErrorCode.MISSING_NAME_IN_PART_OF_DIRECTIVE", mensagem: "Part-of directives must include a library name.", correcao: Some("Try adding a library name after the 'of'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_prefix_in_deferred_import", unico: "ParserErrorCode.MISSING_PREFIX_IN_DEFERRED_IMPORT", mensagem: "Deferred imports should have a prefix.", correcao: Some("Try adding a prefix to the import by adding an 'as' clause."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_primary_constructor", unico: "ParserErrorCode.MISSING_PRIMARY_CONSTRUCTOR", mensagem: "An extension type declaration must have a primary constructor declaration.", correcao: Some("Try adding a primary constructor to the extension type declaration."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_primary_constructor_parameters", unico: "ParserErrorCode.MISSING_PRIMARY_CONSTRUCTOR_PARAMETERS", mensagem: "A primary constructor declaration must have formal parameters.", correcao: Some("Try adding formal parameters after the primary constructor name."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_star_after_sync", unico: "ParserErrorCode.MISSING_STAR_AFTER_SYNC", mensagem: "The modifier 'sync' must be followed by a star ('*').", correcao: Some("Try removing the modifier, or add a star."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_statement", unico: "ParserErrorCode.MISSING_STATEMENT", mensagem: "Expected a statement.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_terminator_for_parameter_group", unico: "ParserErrorCode.MISSING_TERMINATOR_FOR_PARAMETER_GROUP", mensagem: "There is no '{0}' to close the parameter group.", correcao: Some("Try inserting a '{0}' at the end of the group."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_typedef_parameters", unico: "ParserErrorCode.MISSING_TYPEDEF_PARAMETERS", mensagem: "Typedefs must have an explicit list of parameters.", correcao: Some("Try adding a parameter list."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_variable_in_for_each", unico: "ParserErrorCode.MISSING_VARIABLE_IN_FOR_EACH", mensagem: "A loop variable must be declared in a for-each loop before the 'in', but none was found.", correcao: Some("Try declaring a loop variable."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "mixed_parameter_groups", unico: "ParserErrorCode.MIXED_PARAMETER_GROUPS", mensagem: "Can't have both positional and named parameters in a single parameter list.", correcao: Some("Try choosing a single style of optional parameters."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "mixin_declares_constructor", unico: "ParserErrorCode.MIXIN_DECLARES_CONSTRUCTOR", mensagem: "Mixins can't declare constructors.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "mixin_with_clause", unico: "ParserErrorCode.MIXIN_WITH_CLAUSE", mensagem: "A mixin can't have a with clause.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "modifier_out_of_order", unico: "ParserErrorCode.MODIFIER_OUT_OF_ORDER", mensagem: "The modifier '{0}' should be before the modifier '{1}'.", correcao: Some("Try re-ordering the modifiers."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "multiple_clauses", unico: "ParserErrorCode.MULTIPLE_CLAUSES", mensagem: "Each '{0}' definition can have at most one '{1}' clause.", correcao: Some("Try combining all of the '{1}' clauses into a single clause."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "multiple_extends_clauses", unico: "ParserErrorCode.MULTIPLE_EXTENDS_CLAUSES", mensagem: "Each class definition can have at most one extends clause.", correcao: Some("Try choosing one superclass and define your class to implement (or mix in) the others."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "multiple_implements_clauses", unico: "ParserErrorCode.MULTIPLE_IMPLEMENTS_CLAUSES", mensagem: "Each class or mixin definition can have at most one implements clause.", correcao: Some("Try combining all of the implements clauses into a single clause."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "multiple_library_directives", unico: "ParserErrorCode.MULTIPLE_LIBRARY_DIRECTIVES", mensagem: "Only one library directive may be declared in a file.", correcao: Some("Try removing all but one of the library directives."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "multiple_named_parameter_groups", unico: "ParserErrorCode.MULTIPLE_NAMED_PARAMETER_GROUPS", mensagem: "Can't have multiple groups of named parameters in a single parameter list.", correcao: Some("Try combining all of the groups into a single group."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "multiple_on_clauses", unico: "ParserErrorCode.MULTIPLE_ON_CLAUSES", mensagem: "Each mixin definition can have at most one on clause.", correcao: Some("Try combining all of the on clauses into a single clause."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "multiple_part_of_directives", unico: "ParserErrorCode.MULTIPLE_PART_OF_DIRECTIVES", mensagem: "Only one part-of directive may be declared in a file.", correcao: Some("Try removing all but one of the part-of directives."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "multiple_positional_parameter_groups", unico: "ParserErrorCode.MULTIPLE_POSITIONAL_PARAMETER_GROUPS", mensagem: "Can't have multiple groups of positional parameters in a single parameter list.", correcao: Some("Try combining all of the groups into a single group."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "multiple_representation_fields", unico: "ParserErrorCode.MULTIPLE_REPRESENTATION_FIELDS", mensagem: "Each extension type should have exactly one representation field.", correcao: Some("Try combining fields into a record, or removing extra fields."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "multiple_variables_in_for_each", unico: "ParserErrorCode.MULTIPLE_VARIABLES_IN_FOR_EACH", mensagem: "A single loop variable must be declared in a for-each loop before the 'in', but {0} were found.", correcao: Some("Try moving all but one of the declarations inside the loop body."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "multiple_variance_modifiers", unico: "ParserErrorCode.MULTIPLE_VARIANCE_MODIFIERS", mensagem: "Each type parameter can have at most one variance modifier.", correcao: Some("Use at most one of the 'in', 'out', or 'inout' modifiers."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "multiple_with_clauses", unico: "ParserErrorCode.MULTIPLE_WITH_CLAUSES", mensagem: "Each class definition can have at most one with clause.", correcao: Some("Try combining all of the with clauses into a single clause."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "named_function_expression", unico: "ParserErrorCode.NAMED_FUNCTION_EXPRESSION", mensagem: "Function expressions can't be named.", correcao: Some("Try removing the name, or moving the function expression to a function declaration statement."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "named_function_type", unico: "ParserErrorCode.NAMED_FUNCTION_TYPE", mensagem: "Function types can't be named.", correcao: Some("Try replacing the name with the keyword 'Function'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "named_parameter_outside_group", unico: "ParserErrorCode.NAMED_PARAMETER_OUTSIDE_GROUP", mensagem: "Named parameters must be enclosed in curly braces ('{' and '}').", correcao: Some("Try surrounding the named parameters in curly braces."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "native_clause_in_non_sdk_code", unico: "ParserErrorCode.NATIVE_CLAUSE_IN_NON_SDK_CODE", mensagem: "Native clause can only be used in the SDK and code that is loaded through native extensions.", correcao: Some("Try removing the native clause."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "native_clause_should_be_annotation", unico: "ParserErrorCode.NATIVE_CLAUSE_SHOULD_BE_ANNOTATION", mensagem: "Native clause in this form is deprecated.", correcao: Some("Try removing this native clause and adding @native() or @native('native-name') before the declaration."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "native_function_body_in_non_sdk_code", unico: "ParserErrorCode.NATIVE_FUNCTION_BODY_IN_NON_SDK_CODE", mensagem: "Native functions can only be declared in the SDK and code that is loaded through native extensions.", correcao: Some("Try removing the word 'native'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "non_constructor_factory", unico: "ParserErrorCode.NON_CONSTRUCTOR_FACTORY", mensagem: "Only a constructor can be declared to be a factory.", correcao: Some("Try removing the keyword 'factory'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "non_identifier_library_name", unico: "ParserErrorCode.NON_IDENTIFIER_LIBRARY_NAME", mensagem: "The name of a library must be an identifier.", correcao: Some("Try using an identifier as the name of the library."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "non_part_of_directive_in_part", unico: "ParserErrorCode.NON_PART_OF_DIRECTIVE_IN_PART", mensagem: "The part-of directive must be the only directive in a part.", correcao: Some("Try removing the other directives, or moving them to the library for which this is a part."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "non_string_literal_as_uri", unico: "ParserErrorCode.NON_STRING_LITERAL_AS_URI", mensagem: "The URI must be a string literal.", correcao: Some("Try enclosing the URI in either single or double quotes."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "non_user_definable_operator", unico: "ParserErrorCode.NON_USER_DEFINABLE_OPERATOR", mensagem: "The operator '{0}' isn't user definable.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "normal_before_optional_parameters", unico: "ParserErrorCode.NORMAL_BEFORE_OPTIONAL_PARAMETERS", mensagem: "Normal parameters must occur before optional parameters.", correcao: Some("Try moving all of the normal parameters before the optional parameters."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "null_aware_cascade_out_of_order", unico: "ParserErrorCode.NULL_AWARE_CASCADE_OUT_OF_ORDER", mensagem: "The '?..' cascade operator must be first in the cascade sequence.", correcao: Some("Try moving the '?..' operator to be the first cascade operator in the sequence."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "out_of_order_clauses", unico: "ParserErrorCode.OUT_OF_ORDER_CLAUSES", mensagem: "The '{0}' clause must come before the '{1}' clause.", correcao: Some("Try moving the '{0}' clause before the '{1}' clause."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "part_of_name", unico: "ParserErrorCode.PART_OF_NAME", mensagem: "The 'part of' directive can't use a name with the enhanced-parts feature.", correcao: Some("Try using 'part of' with a URI instead."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "pattern_assignment_declares_variable", unico: "ParserErrorCode.PATTERN_ASSIGNMENT_DECLARES_VARIABLE", mensagem: "Variable '{0}' can't be declared in a pattern assignment.", correcao: Some("Try using a preexisting variable or changing the assignment to a pattern variable declaration."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "pattern_variable_declaration_outside_function_or_method", unico: "ParserErrorCode.PATTERN_VARIABLE_DECLARATION_OUTSIDE_FUNCTION_OR_METHOD", mensagem: "A pattern variable declaration may not appear outside a function or method.", correcao: Some("Try declaring ordinary variables and assigning from within a function or method."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "positional_after_named_argument", unico: "ParserErrorCode.POSITIONAL_AFTER_NAMED_ARGUMENT", mensagem: "Positional arguments must occur before named arguments.", correcao: Some("Try moving all of the positional arguments before the named arguments."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "positional_parameter_outside_group", unico: "ParserErrorCode.POSITIONAL_PARAMETER_OUTSIDE_GROUP", mensagem: "Positional parameters must be enclosed in square brackets ('[' and ']').", correcao: Some("Try surrounding the positional parameters in square brackets."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "prefix_after_combinator", unico: "ParserErrorCode.PREFIX_AFTER_COMBINATOR", mensagem: "The prefix ('as' clause) should come before any show/hide combinators.", correcao: Some("Try moving the prefix before the combinators."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "record_literal_one_positional_no_trailing_comma", unico: "ParserErrorCode.RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA", mensagem: "A record literal with exactly one positional field requires a trailing comma.", correcao: Some("Try adding a trailing comma."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "record_type_one_positional_no_trailing_comma", unico: "ParserErrorCode.RECORD_TYPE_ONE_POSITIONAL_NO_TRAILING_COMMA", mensagem: "A record type with exactly one positional field requires a trailing comma.", correcao: Some("Try adding a trailing comma."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "redirecting_constructor_with_body", unico: "ParserErrorCode.REDIRECTING_CONSTRUCTOR_WITH_BODY", mensagem: "Redirecting constructors can't have a body.", correcao: Some("Try removing the body, or not making this a redirecting constructor."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "redirection_in_non_factory_constructor", unico: "ParserErrorCode.REDIRECTION_IN_NON_FACTORY_CONSTRUCTOR", mensagem: "Only factory constructor can specify '=' redirection.", correcao: Some("Try making this a factory constructor, or remove the redirection."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "representation_field_modifier", unico: "ParserErrorCode.REPRESENTATION_FIELD_MODIFIER", mensagem: "Representation fields can't have modifiers.", correcao: Some("Try removing the modifier."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "representation_field_trailing_comma", unico: "ParserErrorCode.REPRESENTATION_FIELD_TRAILING_COMMA", mensagem: "The representation field can't have a trailing comma.", correcao: Some("Try removing the trailing comma."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "sealed_enum", unico: "ParserErrorCode.SEALED_ENUM", mensagem: "Enums can't be declared to be 'sealed'.", correcao: Some("Try removing the keyword 'sealed'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "sealed_mixin", unico: "ParserErrorCode.SEALED_MIXIN", mensagem: "A mixin can't be declared 'sealed'.", correcao: Some("Try removing the 'sealed' keyword."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "sealed_mixin_class", unico: "ParserErrorCode.SEALED_MIXIN_CLASS", mensagem: "A mixin class can't be declared 'sealed'.", correcao: Some("Try removing the 'sealed' keyword."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "setter_constructor", unico: "ParserErrorCode.SETTER_CONSTRUCTOR", mensagem: "Constructors can't be a setter.", correcao: Some("Try removing 'set'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "setter_in_function", unico: "ParserErrorCode.SETTER_IN_FUNCTION", mensagem: "Setters can't be defined within methods or functions.", correcao: Some("Try moving the setter outside the method or function."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "stack_overflow", unico: "ParserErrorCode.STACK_OVERFLOW", mensagem: "The file has too many nested expressions or statements.", correcao: Some("Try simplifying the code."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "static_constructor", unico: "ParserErrorCode.STATIC_CONSTRUCTOR", mensagem: "Constructors can't be static.", correcao: Some("Try removing the keyword 'static'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "static_getter_without_body", unico: "ParserErrorCode.STATIC_GETTER_WITHOUT_BODY", mensagem: "A 'static' getter must have a body.", correcao: Some("Try adding a body to the getter, or removing the keyword 'static'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "static_operator", unico: "ParserErrorCode.STATIC_OPERATOR", mensagem: "Operators can't be static.", correcao: Some("Try removing the keyword 'static'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "static_setter_without_body", unico: "ParserErrorCode.STATIC_SETTER_WITHOUT_BODY", mensagem: "A 'static' setter must have a body.", correcao: Some("Try adding a body to the setter, or removing the keyword 'static'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "switch_has_case_after_default_case", unico: "ParserErrorCode.SWITCH_HAS_CASE_AFTER_DEFAULT_CASE", mensagem: "The default case should be the last case in a switch statement.", correcao: Some("Try moving the default case after the other case clauses."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "switch_has_multiple_default_cases", unico: "ParserErrorCode.SWITCH_HAS_MULTIPLE_DEFAULT_CASES", mensagem: "The 'default' case can only be declared once.", correcao: Some("Try removing all but one default case."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "top_level_operator", unico: "ParserErrorCode.TOP_LEVEL_OPERATOR", mensagem: "Operators must be declared within a class.", correcao: Some("Try removing the operator, moving it to a class, or converting it to be a function."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "typedef_in_class", unico: "ParserErrorCode.TYPEDEF_IN_CLASS", mensagem: "Typedefs can't be declared inside classes.", correcao: Some("Try moving the typedef to the top-level."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "type_arguments_on_type_variable", unico: "ParserErrorCode.TYPE_ARGUMENTS_ON_TYPE_VARIABLE", mensagem: "Can't use type arguments with type variable '{0}'.", correcao: Some("Try removing the type arguments."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "type_before_factory", unico: "ParserErrorCode.TYPE_BEFORE_FACTORY", mensagem: "Factory constructors cannot have a return type.", correcao: Some("Try removing the type appearing before 'factory'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "type_parameter_on_constructor", unico: "ParserErrorCode.TYPE_PARAMETER_ON_CONSTRUCTOR", mensagem: "Constructors can't have type parameters.", correcao: Some("Try removing the type parameters."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "type_parameter_on_operator", unico: "ParserErrorCode.TYPE_PARAMETER_ON_OPERATOR", mensagem: "Types parameters aren't allowed when defining an operator.", correcao: Some("Try removing the type parameters."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "unexpected_terminator_for_parameter_group", unico: "ParserErrorCode.UNEXPECTED_TERMINATOR_FOR_PARAMETER_GROUP", mensagem: "There is no '{0}' to open a parameter group.", correcao: Some("Try inserting the '{0}' at the appropriate location."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "unexpected_token", unico: "ParserErrorCode.UNEXPECTED_TOKEN", mensagem: "Unexpected text '{0}'.", correcao: Some("Try removing the text."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "unexpected_tokens", unico: "ParserErrorCode.UNEXPECTED_TOKENS", mensagem: "Unexpected tokens.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "variable_pattern_keyword_in_declaration_context", unico: "ParserErrorCode.VARIABLE_PATTERN_KEYWORD_IN_DECLARATION_CONTEXT", mensagem: "Variable patterns in declaration context can't specify 'var' or 'final' keyword.", correcao: Some("Try removing the keyword."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: true },
    InfoCodigo { nome: "var_and_type", unico: "ParserErrorCode.VAR_AND_TYPE", mensagem: "Variables can't be declared using both 'var' and a type name.", correcao: Some("Try removing 'var.'"), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "var_as_type_name", unico: "ParserErrorCode.VAR_AS_TYPE_NAME", mensagem: "The keyword 'var' can't be used as a type name.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "var_class", unico: "ParserErrorCode.VAR_CLASS", mensagem: "Classes can't be declared to be 'var'.", correcao: Some("Try removing the keyword 'var'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "var_enum", unico: "ParserErrorCode.VAR_ENUM", mensagem: "Enums can't be declared to be 'var'.", correcao: Some("Try removing the keyword 'var'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "var_return_type", unico: "ParserErrorCode.VAR_RETURN_TYPE", mensagem: "The return type can't be 'var'.", correcao: Some("Try removing the keyword 'var', or replacing it with the name of the return type."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "var_typedef", unico: "ParserErrorCode.VAR_TYPEDEF", mensagem: "Typedefs can't be declared to be 'var'.", correcao: Some("Try removing the keyword 'var', or replacing it with the name of the return type."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "void_with_type_arguments", unico: "ParserErrorCode.VOID_WITH_TYPE_ARGUMENTS", mensagem: "Type 'void' can't have type arguments.", correcao: Some("Try removing the type arguments."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "with_before_extends", unico: "ParserErrorCode.WITH_BEFORE_EXTENDS", mensagem: "The extends clause must be before the with clause.", correcao: Some("Try moving the extends clause before the with clause."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "wrong_separator_for_positional_parameter", unico: "ParserErrorCode.WRONG_SEPARATOR_FOR_POSITIONAL_PARAMETER", mensagem: "The default value of a positional parameter should be preceded by '='.", correcao: Some("Try replacing the ':' with '='."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "wrong_terminator_for_parameter_group", unico: "ParserErrorCode.WRONG_TERMINATOR_FOR_PARAMETER_GROUP", mensagem: "Expected '{0}' to close parameter group.", correcao: Some("Try replacing '{0}' with '{1}'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "expected_token", unico: "ScannerErrorCode.EXPECTED_TOKEN", mensagem: "Expected to find '{0}'.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "illegal_character", unico: "ScannerErrorCode.ILLEGAL_CHARACTER", mensagem: "Illegal character '{0}'.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_digit", unico: "ScannerErrorCode.MISSING_DIGIT", mensagem: "Decimal digit expected.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_hex_digit", unico: "ScannerErrorCode.MISSING_HEX_DIGIT", mensagem: "Hexadecimal digit expected.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_identifier", unico: "ScannerErrorCode.MISSING_IDENTIFIER", mensagem: "Expected an identifier.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "missing_quote", unico: "ScannerErrorCode.MISSING_QUOTE", mensagem: "Expected quote (' or \").", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "unable_get_content", unico: "ScannerErrorCode.UNABLE_GET_CONTENT", mensagem: "Unable to get content of '{0}'.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "unexpected_dollar_in_string", unico: "ScannerErrorCode.UNEXPECTED_DOLLAR_IN_STRING", mensagem: "A '$' has special meaning inside a string, and must be followed by an identifier or an expression in curly braces ({}).", correcao: Some("Try adding a backslash (\\) to escape the '$'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "unexpected_separator_in_number", unico: "ScannerErrorCode.UNEXPECTED_SEPARATOR_IN_NUMBER", mensagem: "Digit separators ('_') in a number literal can only be placed between two digits.", correcao: Some("Try removing the '_'."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "unsupported_operator", unico: "ScannerErrorCode.UNSUPPORTED_OPERATOR", mensagem: "The '{0}' operator is not supported.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "unterminated_multi_line_comment", unico: "ScannerErrorCode.UNTERMINATED_MULTI_LINE_COMMENT", mensagem: "Unterminated multi-line comment.", correcao: Some("Try terminating the comment with '*/', or removing any unbalanced occurrences of '/*' (because comments nest in Dart)."), tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "unterminated_string_literal", unico: "ScannerErrorCode.UNTERMINATED_STRING_LITERAL", mensagem: "Unterminated string literal.", correcao: None, tipo: TipoErro::SyntacticError, severidade: Severidade::Error, documentado: false },
    InfoCodigo { nome: "todo", unico: "TodoCode.TODO", mensagem: "{0}", correcao: None, tipo: TipoErro::Todo, severidade: Severidade::Info, documentado: false },
    InfoCodigo { nome: "fixme", unico: "TodoCode.FIXME", mensagem: "{0}", correcao: None, tipo: TipoErro::Todo, severidade: Severidade::Info, documentado: false },
    InfoCodigo { nome: "hack", unico: "TodoCode.HACK", mensagem: "{0}", correcao: None, tipo: TipoErro::Todo, severidade: Severidade::Info, documentado: false },
    InfoCodigo { nome: "undone", unico: "TodoCode.UNDONE", mensagem: "{0}", correcao: None, tipo: TipoErro::Todo, severidade: Severidade::Info, documentado: false },
];

pub(crate) static POR_UNICO: [(&str, u16); 1030] = [
    ("CompileTimeErrorCode.ABSTRACT_FIELD_CONSTRUCTOR_INITIALIZER", 0),
    ("CompileTimeErrorCode.ABSTRACT_FIELD_INITIALIZER", 1),
    ("CompileTimeErrorCode.ABSTRACT_SUPER_MEMBER_REFERENCE", 2),
    ("CompileTimeErrorCode.AMBIGUOUS_EXPORT", 3),
    ("CompileTimeErrorCode.AMBIGUOUS_EXTENSION_MEMBER_ACCESS", 4),
    ("CompileTimeErrorCode.AMBIGUOUS_IMPORT", 5),
    ("CompileTimeErrorCode.AMBIGUOUS_SET_OR_MAP_LITERAL_BOTH", 6),
    ("CompileTimeErrorCode.AMBIGUOUS_SET_OR_MAP_LITERAL_EITHER", 7),
    ("CompileTimeErrorCode.ARGUMENT_TYPE_NOT_ASSIGNABLE", 8),
    ("CompileTimeErrorCode.ASSERT_IN_REDIRECTING_CONSTRUCTOR", 9),
    ("CompileTimeErrorCode.ASSIGNMENT_TO_CONST", 10),
    ("CompileTimeErrorCode.ASSIGNMENT_TO_FINAL", 11),
    ("CompileTimeErrorCode.ASSIGNMENT_TO_FINAL_LOCAL", 12),
    ("CompileTimeErrorCode.ASSIGNMENT_TO_FINAL_NO_SETTER", 13),
    ("CompileTimeErrorCode.ASSIGNMENT_TO_FUNCTION", 14),
    ("CompileTimeErrorCode.ASSIGNMENT_TO_METHOD", 15),
    ("CompileTimeErrorCode.ASSIGNMENT_TO_TYPE", 16),
    ("CompileTimeErrorCode.ASYNC_FOR_IN_WRONG_CONTEXT", 17),
    ("CompileTimeErrorCode.AUGMENTATION_EXTENDS_CLAUSE_ALREADY_PRESENT", 18),
    ("CompileTimeErrorCode.AUGMENTATION_MODIFIER_EXTRA", 19),
    ("CompileTimeErrorCode.AUGMENTATION_MODIFIER_MISSING", 20),
    ("CompileTimeErrorCode.AUGMENTATION_OF_DIFFERENT_DECLARATION_KIND", 21),
    ("CompileTimeErrorCode.AUGMENTATION_TYPE_PARAMETER_BOUND", 22),
    ("CompileTimeErrorCode.AUGMENTATION_TYPE_PARAMETER_COUNT", 23),
    ("CompileTimeErrorCode.AUGMENTATION_TYPE_PARAMETER_NAME", 24),
    ("CompileTimeErrorCode.AUGMENTATION_WITHOUT_DECLARATION", 25),
    ("CompileTimeErrorCode.AUGMENTED_EXPRESSION_IS_NOT_SETTER", 26),
    ("CompileTimeErrorCode.AUGMENTED_EXPRESSION_IS_SETTER", 27),
    ("CompileTimeErrorCode.AUGMENTED_EXPRESSION_NOT_OPERATOR", 28),
    ("CompileTimeErrorCode.AWAIT_IN_LATE_LOCAL_VARIABLE_INITIALIZER", 29),
    ("CompileTimeErrorCode.AWAIT_IN_WRONG_CONTEXT", 30),
    ("CompileTimeErrorCode.AWAIT_OF_INCOMPATIBLE_TYPE", 31),
    ("CompileTimeErrorCode.BASE_CLASS_IMPLEMENTED_OUTSIDE_OF_LIBRARY", 32),
    ("CompileTimeErrorCode.BASE_MIXIN_IMPLEMENTED_OUTSIDE_OF_LIBRARY", 33),
    ("CompileTimeErrorCode.BODY_MIGHT_COMPLETE_NORMALLY", 34),
    ("CompileTimeErrorCode.BREAK_LABEL_ON_SWITCH_MEMBER", 35),
    ("CompileTimeErrorCode.BUILT_IN_IDENTIFIER_AS_EXTENSION_NAME", 36),
    ("CompileTimeErrorCode.BUILT_IN_IDENTIFIER_AS_EXTENSION_TYPE_NAME", 37),
    ("CompileTimeErrorCode.BUILT_IN_IDENTIFIER_AS_PREFIX_NAME", 38),
    ("CompileTimeErrorCode.BUILT_IN_IDENTIFIER_AS_TYPE", 39),
    ("CompileTimeErrorCode.BUILT_IN_IDENTIFIER_AS_TYPEDEF_NAME", 40),
    ("CompileTimeErrorCode.BUILT_IN_IDENTIFIER_AS_TYPE_NAME", 41),
    ("CompileTimeErrorCode.BUILT_IN_IDENTIFIER_AS_TYPE_PARAMETER_NAME", 42),
    ("CompileTimeErrorCode.CASE_EXPRESSION_TYPE_IMPLEMENTS_EQUALS", 43),
    ("CompileTimeErrorCode.CASE_EXPRESSION_TYPE_IS_NOT_SWITCH_EXPRESSION_SUBTYPE", 44),
    ("CompileTimeErrorCode.CAST_TO_NON_TYPE", 45),
    ("CompileTimeErrorCode.CLASS_INSTANTIATION_ACCESS_TO_INSTANCE_MEMBER", 46),
    ("CompileTimeErrorCode.CLASS_INSTANTIATION_ACCESS_TO_STATIC_MEMBER", 47),
    ("CompileTimeErrorCode.CLASS_INSTANTIATION_ACCESS_TO_UNKNOWN_MEMBER", 48),
    ("CompileTimeErrorCode.CLASS_USED_AS_MIXIN", 49),
    ("CompileTimeErrorCode.CONCRETE_CLASS_HAS_ENUM_SUPERINTERFACE", 50),
    ("CompileTimeErrorCode.CONCRETE_CLASS_WITH_ABSTRACT_MEMBER", 51),
    ("CompileTimeErrorCode.CONFLICTING_CONSTRUCTOR_AND_STATIC_FIELD", 52),
    ("CompileTimeErrorCode.CONFLICTING_CONSTRUCTOR_AND_STATIC_GETTER", 53),
    ("CompileTimeErrorCode.CONFLICTING_CONSTRUCTOR_AND_STATIC_METHOD", 54),
    ("CompileTimeErrorCode.CONFLICTING_CONSTRUCTOR_AND_STATIC_SETTER", 55),
    ("CompileTimeErrorCode.CONFLICTING_FIELD_AND_METHOD", 56),
    ("CompileTimeErrorCode.CONFLICTING_GENERIC_INTERFACES", 57),
    ("CompileTimeErrorCode.CONFLICTING_INHERITED_METHOD_AND_SETTER", 58),
    ("CompileTimeErrorCode.CONFLICTING_METHOD_AND_FIELD", 59),
    ("CompileTimeErrorCode.CONFLICTING_STATIC_AND_INSTANCE", 60),
    ("CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_CLASS", 61),
    ("CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_ENUM", 62),
    ("CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_EXTENSION", 63),
    ("CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_EXTENSION_TYPE", 64),
    ("CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_MEMBER_CLASS", 65),
    ("CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_MEMBER_ENUM", 66),
    ("CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_MEMBER_EXTENSION", 67),
    ("CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_MEMBER_EXTENSION_TYPE", 68),
    ("CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_MEMBER_MIXIN", 69),
    ("CompileTimeErrorCode.CONFLICTING_TYPE_VARIABLE_AND_MIXIN", 70),
    ("CompileTimeErrorCode.CONSTANT_PATTERN_WITH_NON_CONSTANT_EXPRESSION", 71),
    ("CompileTimeErrorCode.CONST_CONSTRUCTOR_CONSTANT_FROM_DEFERRED_LIBRARY", 72),
    ("CompileTimeErrorCode.CONST_CONSTRUCTOR_FIELD_TYPE_MISMATCH", 73),
    ("CompileTimeErrorCode.CONST_CONSTRUCTOR_PARAM_TYPE_MISMATCH", 74),
    ("CompileTimeErrorCode.CONST_CONSTRUCTOR_THROWS_EXCEPTION", 75),
    ("CompileTimeErrorCode.CONST_CONSTRUCTOR_WITH_FIELD_INITIALIZED_BY_NON_CONST", 76),
    ("CompileTimeErrorCode.CONST_CONSTRUCTOR_WITH_MIXIN_WITH_FIELD", 77),
    ("CompileTimeErrorCode.CONST_CONSTRUCTOR_WITH_MIXIN_WITH_FIELDS", 78),
    ("CompileTimeErrorCode.CONST_CONSTRUCTOR_WITH_NON_CONST_SUPER", 79),
    ("CompileTimeErrorCode.CONST_CONSTRUCTOR_WITH_NON_FINAL_FIELD", 80),
    ("CompileTimeErrorCode.CONST_DEFERRED_CLASS", 81),
    ("CompileTimeErrorCode.CONST_EVAL_ASSERTION_FAILURE", 82),
    ("CompileTimeErrorCode.CONST_EVAL_ASSERTION_FAILURE_WITH_MESSAGE", 83),
    ("CompileTimeErrorCode.CONST_EVAL_EXTENSION_METHOD", 84),
    ("CompileTimeErrorCode.CONST_EVAL_EXTENSION_TYPE_METHOD", 85),
    ("CompileTimeErrorCode.CONST_EVAL_FOR_ELEMENT", 86),
    ("CompileTimeErrorCode.CONST_EVAL_METHOD_INVOCATION", 87),
    ("CompileTimeErrorCode.CONST_EVAL_PROPERTY_ACCESS", 88),
    ("CompileTimeErrorCode.CONST_EVAL_THROWS_EXCEPTION", 89),
    ("CompileTimeErrorCode.CONST_EVAL_THROWS_IDBZE", 90),
    ("CompileTimeErrorCode.CONST_EVAL_TYPE_BOOL", 91),
    ("CompileTimeErrorCode.CONST_EVAL_TYPE_BOOL_INT", 92),
    ("CompileTimeErrorCode.CONST_EVAL_TYPE_BOOL_NUM_STRING", 93),
    ("CompileTimeErrorCode.CONST_EVAL_TYPE_INT", 94),
    ("CompileTimeErrorCode.CONST_EVAL_TYPE_NUM", 95),
    ("CompileTimeErrorCode.CONST_EVAL_TYPE_NUM_STRING", 96),
    ("CompileTimeErrorCode.CONST_EVAL_TYPE_STRING", 97),
    ("CompileTimeErrorCode.CONST_EVAL_TYPE_TYPE", 98),
    ("CompileTimeErrorCode.CONST_FIELD_INITIALIZER_NOT_ASSIGNABLE", 99),
    ("CompileTimeErrorCode.CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE", 100),
    ("CompileTimeErrorCode.CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE_FROM_DEFERRED_LIBRARY", 101),
    ("CompileTimeErrorCode.CONST_INSTANCE_FIELD", 102),
    ("CompileTimeErrorCode.CONST_MAP_KEY_NOT_PRIMITIVE_EQUALITY", 103),
    ("CompileTimeErrorCode.CONST_NOT_INITIALIZED", 104),
    ("CompileTimeErrorCode.CONST_SET_ELEMENT_NOT_PRIMITIVE_EQUALITY", 105),
    ("CompileTimeErrorCode.CONST_SPREAD_EXPECTED_LIST_OR_SET", 106),
    ("CompileTimeErrorCode.CONST_SPREAD_EXPECTED_MAP", 107),
    ("CompileTimeErrorCode.CONST_TYPE_PARAMETER", 108),
    ("CompileTimeErrorCode.CONST_WITH_NON_CONST", 109),
    ("CompileTimeErrorCode.CONST_WITH_NON_CONSTANT_ARGUMENT", 110),
    ("CompileTimeErrorCode.CONST_WITH_NON_TYPE", 111),
    ("CompileTimeErrorCode.CONST_WITH_TYPE_PARAMETERS", 112),
    ("CompileTimeErrorCode.CONST_WITH_TYPE_PARAMETERS_CONSTRUCTOR_TEAROFF", 113),
    ("CompileTimeErrorCode.CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF", 114),
    ("CompileTimeErrorCode.CONST_WITH_UNDEFINED_CONSTRUCTOR", 115),
    ("CompileTimeErrorCode.CONST_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT", 116),
    ("CompileTimeErrorCode.CONTINUE_LABEL_INVALID", 117),
    ("CompileTimeErrorCode.COULD_NOT_INFER", 118),
    ("CompileTimeErrorCode.DEFAULT_VALUE_IN_REDIRECTING_FACTORY_CONSTRUCTOR", 119),
    ("CompileTimeErrorCode.DEFAULT_VALUE_ON_REQUIRED_PARAMETER", 120),
    ("CompileTimeErrorCode.DEFERRED_IMPORT_OF_EXTENSION", 121),
    ("CompileTimeErrorCode.DEFINITELY_UNASSIGNED_LATE_LOCAL_VARIABLE", 122),
    ("CompileTimeErrorCode.DISALLOWED_TYPE_INSTANTIATION_EXPRESSION", 123),
    ("CompileTimeErrorCode.DUPLICATE_CONSTRUCTOR_DEFAULT", 124),
    ("CompileTimeErrorCode.DUPLICATE_CONSTRUCTOR_NAME", 125),
    ("CompileTimeErrorCode.DUPLICATE_DEFINITION", 126),
    ("CompileTimeErrorCode.DUPLICATE_FIELD_FORMAL_PARAMETER", 127),
    ("CompileTimeErrorCode.DUPLICATE_FIELD_NAME", 128),
    ("CompileTimeErrorCode.DUPLICATE_NAMED_ARGUMENT", 129),
    ("CompileTimeErrorCode.DUPLICATE_PART", 130),
    ("CompileTimeErrorCode.DUPLICATE_PATTERN_ASSIGNMENT_VARIABLE", 131),
    ("CompileTimeErrorCode.DUPLICATE_PATTERN_FIELD", 132),
    ("CompileTimeErrorCode.DUPLICATE_REST_ELEMENT_IN_PATTERN", 133),
    ("CompileTimeErrorCode.DUPLICATE_VARIABLE_PATTERN", 134),
    ("CompileTimeErrorCode.EMPTY_MAP_PATTERN", 135),
    ("CompileTimeErrorCode.ENUM_CONSTANT_INVOKES_FACTORY_CONSTRUCTOR", 136),
    ("CompileTimeErrorCode.ENUM_CONSTANT_SAME_NAME_AS_ENCLOSING", 137),
    ("CompileTimeErrorCode.ENUM_INSTANTIATED_TO_BOUNDS_IS_NOT_WELL_BOUNDED", 138),
    ("CompileTimeErrorCode.ENUM_MIXIN_WITH_INSTANCE_VARIABLE", 139),
    ("CompileTimeErrorCode.ENUM_WITHOUT_CONSTANTS", 140),
    ("CompileTimeErrorCode.ENUM_WITH_ABSTRACT_MEMBER", 141),
    ("CompileTimeErrorCode.ENUM_WITH_NAME_VALUES", 142),
    ("CompileTimeErrorCode.EQUAL_ELEMENTS_IN_CONST_SET", 143),
    ("CompileTimeErrorCode.EQUAL_KEYS_IN_CONST_MAP", 144),
    ("CompileTimeErrorCode.EQUAL_KEYS_IN_MAP_PATTERN", 145),
    ("CompileTimeErrorCode.EXPECTED_ONE_LIST_PATTERN_TYPE_ARGUMENTS", 146),
    ("CompileTimeErrorCode.EXPECTED_ONE_LIST_TYPE_ARGUMENTS", 147),
    ("CompileTimeErrorCode.EXPECTED_ONE_SET_TYPE_ARGUMENTS", 148),
    ("CompileTimeErrorCode.EXPECTED_TWO_MAP_PATTERN_TYPE_ARGUMENTS", 149),
    ("CompileTimeErrorCode.EXPECTED_TWO_MAP_TYPE_ARGUMENTS", 150),
    ("CompileTimeErrorCode.EXPORT_INTERNAL_LIBRARY", 151),
    ("CompileTimeErrorCode.EXPORT_OF_NON_LIBRARY", 152),
    ("CompileTimeErrorCode.EXPRESSION_IN_MAP", 153),
    ("CompileTimeErrorCode.EXTENDS_DEFERRED_CLASS", 154),
    ("CompileTimeErrorCode.EXTENDS_DISALLOWED_CLASS", 155),
    ("CompileTimeErrorCode.EXTENDS_NON_CLASS", 156),
    ("CompileTimeErrorCode.EXTENDS_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER", 157),
    ("CompileTimeErrorCode.EXTENSION_AS_EXPRESSION", 158),
    ("CompileTimeErrorCode.EXTENSION_CONFLICTING_STATIC_AND_INSTANCE", 159),
    ("CompileTimeErrorCode.EXTENSION_DECLARES_MEMBER_OF_OBJECT", 160),
    ("CompileTimeErrorCode.EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER", 161),
    ("CompileTimeErrorCode.EXTENSION_OVERRIDE_ARGUMENT_NOT_ASSIGNABLE", 162),
    ("CompileTimeErrorCode.EXTENSION_OVERRIDE_WITHOUT_ACCESS", 163),
    ("CompileTimeErrorCode.EXTENSION_OVERRIDE_WITH_CASCADE", 164),
    ("CompileTimeErrorCode.EXTENSION_TYPE_CONSTRUCTOR_WITH_SUPER_FORMAL_PARAMETER", 165),
    ("CompileTimeErrorCode.EXTENSION_TYPE_CONSTRUCTOR_WITH_SUPER_INVOCATION", 166),
    ("CompileTimeErrorCode.EXTENSION_TYPE_DECLARES_INSTANCE_FIELD", 167),
    ("CompileTimeErrorCode.EXTENSION_TYPE_DECLARES_MEMBER_OF_OBJECT", 168),
    ("CompileTimeErrorCode.EXTENSION_TYPE_IMPLEMENTS_DISALLOWED_TYPE", 169),
    ("CompileTimeErrorCode.EXTENSION_TYPE_IMPLEMENTS_ITSELF", 170),
    ("CompileTimeErrorCode.EXTENSION_TYPE_IMPLEMENTS_NOT_SUPERTYPE", 171),
    ("CompileTimeErrorCode.EXTENSION_TYPE_IMPLEMENTS_REPRESENTATION_NOT_SUPERTYPE", 172),
    ("CompileTimeErrorCode.EXTENSION_TYPE_INHERITED_MEMBER_CONFLICT", 173),
    ("CompileTimeErrorCode.EXTENSION_TYPE_REPRESENTATION_DEPENDS_ON_ITSELF", 174),
    ("CompileTimeErrorCode.EXTENSION_TYPE_REPRESENTATION_TYPE_BOTTOM", 175),
    ("CompileTimeErrorCode.EXTENSION_TYPE_WITH_ABSTRACT_MEMBER", 176),
    ("CompileTimeErrorCode.EXTERNAL_FIELD_CONSTRUCTOR_INITIALIZER", 177),
    ("CompileTimeErrorCode.EXTERNAL_FIELD_INITIALIZER", 178),
    ("CompileTimeErrorCode.EXTERNAL_VARIABLE_INITIALIZER", 179),
    ("CompileTimeErrorCode.EXTRA_POSITIONAL_ARGUMENTS", 180),
    ("CompileTimeErrorCode.EXTRA_POSITIONAL_ARGUMENTS_COULD_BE_NAMED", 181),
    ("CompileTimeErrorCode.FIELD_INITIALIZED_BY_MULTIPLE_INITIALIZERS", 182),
    ("CompileTimeErrorCode.FIELD_INITIALIZED_IN_INITIALIZER_AND_DECLARATION", 183),
    ("CompileTimeErrorCode.FIELD_INITIALIZED_IN_PARAMETER_AND_INITIALIZER", 184),
    ("CompileTimeErrorCode.FIELD_INITIALIZER_FACTORY_CONSTRUCTOR", 185),
    ("CompileTimeErrorCode.FIELD_INITIALIZER_NOT_ASSIGNABLE", 186),
    ("CompileTimeErrorCode.FIELD_INITIALIZER_OUTSIDE_CONSTRUCTOR", 187),
    ("CompileTimeErrorCode.FIELD_INITIALIZER_REDIRECTING_CONSTRUCTOR", 188),
    ("CompileTimeErrorCode.FIELD_INITIALIZING_FORMAL_NOT_ASSIGNABLE", 189),
    ("CompileTimeErrorCode.FINAL_CLASS_EXTENDED_OUTSIDE_OF_LIBRARY", 190),
    ("CompileTimeErrorCode.FINAL_CLASS_IMPLEMENTED_OUTSIDE_OF_LIBRARY", 191),
    ("CompileTimeErrorCode.FINAL_CLASS_USED_AS_MIXIN_CONSTRAINT_OUTSIDE_OF_LIBRARY", 192),
    ("CompileTimeErrorCode.FINAL_INITIALIZED_IN_DECLARATION_AND_CONSTRUCTOR", 193),
    ("CompileTimeErrorCode.FINAL_NOT_INITIALIZED", 194),
    ("CompileTimeErrorCode.FINAL_NOT_INITIALIZED_CONSTRUCTOR_1", 195),
    ("CompileTimeErrorCode.FINAL_NOT_INITIALIZED_CONSTRUCTOR_2", 196),
    ("CompileTimeErrorCode.FINAL_NOT_INITIALIZED_CONSTRUCTOR_3_PLUS", 197),
    ("CompileTimeErrorCode.FOR_IN_OF_INVALID_ELEMENT_TYPE", 198),
    ("CompileTimeErrorCode.FOR_IN_OF_INVALID_TYPE", 199),
    ("CompileTimeErrorCode.FOR_IN_WITH_CONST_VARIABLE", 200),
    ("CompileTimeErrorCode.GENERIC_FUNCTION_TYPE_CANNOT_BE_BOUND", 201),
    ("CompileTimeErrorCode.GENERIC_FUNCTION_TYPE_CANNOT_BE_TYPE_ARGUMENT", 202),
    ("CompileTimeErrorCode.GENERIC_METHOD_TYPE_INSTANTIATION_ON_DYNAMIC", 203),
    ("CompileTimeErrorCode.GETTER_NOT_ASSIGNABLE_SETTER_TYPES", 204),
    ("CompileTimeErrorCode.GETTER_NOT_SUBTYPE_SETTER_TYPES", 205),
    ("CompileTimeErrorCode.IF_ELEMENT_CONDITION_FROM_DEFERRED_LIBRARY", 206),
    ("CompileTimeErrorCode.ILLEGAL_ASYNC_GENERATOR_RETURN_TYPE", 207),
    ("CompileTimeErrorCode.ILLEGAL_ASYNC_RETURN_TYPE", 208),
    ("CompileTimeErrorCode.ILLEGAL_CONCRETE_ENUM_MEMBER_DECLARATION", 209),
    ("CompileTimeErrorCode.ILLEGAL_CONCRETE_ENUM_MEMBER_INHERITANCE", 210),
    ("CompileTimeErrorCode.ILLEGAL_ENUM_VALUES_DECLARATION", 211),
    ("CompileTimeErrorCode.ILLEGAL_ENUM_VALUES_INHERITANCE", 212),
    ("CompileTimeErrorCode.ILLEGAL_LANGUAGE_VERSION_OVERRIDE", 213),
    ("CompileTimeErrorCode.ILLEGAL_SYNC_GENERATOR_RETURN_TYPE", 214),
    ("CompileTimeErrorCode.IMPLEMENTS_DEFERRED_CLASS", 215),
    ("CompileTimeErrorCode.IMPLEMENTS_DISALLOWED_CLASS", 216),
    ("CompileTimeErrorCode.IMPLEMENTS_NON_CLASS", 217),
    ("CompileTimeErrorCode.IMPLEMENTS_REPEATED", 218),
    ("CompileTimeErrorCode.IMPLEMENTS_SUPER_CLASS", 219),
    ("CompileTimeErrorCode.IMPLEMENTS_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER", 220),
    ("CompileTimeErrorCode.IMPLICIT_SUPER_INITIALIZER_MISSING_ARGUMENTS", 221),
    ("CompileTimeErrorCode.IMPLICIT_THIS_REFERENCE_IN_INITIALIZER", 222),
    ("CompileTimeErrorCode.IMPORT_INTERNAL_LIBRARY", 223),
    ("CompileTimeErrorCode.IMPORT_OF_NON_LIBRARY", 224),
    ("CompileTimeErrorCode.INCONSISTENT_CASE_EXPRESSION_TYPES", 225),
    ("CompileTimeErrorCode.INCONSISTENT_INHERITANCE", 226),
    ("CompileTimeErrorCode.INCONSISTENT_INHERITANCE_GETTER_AND_METHOD", 227),
    ("CompileTimeErrorCode.INCONSISTENT_LANGUAGE_VERSION_OVERRIDE", 228),
    ("CompileTimeErrorCode.INCONSISTENT_PATTERN_VARIABLE_LOGICAL_OR", 229),
    ("CompileTimeErrorCode.INITIALIZER_FOR_NON_EXISTENT_FIELD", 230),
    ("CompileTimeErrorCode.INITIALIZER_FOR_STATIC_FIELD", 231),
    ("CompileTimeErrorCode.INITIALIZING_FORMAL_FOR_NON_EXISTENT_FIELD", 232),
    ("CompileTimeErrorCode.INSTANCE_ACCESS_TO_STATIC_MEMBER", 233),
    ("CompileTimeErrorCode.INSTANCE_ACCESS_TO_STATIC_MEMBER_OF_UNNAMED_EXTENSION", 234),
    ("CompileTimeErrorCode.INSTANCE_MEMBER_ACCESS_FROM_FACTORY", 235),
    ("CompileTimeErrorCode.INSTANCE_MEMBER_ACCESS_FROM_STATIC", 236),
    ("CompileTimeErrorCode.INSTANTIATE_ABSTRACT_CLASS", 237),
    ("CompileTimeErrorCode.INSTANTIATE_ENUM", 238),
    ("CompileTimeErrorCode.INSTANTIATE_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER", 239),
    ("CompileTimeErrorCode.INTEGER_LITERAL_IMPRECISE_AS_DOUBLE", 240),
    ("CompileTimeErrorCode.INTEGER_LITERAL_OUT_OF_RANGE", 241),
    ("CompileTimeErrorCode.INTERFACE_CLASS_EXTENDED_OUTSIDE_OF_LIBRARY", 242),
    ("CompileTimeErrorCode.INVALID_ANNOTATION", 243),
    ("CompileTimeErrorCode.INVALID_ANNOTATION_CONSTANT_VALUE_FROM_DEFERRED_LIBRARY", 244),
    ("CompileTimeErrorCode.INVALID_ANNOTATION_FROM_DEFERRED_LIBRARY", 245),
    ("CompileTimeErrorCode.INVALID_ASSIGNMENT", 246),
    ("CompileTimeErrorCode.INVALID_CAST_FUNCTION", 247),
    ("CompileTimeErrorCode.INVALID_CAST_FUNCTION_EXPR", 248),
    ("CompileTimeErrorCode.INVALID_CAST_LITERAL", 249),
    ("CompileTimeErrorCode.INVALID_CAST_LITERAL_LIST", 250),
    ("CompileTimeErrorCode.INVALID_CAST_LITERAL_MAP", 251),
    ("CompileTimeErrorCode.INVALID_CAST_LITERAL_SET", 252),
    ("CompileTimeErrorCode.INVALID_CAST_METHOD", 253),
    ("CompileTimeErrorCode.INVALID_CAST_NEW_EXPR", 254),
    ("CompileTimeErrorCode.INVALID_CONSTANT", 255),
    ("CompileTimeErrorCode.INVALID_EXTENSION_ARGUMENT_COUNT", 256),
    ("CompileTimeErrorCode.INVALID_FACTORY_NAME_NOT_A_CLASS", 257),
    ("CompileTimeErrorCode.INVALID_FIELD_NAME_FROM_OBJECT", 258),
    ("CompileTimeErrorCode.INVALID_FIELD_NAME_POSITIONAL", 259),
    ("CompileTimeErrorCode.INVALID_FIELD_NAME_PRIVATE", 260),
    ("CompileTimeErrorCode.INVALID_IMPLEMENTATION_OVERRIDE", 261),
    ("CompileTimeErrorCode.INVALID_IMPLEMENTATION_OVERRIDE_SETTER", 262),
    ("CompileTimeErrorCode.INVALID_INLINE_FUNCTION_TYPE", 263),
    ("CompileTimeErrorCode.INVALID_MACRO_APPLICATION_TARGET", 264),
    ("CompileTimeErrorCode.INVALID_MODIFIER_ON_CONSTRUCTOR", 265),
    ("CompileTimeErrorCode.INVALID_MODIFIER_ON_SETTER", 266),
    ("CompileTimeErrorCode.INVALID_OVERRIDE", 267),
    ("CompileTimeErrorCode.INVALID_OVERRIDE_SETTER", 268),
    ("CompileTimeErrorCode.INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR", 269),
    ("CompileTimeErrorCode.INVALID_REFERENCE_TO_THIS", 270),
    ("CompileTimeErrorCode.INVALID_SUPER_FORMAL_PARAMETER_LOCATION", 271),
    ("CompileTimeErrorCode.INVALID_TYPE_ARGUMENT_IN_CONST_LIST", 272),
    ("CompileTimeErrorCode.INVALID_TYPE_ARGUMENT_IN_CONST_MAP", 273),
    ("CompileTimeErrorCode.INVALID_TYPE_ARGUMENT_IN_CONST_SET", 274),
    ("CompileTimeErrorCode.INVALID_URI", 275),
    ("CompileTimeErrorCode.INVALID_USE_OF_COVARIANT", 276),
    ("CompileTimeErrorCode.INVALID_USE_OF_NULL_VALUE", 277),
    ("CompileTimeErrorCode.INVOCATION_OF_EXTENSION_WITHOUT_CALL", 278),
    ("CompileTimeErrorCode.INVOCATION_OF_NON_FUNCTION", 279),
    ("CompileTimeErrorCode.INVOCATION_OF_NON_FUNCTION_EXPRESSION", 280),
    ("CompileTimeErrorCode.LABEL_IN_OUTER_SCOPE", 281),
    ("CompileTimeErrorCode.LABEL_UNDEFINED", 282),
    ("CompileTimeErrorCode.LATE_FINAL_FIELD_WITH_CONST_CONSTRUCTOR", 283),
    ("CompileTimeErrorCode.LATE_FINAL_LOCAL_ALREADY_ASSIGNED", 284),
    ("CompileTimeErrorCode.LIST_ELEMENT_TYPE_NOT_ASSIGNABLE", 285),
    ("CompileTimeErrorCode.MACRO_APPLICATION_ARGUMENT_ERROR", 286),
    ("CompileTimeErrorCode.MACRO_DECLARATIONS_PHASE_INTROSPECTION_CYCLE", 287),
    ("CompileTimeErrorCode.MACRO_DEFINITION_APPLICATION_SAME_LIBRARY_CYCLE", 288),
    ("CompileTimeErrorCode.MACRO_ERROR", 289),
    ("CompileTimeErrorCode.MACRO_INTERNAL_EXCEPTION", 290),
    ("CompileTimeErrorCode.MACRO_NOT_ALLOWED_DECLARATION", 291),
    ("CompileTimeErrorCode.MAIN_FIRST_POSITIONAL_PARAMETER_TYPE", 292),
    ("CompileTimeErrorCode.MAIN_HAS_REQUIRED_NAMED_PARAMETERS", 293),
    ("CompileTimeErrorCode.MAIN_HAS_TOO_MANY_REQUIRED_POSITIONAL_PARAMETERS", 294),
    ("CompileTimeErrorCode.MAIN_IS_NOT_FUNCTION", 295),
    ("CompileTimeErrorCode.MAP_ENTRY_NOT_IN_MAP", 296),
    ("CompileTimeErrorCode.MAP_KEY_TYPE_NOT_ASSIGNABLE", 297),
    ("CompileTimeErrorCode.MAP_VALUE_TYPE_NOT_ASSIGNABLE", 298),
    ("CompileTimeErrorCode.MISSING_CONST_IN_LIST_LITERAL", 299),
    ("CompileTimeErrorCode.MISSING_CONST_IN_MAP_LITERAL", 300),
    ("CompileTimeErrorCode.MISSING_CONST_IN_SET_LITERAL", 301),
    ("CompileTimeErrorCode.MISSING_DART_LIBRARY", 302),
    ("CompileTimeErrorCode.MISSING_DEFAULT_VALUE_FOR_PARAMETER", 303),
    ("CompileTimeErrorCode.MISSING_DEFAULT_VALUE_FOR_PARAMETER_POSITIONAL", 304),
    ("CompileTimeErrorCode.MISSING_DEFAULT_VALUE_FOR_PARAMETER_WITH_ANNOTATION", 305),
    ("CompileTimeErrorCode.MISSING_NAMED_PATTERN_FIELD_NAME", 306),
    ("CompileTimeErrorCode.MISSING_REQUIRED_ARGUMENT", 307),
    ("CompileTimeErrorCode.MISSING_VARIABLE_PATTERN", 308),
    ("CompileTimeErrorCode.MIXINS_SUPER_CLASS", 309),
    ("CompileTimeErrorCode.MIXIN_APPLICATION_CONCRETE_SUPER_INVOKED_MEMBER_TYPE", 310),
    ("CompileTimeErrorCode.MIXIN_APPLICATION_NOT_IMPLEMENTED_INTERFACE", 311),
    ("CompileTimeErrorCode.MIXIN_APPLICATION_NO_CONCRETE_SUPER_INVOKED_MEMBER", 312),
    ("CompileTimeErrorCode.MIXIN_APPLICATION_NO_CONCRETE_SUPER_INVOKED_SETTER", 313),
    ("CompileTimeErrorCode.MIXIN_CLASS_DECLARATION_EXTENDS_NOT_OBJECT", 314),
    ("CompileTimeErrorCode.MIXIN_CLASS_DECLARES_CONSTRUCTOR", 315),
    ("CompileTimeErrorCode.MIXIN_DEFERRED_CLASS", 316),
    ("CompileTimeErrorCode.MIXIN_INHERITS_FROM_NOT_OBJECT", 317),
    ("CompileTimeErrorCode.MIXIN_INSTANTIATE", 318),
    ("CompileTimeErrorCode.MIXIN_OF_DISALLOWED_CLASS", 319),
    ("CompileTimeErrorCode.MIXIN_OF_NON_CLASS", 320),
    ("CompileTimeErrorCode.MIXIN_OF_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER", 321),
    ("CompileTimeErrorCode.MIXIN_ON_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER", 322),
    ("CompileTimeErrorCode.MIXIN_SUBTYPE_OF_BASE_IS_NOT_BASE", 323),
    ("CompileTimeErrorCode.MIXIN_SUBTYPE_OF_FINAL_IS_NOT_BASE", 324),
    ("CompileTimeErrorCode.MIXIN_SUPER_CLASS_CONSTRAINT_DEFERRED_CLASS", 325),
    ("CompileTimeErrorCode.MIXIN_SUPER_CLASS_CONSTRAINT_DISALLOWED_CLASS", 326),
    ("CompileTimeErrorCode.MIXIN_SUPER_CLASS_CONSTRAINT_NON_INTERFACE", 327),
    ("CompileTimeErrorCode.MIXIN_WITH_NON_CLASS_SUPERCLASS", 328),
    ("CompileTimeErrorCode.MULTIPLE_REDIRECTING_CONSTRUCTOR_INVOCATIONS", 329),
    ("CompileTimeErrorCode.MULTIPLE_SUPER_INITIALIZERS", 330),
    ("CompileTimeErrorCode.NEW_WITH_NON_TYPE", 331),
    ("CompileTimeErrorCode.NEW_WITH_UNDEFINED_CONSTRUCTOR", 332),
    ("CompileTimeErrorCode.NEW_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT", 333),
    ("CompileTimeErrorCode.NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_FIVE_PLUS", 334),
    ("CompileTimeErrorCode.NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_FOUR", 335),
    ("CompileTimeErrorCode.NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_ONE", 336),
    ("CompileTimeErrorCode.NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_THREE", 337),
    ("CompileTimeErrorCode.NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_TWO", 338),
    ("CompileTimeErrorCode.NON_BOOL_CONDITION", 339),
    ("CompileTimeErrorCode.NON_BOOL_EXPRESSION", 340),
    ("CompileTimeErrorCode.NON_BOOL_NEGATION_EXPRESSION", 341),
    ("CompileTimeErrorCode.NON_BOOL_OPERAND", 342),
    ("CompileTimeErrorCode.NON_CONSTANT_ANNOTATION_CONSTRUCTOR", 343),
    ("CompileTimeErrorCode.NON_CONSTANT_CASE_EXPRESSION", 344),
    ("CompileTimeErrorCode.NON_CONSTANT_CASE_EXPRESSION_FROM_DEFERRED_LIBRARY", 345),
    ("CompileTimeErrorCode.NON_CONSTANT_DEFAULT_VALUE", 346),
    ("CompileTimeErrorCode.NON_CONSTANT_DEFAULT_VALUE_FROM_DEFERRED_LIBRARY", 347),
    ("CompileTimeErrorCode.NON_CONSTANT_LIST_ELEMENT", 348),
    ("CompileTimeErrorCode.NON_CONSTANT_LIST_ELEMENT_FROM_DEFERRED_LIBRARY", 349),
    ("CompileTimeErrorCode.NON_CONSTANT_MAP_ELEMENT", 350),
    ("CompileTimeErrorCode.NON_CONSTANT_MAP_KEY", 351),
    ("CompileTimeErrorCode.NON_CONSTANT_MAP_KEY_FROM_DEFERRED_LIBRARY", 352),
    ("CompileTimeErrorCode.NON_CONSTANT_MAP_PATTERN_KEY", 353),
    ("CompileTimeErrorCode.NON_CONSTANT_MAP_VALUE", 354),
    ("CompileTimeErrorCode.NON_CONSTANT_MAP_VALUE_FROM_DEFERRED_LIBRARY", 355),
    ("CompileTimeErrorCode.NON_CONSTANT_RECORD_FIELD", 356),
    ("CompileTimeErrorCode.NON_CONSTANT_RECORD_FIELD_FROM_DEFERRED_LIBRARY", 357),
    ("CompileTimeErrorCode.NON_CONSTANT_RELATIONAL_PATTERN_EXPRESSION", 358),
    ("CompileTimeErrorCode.NON_CONSTANT_SET_ELEMENT", 359),
    ("CompileTimeErrorCode.NON_CONST_GENERATIVE_ENUM_CONSTRUCTOR", 360),
    ("CompileTimeErrorCode.NON_CONST_MAP_AS_EXPRESSION_STATEMENT", 361),
    ("CompileTimeErrorCode.NON_COVARIANT_TYPE_PARAMETER_POSITION_IN_REPRESENTATION_TYPE", 362),
    ("CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_EXPRESSION", 363),
    ("CompileTimeErrorCode.NON_EXHAUSTIVE_SWITCH_STATEMENT", 364),
    ("CompileTimeErrorCode.NON_FINAL_FIELD_IN_ENUM", 365),
    ("CompileTimeErrorCode.NON_GENERATIVE_CONSTRUCTOR", 366),
    ("CompileTimeErrorCode.NON_GENERATIVE_IMPLICIT_CONSTRUCTOR", 367),
    ("CompileTimeErrorCode.NON_SYNC_FACTORY", 368),
    ("CompileTimeErrorCode.NON_TYPE_AS_TYPE_ARGUMENT", 369),
    ("CompileTimeErrorCode.NON_TYPE_IN_CATCH_CLAUSE", 370),
    ("CompileTimeErrorCode.NON_VOID_RETURN_FOR_OPERATOR", 371),
    ("CompileTimeErrorCode.NON_VOID_RETURN_FOR_SETTER", 372),
    ("CompileTimeErrorCode.NOT_ASSIGNED_POTENTIALLY_NON_NULLABLE_LOCAL_VARIABLE", 373),
    ("CompileTimeErrorCode.NOT_A_TYPE", 374),
    ("CompileTimeErrorCode.NOT_BINARY_OPERATOR", 375),
    ("CompileTimeErrorCode.NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_PLURAL", 376),
    ("CompileTimeErrorCode.NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_SINGULAR", 377),
    ("CompileTimeErrorCode.NOT_ENOUGH_POSITIONAL_ARGUMENTS_PLURAL", 378),
    ("CompileTimeErrorCode.NOT_ENOUGH_POSITIONAL_ARGUMENTS_SINGULAR", 379),
    ("CompileTimeErrorCode.NOT_INITIALIZED_NON_NULLABLE_INSTANCE_FIELD", 380),
    ("CompileTimeErrorCode.NOT_INITIALIZED_NON_NULLABLE_INSTANCE_FIELD_CONSTRUCTOR", 381),
    ("CompileTimeErrorCode.NOT_INITIALIZED_NON_NULLABLE_VARIABLE", 382),
    ("CompileTimeErrorCode.NOT_INSTANTIATED_BOUND", 383),
    ("CompileTimeErrorCode.NOT_ITERABLE_SPREAD", 384),
    ("CompileTimeErrorCode.NOT_MAP_SPREAD", 385),
    ("CompileTimeErrorCode.NOT_NULL_AWARE_NULL_SPREAD", 386),
    ("CompileTimeErrorCode.NO_ANNOTATION_CONSTRUCTOR_ARGUMENTS", 387),
    ("CompileTimeErrorCode.NO_COMBINED_SUPER_SIGNATURE", 388),
    ("CompileTimeErrorCode.NO_DEFAULT_SUPER_CONSTRUCTOR_EXPLICIT", 389),
    ("CompileTimeErrorCode.NO_DEFAULT_SUPER_CONSTRUCTOR_IMPLICIT", 390),
    ("CompileTimeErrorCode.NO_GENERATIVE_CONSTRUCTORS_IN_SUPERCLASS", 391),
    ("CompileTimeErrorCode.NULLABLE_TYPE_IN_EXTENDS_CLAUSE", 392),
    ("CompileTimeErrorCode.NULLABLE_TYPE_IN_IMPLEMENTS_CLAUSE", 393),
    ("CompileTimeErrorCode.NULLABLE_TYPE_IN_ON_CLAUSE", 394),
    ("CompileTimeErrorCode.NULLABLE_TYPE_IN_WITH_CLAUSE", 395),
    ("CompileTimeErrorCode.OBJECT_CANNOT_EXTEND_ANOTHER_CLASS", 396),
    ("CompileTimeErrorCode.OBSOLETE_COLON_FOR_DEFAULT_VALUE", 397),
    ("CompileTimeErrorCode.ON_REPEATED", 398),
    ("CompileTimeErrorCode.OPTIONAL_PARAMETER_IN_OPERATOR", 399),
    ("CompileTimeErrorCode.PART_OF_DIFFERENT_LIBRARY", 400),
    ("CompileTimeErrorCode.PART_OF_NON_PART", 401),
    ("CompileTimeErrorCode.PART_OF_UNNAMED_LIBRARY", 402),
    ("CompileTimeErrorCode.PATTERN_ASSIGNMENT_NOT_LOCAL_VARIABLE", 403),
    ("CompileTimeErrorCode.PATTERN_CONSTANT_FROM_DEFERRED_LIBRARY", 404),
    ("CompileTimeErrorCode.PATTERN_TYPE_MISMATCH_IN_IRREFUTABLE_CONTEXT", 405),
    ("CompileTimeErrorCode.PATTERN_VARIABLE_ASSIGNMENT_INSIDE_GUARD", 406),
    ("CompileTimeErrorCode.PATTERN_VARIABLE_SHARED_CASE_SCOPE_DIFFERENT_FINALITY_OR_TYPE", 407),
    ("CompileTimeErrorCode.PATTERN_VARIABLE_SHARED_CASE_SCOPE_HAS_LABEL", 408),
    ("CompileTimeErrorCode.PATTERN_VARIABLE_SHARED_CASE_SCOPE_NOT_ALL_CASES", 409),
    ("CompileTimeErrorCode.POSITIONAL_FIELD_IN_OBJECT_PATTERN", 410),
    ("CompileTimeErrorCode.POSITIONAL_SUPER_FORMAL_PARAMETER_WITH_POSITIONAL_ARGUMENT", 411),
    ("CompileTimeErrorCode.PREFIX_COLLIDES_WITH_TOP_LEVEL_MEMBER", 412),
    ("CompileTimeErrorCode.PREFIX_IDENTIFIER_NOT_FOLLOWED_BY_DOT", 413),
    ("CompileTimeErrorCode.PREFIX_SHADOWED_BY_LOCAL_DECLARATION", 414),
    ("CompileTimeErrorCode.PRIVATE_COLLISION_IN_MIXIN_APPLICATION", 415),
    ("CompileTimeErrorCode.PRIVATE_OPTIONAL_PARAMETER", 416),
    ("CompileTimeErrorCode.PRIVATE_SETTER", 417),
    ("CompileTimeErrorCode.READ_POTENTIALLY_UNASSIGNED_FINAL", 418),
    ("CompileTimeErrorCode.RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA", 419),
    ("CompileTimeErrorCode.RECURSIVE_COMPILE_TIME_CONSTANT", 420),
    ("CompileTimeErrorCode.RECURSIVE_CONSTANT_CONSTRUCTOR", 421),
    ("CompileTimeErrorCode.RECURSIVE_CONSTRUCTOR_REDIRECT", 422),
    ("CompileTimeErrorCode.RECURSIVE_FACTORY_REDIRECT", 423),
    ("CompileTimeErrorCode.RECURSIVE_INTERFACE_INHERITANCE", 424),
    ("CompileTimeErrorCode.RECURSIVE_INTERFACE_INHERITANCE_EXTENDS", 425),
    ("CompileTimeErrorCode.RECURSIVE_INTERFACE_INHERITANCE_IMPLEMENTS", 426),
    ("CompileTimeErrorCode.RECURSIVE_INTERFACE_INHERITANCE_ON", 427),
    ("CompileTimeErrorCode.RECURSIVE_INTERFACE_INHERITANCE_WITH", 428),
    ("CompileTimeErrorCode.REDIRECT_GENERATIVE_TO_MISSING_CONSTRUCTOR", 429),
    ("CompileTimeErrorCode.REDIRECT_GENERATIVE_TO_NON_GENERATIVE_CONSTRUCTOR", 430),
    ("CompileTimeErrorCode.REDIRECT_TO_ABSTRACT_CLASS_CONSTRUCTOR", 431),
    ("CompileTimeErrorCode.REDIRECT_TO_INVALID_FUNCTION_TYPE", 432),
    ("CompileTimeErrorCode.REDIRECT_TO_INVALID_RETURN_TYPE", 433),
    ("CompileTimeErrorCode.REDIRECT_TO_MISSING_CONSTRUCTOR", 434),
    ("CompileTimeErrorCode.REDIRECT_TO_NON_CLASS", 435),
    ("CompileTimeErrorCode.REDIRECT_TO_NON_CONST_CONSTRUCTOR", 436),
    ("CompileTimeErrorCode.REDIRECT_TO_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER", 437),
    ("CompileTimeErrorCode.REFERENCED_BEFORE_DECLARATION", 438),
    ("CompileTimeErrorCode.REFUTABLE_PATTERN_IN_IRREFUTABLE_CONTEXT", 439),
    ("CompileTimeErrorCode.RELATIONAL_PATTERN_OPERAND_TYPE_NOT_ASSIGNABLE", 440),
    ("CompileTimeErrorCode.RELATIONAL_PATTERN_OPERATOR_RETURN_TYPE_NOT_ASSIGNABLE_TO_BOOL", 441),
    ("CompileTimeErrorCode.REST_ELEMENT_IN_MAP_PATTERN", 442),
    ("CompileTimeErrorCode.RETHROW_OUTSIDE_CATCH", 443),
    ("CompileTimeErrorCode.RETURN_IN_GENERATIVE_CONSTRUCTOR", 444),
    ("CompileTimeErrorCode.RETURN_IN_GENERATOR", 445),
    ("CompileTimeErrorCode.RETURN_OF_INVALID_TYPE_FROM_CLOSURE", 446),
    ("CompileTimeErrorCode.RETURN_OF_INVALID_TYPE_FROM_CONSTRUCTOR", 447),
    ("CompileTimeErrorCode.RETURN_OF_INVALID_TYPE_FROM_FUNCTION", 448),
    ("CompileTimeErrorCode.RETURN_OF_INVALID_TYPE_FROM_METHOD", 449),
    ("CompileTimeErrorCode.RETURN_WITHOUT_VALUE", 450),
    ("CompileTimeErrorCode.SEALED_CLASS_SUBTYPE_OUTSIDE_OF_LIBRARY", 451),
    ("CompileTimeErrorCode.SET_ELEMENT_FROM_DEFERRED_LIBRARY", 452),
    ("CompileTimeErrorCode.SET_ELEMENT_TYPE_NOT_ASSIGNABLE", 453),
    ("CompileTimeErrorCode.SHARED_DEFERRED_PREFIX", 454),
    ("CompileTimeErrorCode.SPREAD_EXPRESSION_FROM_DEFERRED_LIBRARY", 455),
    ("CompileTimeErrorCode.STATIC_ACCESS_TO_INSTANCE_MEMBER", 456),
    ("CompileTimeErrorCode.SUBTYPE_OF_BASE_IS_NOT_BASE_FINAL_OR_SEALED", 457),
    ("CompileTimeErrorCode.SUBTYPE_OF_FINAL_IS_NOT_BASE_FINAL_OR_SEALED", 458),
    ("CompileTimeErrorCode.SUPER_FORMAL_PARAMETER_TYPE_IS_NOT_SUBTYPE_OF_ASSOCIATED", 459),
    ("CompileTimeErrorCode.SUPER_FORMAL_PARAMETER_WITHOUT_ASSOCIATED_NAMED", 460),
    ("CompileTimeErrorCode.SUPER_FORMAL_PARAMETER_WITHOUT_ASSOCIATED_POSITIONAL", 461),
    ("CompileTimeErrorCode.SUPER_INITIALIZER_IN_OBJECT", 462),
    ("CompileTimeErrorCode.SUPER_INVOCATION_NOT_LAST", 463),
    ("CompileTimeErrorCode.SUPER_IN_ENUM_CONSTRUCTOR", 464),
    ("CompileTimeErrorCode.SUPER_IN_EXTENSION", 465),
    ("CompileTimeErrorCode.SUPER_IN_EXTENSION_TYPE", 466),
    ("CompileTimeErrorCode.SUPER_IN_INVALID_CONTEXT", 467),
    ("CompileTimeErrorCode.SUPER_IN_REDIRECTING_CONSTRUCTOR", 468),
    ("CompileTimeErrorCode.SWITCH_CASE_COMPLETES_NORMALLY", 469),
    ("CompileTimeErrorCode.TEAROFF_OF_GENERATIVE_CONSTRUCTOR_OF_ABSTRACT_CLASS", 470),
    ("CompileTimeErrorCode.THROW_OF_INVALID_TYPE", 471),
    ("CompileTimeErrorCode.TOP_LEVEL_CYCLE", 472),
    ("CompileTimeErrorCode.TYPE_ALIAS_CANNOT_REFERENCE_ITSELF", 473),
    ("CompileTimeErrorCode.TYPE_ANNOTATION_DEFERRED_CLASS", 474),
    ("CompileTimeErrorCode.TYPE_ARGUMENT_NOT_MATCHING_BOUNDS", 475),
    ("CompileTimeErrorCode.TYPE_PARAMETER_REFERENCED_BY_STATIC", 476),
    ("CompileTimeErrorCode.TYPE_PARAMETER_SUPERTYPE_OF_ITS_BOUND", 477),
    ("CompileTimeErrorCode.TYPE_TEST_WITH_NON_TYPE", 478),
    ("CompileTimeErrorCode.TYPE_TEST_WITH_UNDEFINED_NAME", 479),
    ("CompileTimeErrorCode.UNCHECKED_INVOCATION_OF_NULLABLE_VALUE", 480),
    ("CompileTimeErrorCode.UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE", 481),
    ("CompileTimeErrorCode.UNCHECKED_OPERATOR_INVOCATION_OF_NULLABLE_VALUE", 482),
    ("CompileTimeErrorCode.UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE", 483),
    ("CompileTimeErrorCode.UNCHECKED_USE_OF_NULLABLE_VALUE_AS_CONDITION", 484),
    ("CompileTimeErrorCode.UNCHECKED_USE_OF_NULLABLE_VALUE_AS_ITERATOR", 485),
    ("CompileTimeErrorCode.UNCHECKED_USE_OF_NULLABLE_VALUE_IN_SPREAD", 486),
    ("CompileTimeErrorCode.UNCHECKED_USE_OF_NULLABLE_VALUE_IN_YIELD_EACH", 487),
    ("CompileTimeErrorCode.UNDEFINED_ANNOTATION", 488),
    ("CompileTimeErrorCode.UNDEFINED_CLASS", 489),
    ("CompileTimeErrorCode.UNDEFINED_CLASS_BOOLEAN", 490),
    ("CompileTimeErrorCode.UNDEFINED_CONSTRUCTOR_IN_INITIALIZER", 491),
    ("CompileTimeErrorCode.UNDEFINED_CONSTRUCTOR_IN_INITIALIZER_DEFAULT", 492),
    ("CompileTimeErrorCode.UNDEFINED_ENUM_CONSTANT", 493),
    ("CompileTimeErrorCode.UNDEFINED_ENUM_CONSTRUCTOR_NAMED", 494),
    ("CompileTimeErrorCode.UNDEFINED_ENUM_CONSTRUCTOR_UNNAMED", 495),
    ("CompileTimeErrorCode.UNDEFINED_EXTENSION_GETTER", 496),
    ("CompileTimeErrorCode.UNDEFINED_EXTENSION_METHOD", 497),
    ("CompileTimeErrorCode.UNDEFINED_EXTENSION_OPERATOR", 498),
    ("CompileTimeErrorCode.UNDEFINED_EXTENSION_SETTER", 499),
    ("CompileTimeErrorCode.UNDEFINED_FUNCTION", 500),
    ("CompileTimeErrorCode.UNDEFINED_GETTER", 501),
    ("CompileTimeErrorCode.UNDEFINED_GETTER_ON_FUNCTION_TYPE", 502),
    ("CompileTimeErrorCode.UNDEFINED_IDENTIFIER", 503),
    ("CompileTimeErrorCode.UNDEFINED_IDENTIFIER_AWAIT", 504),
    ("CompileTimeErrorCode.UNDEFINED_METHOD", 505),
    ("CompileTimeErrorCode.UNDEFINED_METHOD_ON_FUNCTION_TYPE", 506),
    ("CompileTimeErrorCode.UNDEFINED_NAMED_PARAMETER", 507),
    ("CompileTimeErrorCode.UNDEFINED_OPERATOR", 508),
    ("CompileTimeErrorCode.UNDEFINED_PREFIXED_NAME", 509),
    ("CompileTimeErrorCode.UNDEFINED_SETTER", 510),
    ("CompileTimeErrorCode.UNDEFINED_SETTER_ON_FUNCTION_TYPE", 511),
    ("CompileTimeErrorCode.UNDEFINED_SUPER_GETTER", 512),
    ("CompileTimeErrorCode.UNDEFINED_SUPER_METHOD", 513),
    ("CompileTimeErrorCode.UNDEFINED_SUPER_OPERATOR", 514),
    ("CompileTimeErrorCode.UNDEFINED_SUPER_SETTER", 515),
    ("CompileTimeErrorCode.UNQUALIFIED_REFERENCE_TO_NON_LOCAL_STATIC_MEMBER", 516),
    ("CompileTimeErrorCode.UNQUALIFIED_REFERENCE_TO_STATIC_MEMBER_OF_EXTENDED_TYPE", 517),
    ("CompileTimeErrorCode.URI_DOES_NOT_EXIST", 518),
    ("CompileTimeErrorCode.URI_HAS_NOT_BEEN_GENERATED", 519),
    ("CompileTimeErrorCode.URI_WITH_INTERPOLATION", 520),
    ("CompileTimeErrorCode.USE_OF_NATIVE_EXTENSION", 521),
    ("CompileTimeErrorCode.USE_OF_VOID_RESULT", 522),
    ("CompileTimeErrorCode.VALUES_DECLARATION_IN_ENUM", 523),
    ("CompileTimeErrorCode.VARIABLE_TYPE_MISMATCH", 524),
    ("CompileTimeErrorCode.WRONG_EXPLICIT_TYPE_PARAMETER_VARIANCE_IN_SUPERINTERFACE", 525),
    ("CompileTimeErrorCode.WRONG_NUMBER_OF_PARAMETERS_FOR_OPERATOR", 526),
    ("CompileTimeErrorCode.WRONG_NUMBER_OF_PARAMETERS_FOR_OPERATOR_MINUS", 527),
    ("CompileTimeErrorCode.WRONG_NUMBER_OF_PARAMETERS_FOR_SETTER", 528),
    ("CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS", 529),
    ("CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_ANONYMOUS_FUNCTION", 530),
    ("CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_CONSTRUCTOR", 531),
    ("CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_ENUM", 532),
    ("CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_EXTENSION", 533),
    ("CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION", 534),
    ("CompileTimeErrorCode.WRONG_NUMBER_OF_TYPE_ARGUMENTS_METHOD", 535),
    ("CompileTimeErrorCode.WRONG_TYPE_PARAMETER_VARIANCE_IN_SUPERINTERFACE", 536),
    ("CompileTimeErrorCode.WRONG_TYPE_PARAMETER_VARIANCE_POSITION", 537),
    ("CompileTimeErrorCode.YIELD_EACH_IN_NON_GENERATOR", 538),
    ("CompileTimeErrorCode.YIELD_EACH_OF_INVALID_TYPE", 539),
    ("CompileTimeErrorCode.YIELD_IN_NON_GENERATOR", 540),
    ("CompileTimeErrorCode.YIELD_OF_INVALID_TYPE", 541),
    ("FfiCode.ABI_SPECIFIC_INTEGER_INVALID", 701),
    ("FfiCode.ABI_SPECIFIC_INTEGER_MAPPING_EXTRA", 702),
    ("FfiCode.ABI_SPECIFIC_INTEGER_MAPPING_MISSING", 703),
    ("FfiCode.ABI_SPECIFIC_INTEGER_MAPPING_UNSUPPORTED", 704),
    ("FfiCode.ADDRESS_POSITION", 705),
    ("FfiCode.ADDRESS_RECEIVER", 706),
    ("FfiCode.ANNOTATION_ON_POINTER_FIELD", 707),
    ("FfiCode.ARGUMENT_MUST_BE_A_CONSTANT", 708),
    ("FfiCode.ARGUMENT_MUST_BE_NATIVE", 709),
    ("FfiCode.COMPOUND_IMPLEMENTS_FINALIZABLE", 710),
    ("FfiCode.CREATION_OF_STRUCT_OR_UNION", 711),
    ("FfiCode.EMPTY_STRUCT", 712),
    ("FfiCode.EXTRA_ANNOTATION_ON_STRUCT_FIELD", 713),
    ("FfiCode.EXTRA_SIZE_ANNOTATION_CARRAY", 714),
    ("FfiCode.FFI_NATIVE_INVALID_DUPLICATE_DEFAULT_ASSET", 715),
    ("FfiCode.FFI_NATIVE_INVALID_MULTIPLE_ANNOTATIONS", 716),
    ("FfiCode.FFI_NATIVE_MUST_BE_EXTERNAL", 717),
    ("FfiCode.FFI_NATIVE_ONLY_CLASSES_EXTENDING_NATIVEFIELDWRAPPERCLASS1_CAN_BE_POINTER", 718),
    ("FfiCode.FFI_NATIVE_UNEXPECTED_NUMBER_OF_PARAMETERS", 719),
    ("FfiCode.FFI_NATIVE_UNEXPECTED_NUMBER_OF_PARAMETERS_WITH_RECEIVER", 720),
    ("FfiCode.FIELD_MUST_BE_EXTERNAL_IN_STRUCT", 721),
    ("FfiCode.GENERIC_STRUCT_SUBCLASS", 722),
    ("FfiCode.INVALID_EXCEPTION_VALUE", 723),
    ("FfiCode.INVALID_FIELD_TYPE_IN_STRUCT", 724),
    ("FfiCode.LEAF_CALL_MUST_NOT_RETURN_HANDLE", 725),
    ("FfiCode.LEAF_CALL_MUST_NOT_TAKE_HANDLE", 726),
    ("FfiCode.MISMATCHED_ANNOTATION_ON_STRUCT_FIELD", 727),
    ("FfiCode.MISSING_ANNOTATION_ON_STRUCT_FIELD", 728),
    ("FfiCode.MISSING_EXCEPTION_VALUE", 729),
    ("FfiCode.MISSING_FIELD_TYPE_IN_STRUCT", 730),
    ("FfiCode.MISSING_SIZE_ANNOTATION_CARRAY", 731),
    ("FfiCode.MUST_BE_A_NATIVE_FUNCTION_TYPE", 732),
    ("FfiCode.MUST_BE_A_SUBTYPE", 733),
    ("FfiCode.MUST_RETURN_VOID", 734),
    ("FfiCode.NATIVE_FIELD_INVALID_TYPE", 735),
    ("FfiCode.NATIVE_FIELD_MISSING_TYPE", 736),
    ("FfiCode.NATIVE_FIELD_NOT_STATIC", 737),
    ("FfiCode.NON_CONSTANT_TYPE_ARGUMENT", 738),
    ("FfiCode.NON_NATIVE_FUNCTION_TYPE_ARGUMENT_TO_POINTER", 739),
    ("FfiCode.NON_POSITIVE_ARRAY_DIMENSION", 740),
    ("FfiCode.NON_SIZED_TYPE_ARGUMENT", 741),
    ("FfiCode.PACKED_ANNOTATION", 742),
    ("FfiCode.PACKED_ANNOTATION_ALIGNMENT", 743),
    ("FfiCode.SIZE_ANNOTATION_DIMENSIONS", 744),
    ("FfiCode.SUBTYPE_OF_STRUCT_CLASS_IN_EXTENDS", 745),
    ("FfiCode.SUBTYPE_OF_STRUCT_CLASS_IN_IMPLEMENTS", 746),
    ("FfiCode.SUBTYPE_OF_STRUCT_CLASS_IN_WITH", 747),
    ("FfiCode.VARIABLE_LENGTH_ARRAY_NOT_LAST", 748),
    ("HintCode.DEPRECATED_COLON_FOR_DEFAULT_VALUE", 693),
    ("HintCode.DEPRECATED_MEMBER_USE", 694),
    ("HintCode.DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE", 695),
    ("HintCode.DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE_WITH_MESSAGE", 696),
    ("HintCode.DEPRECATED_MEMBER_USE_WITH_MESSAGE", 697),
    ("HintCode.IMPORT_DEFERRED_LIBRARY_WITH_LOAD_FUNCTION", 698),
    ("HintCode.MACRO_INFO", 699),
    ("HintCode.UNNECESSARY_IMPORT", 700),
    ("ParserErrorCode.ABSTRACT_CLASS_MEMBER", 749),
    ("ParserErrorCode.ABSTRACT_EXTERNAL_FIELD", 750),
    ("ParserErrorCode.ABSTRACT_FINAL_BASE_CLASS", 751),
    ("ParserErrorCode.ABSTRACT_FINAL_INTERFACE_CLASS", 752),
    ("ParserErrorCode.ABSTRACT_LATE_FIELD", 753),
    ("ParserErrorCode.ABSTRACT_SEALED_CLASS", 754),
    ("ParserErrorCode.ABSTRACT_STATIC_FIELD", 755),
    ("ParserErrorCode.ABSTRACT_STATIC_METHOD", 756),
    ("ParserErrorCode.ANNOTATION_ON_TYPE_ARGUMENT", 757),
    ("ParserErrorCode.ANNOTATION_SPACE_BEFORE_PARENTHESIS", 758),
    ("ParserErrorCode.ANNOTATION_WITH_TYPE_ARGUMENTS", 759),
    ("ParserErrorCode.ANNOTATION_WITH_TYPE_ARGUMENTS_UNINSTANTIATED", 760),
    ("ParserErrorCode.ASYNC_KEYWORD_USED_AS_IDENTIFIER", 761),
    ("ParserErrorCode.BASE_ENUM", 762),
    ("ParserErrorCode.BINARY_OPERATOR_WRITTEN_OUT", 763),
    ("ParserErrorCode.BREAK_OUTSIDE_OF_LOOP", 764),
    ("ParserErrorCode.CATCH_SYNTAX", 765),
    ("ParserErrorCode.CATCH_SYNTAX_EXTRA_PARAMETERS", 766),
    ("ParserErrorCode.CLASS_IN_CLASS", 767),
    ("ParserErrorCode.COLON_IN_PLACE_OF_IN", 768),
    ("ParserErrorCode.CONFLICTING_MODIFIERS", 769),
    ("ParserErrorCode.CONSTRUCTOR_WITH_RETURN_TYPE", 770),
    ("ParserErrorCode.CONSTRUCTOR_WITH_TYPE_ARGUMENTS", 771),
    ("ParserErrorCode.CONST_AND_FINAL", 772),
    ("ParserErrorCode.CONST_CLASS", 773),
    ("ParserErrorCode.CONST_CONSTRUCTOR_WITH_BODY", 774),
    ("ParserErrorCode.CONST_FACTORY", 775),
    ("ParserErrorCode.CONST_METHOD", 776),
    ("ParserErrorCode.CONTINUE_OUTSIDE_OF_LOOP", 777),
    ("ParserErrorCode.CONTINUE_WITHOUT_LABEL_IN_CASE", 778),
    ("ParserErrorCode.COVARIANT_AND_STATIC", 779),
    ("ParserErrorCode.COVARIANT_CONSTRUCTOR", 780),
    ("ParserErrorCode.COVARIANT_MEMBER", 781),
    ("ParserErrorCode.DECLARATION_NAMED_AUGMENTED_INSIDE_AUGMENTATION", 782),
    ("ParserErrorCode.DEFAULT_IN_SWITCH_EXPRESSION", 783),
    ("ParserErrorCode.DEFAULT_VALUE_IN_FUNCTION_TYPE", 784),
    ("ParserErrorCode.DEFERRED_AFTER_PREFIX", 785),
    ("ParserErrorCode.DIRECTIVE_AFTER_DECLARATION", 786),
    ("ParserErrorCode.DUPLICATED_MODIFIER", 787),
    ("ParserErrorCode.DUPLICATE_DEFERRED", 788),
    ("ParserErrorCode.DUPLICATE_LABEL_IN_SWITCH_STATEMENT", 789),
    ("ParserErrorCode.DUPLICATE_PREFIX", 790),
    ("ParserErrorCode.EMPTY_ENUM_BODY", 791),
    ("ParserErrorCode.EMPTY_RECORD_LITERAL_WITH_COMMA", 792),
    ("ParserErrorCode.EMPTY_RECORD_TYPE_NAMED_FIELDS_LIST", 793),
    ("ParserErrorCode.EMPTY_RECORD_TYPE_WITH_COMMA", 794),
    ("ParserErrorCode.ENUM_IN_CLASS", 795),
    ("ParserErrorCode.EQUALITY_CANNOT_BE_EQUALITY_OPERAND", 796),
    ("ParserErrorCode.EXPECTED_CASE_OR_DEFAULT", 797),
    ("ParserErrorCode.EXPECTED_CATCH_CLAUSE_BODY", 798),
    ("ParserErrorCode.EXPECTED_CLASS_BODY", 799),
    ("ParserErrorCode.EXPECTED_CLASS_MEMBER", 800),
    ("ParserErrorCode.EXPECTED_ELSE_OR_COMMA", 801),
    ("ParserErrorCode.EXPECTED_EXECUTABLE", 802),
    ("ParserErrorCode.EXPECTED_EXTENSION_BODY", 803),
    ("ParserErrorCode.EXPECTED_EXTENSION_TYPE_BODY", 804),
    ("ParserErrorCode.EXPECTED_FINALLY_CLAUSE_BODY", 805),
    ("ParserErrorCode.EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD", 806),
    ("ParserErrorCode.EXPECTED_INSTEAD", 807),
    ("ParserErrorCode.EXPECTED_LIST_OR_MAP_LITERAL", 808),
    ("ParserErrorCode.EXPECTED_MIXIN_BODY", 809),
    ("ParserErrorCode.EXPECTED_NAMED_TYPE_EXTENDS", 810),
    ("ParserErrorCode.EXPECTED_NAMED_TYPE_IMPLEMENTS", 811),
    ("ParserErrorCode.EXPECTED_NAMED_TYPE_ON", 812),
    ("ParserErrorCode.EXPECTED_NAMED_TYPE_WITH", 813),
    ("ParserErrorCode.EXPECTED_REPRESENTATION_FIELD", 814),
    ("ParserErrorCode.EXPECTED_REPRESENTATION_TYPE", 815),
    ("ParserErrorCode.EXPECTED_STRING_LITERAL", 816),
    ("ParserErrorCode.EXPECTED_SWITCH_EXPRESSION_BODY", 817),
    ("ParserErrorCode.EXPECTED_SWITCH_STATEMENT_BODY", 818),
    ("ParserErrorCode.EXPECTED_TOKEN", 819),
    ("ParserErrorCode.EXPECTED_TRY_STATEMENT_BODY", 820),
    ("ParserErrorCode.EXPECTED_TYPE_NAME", 821),
    ("ParserErrorCode.EXPERIMENT_NOT_ENABLED", 822),
    ("ParserErrorCode.EXPERIMENT_NOT_ENABLED_OFF_BY_DEFAULT", 823),
    ("ParserErrorCode.EXPORT_DIRECTIVE_AFTER_PART_DIRECTIVE", 824),
    ("ParserErrorCode.EXTENSION_AUGMENTATION_HAS_ON_CLAUSE", 825),
    ("ParserErrorCode.EXTENSION_DECLARES_ABSTRACT_MEMBER", 826),
    ("ParserErrorCode.EXTENSION_DECLARES_CONSTRUCTOR", 827),
    ("ParserErrorCode.EXTENSION_DECLARES_INSTANCE_FIELD", 828),
    ("ParserErrorCode.EXTENSION_TYPE_EXTENDS", 829),
    ("ParserErrorCode.EXTENSION_TYPE_WITH", 830),
    ("ParserErrorCode.EXTERNAL_CLASS", 831),
    ("ParserErrorCode.EXTERNAL_CONSTRUCTOR_WITH_BODY", 832),
    ("ParserErrorCode.EXTERNAL_CONSTRUCTOR_WITH_FIELD_INITIALIZERS", 833),
    ("ParserErrorCode.EXTERNAL_CONSTRUCTOR_WITH_INITIALIZER", 834),
    ("ParserErrorCode.EXTERNAL_ENUM", 835),
    ("ParserErrorCode.EXTERNAL_FACTORY_REDIRECTION", 836),
    ("ParserErrorCode.EXTERNAL_FACTORY_WITH_BODY", 837),
    ("ParserErrorCode.EXTERNAL_FIELD", 838),
    ("ParserErrorCode.EXTERNAL_GETTER_WITH_BODY", 839),
    ("ParserErrorCode.EXTERNAL_LATE_FIELD", 840),
    ("ParserErrorCode.EXTERNAL_METHOD_WITH_BODY", 841),
    ("ParserErrorCode.EXTERNAL_OPERATOR_WITH_BODY", 842),
    ("ParserErrorCode.EXTERNAL_SETTER_WITH_BODY", 843),
    ("ParserErrorCode.EXTERNAL_TYPEDEF", 844),
    ("ParserErrorCode.EXTRANEOUS_MODIFIER", 845),
    ("ParserErrorCode.EXTRANEOUS_MODIFIER_IN_EXTENSION_TYPE", 846),
    ("ParserErrorCode.EXTRANEOUS_MODIFIER_IN_PRIMARY_CONSTRUCTOR", 847),
    ("ParserErrorCode.FACTORY_TOP_LEVEL_DECLARATION", 848),
    ("ParserErrorCode.FACTORY_WITHOUT_BODY", 849),
    ("ParserErrorCode.FACTORY_WITH_INITIALIZERS", 850),
    ("ParserErrorCode.FIELD_INITIALIZED_OUTSIDE_DECLARING_CLASS", 851),
    ("ParserErrorCode.FIELD_INITIALIZER_OUTSIDE_CONSTRUCTOR", 852),
    ("ParserErrorCode.FINAL_AND_COVARIANT", 853),
    ("ParserErrorCode.FINAL_AND_COVARIANT_LATE_WITH_INITIALIZER", 854),
    ("ParserErrorCode.FINAL_AND_VAR", 855),
    ("ParserErrorCode.FINAL_CONSTRUCTOR", 856),
    ("ParserErrorCode.FINAL_ENUM", 857),
    ("ParserErrorCode.FINAL_METHOD", 858),
    ("ParserErrorCode.FINAL_MIXIN", 859),
    ("ParserErrorCode.FINAL_MIXIN_CLASS", 860),
    ("ParserErrorCode.FUNCTION_TYPED_PARAMETER_VAR", 861),
    ("ParserErrorCode.GETTER_CONSTRUCTOR", 862),
    ("ParserErrorCode.GETTER_IN_FUNCTION", 863),
    ("ParserErrorCode.GETTER_WITH_PARAMETERS", 864),
    ("ParserErrorCode.ILLEGAL_ASSIGNMENT_TO_NON_ASSIGNABLE", 865),
    ("ParserErrorCode.ILLEGAL_PATTERN_ASSIGNMENT_VARIABLE_NAME", 866),
    ("ParserErrorCode.ILLEGAL_PATTERN_IDENTIFIER_NAME", 867),
    ("ParserErrorCode.ILLEGAL_PATTERN_VARIABLE_NAME", 868),
    ("ParserErrorCode.IMPLEMENTS_BEFORE_EXTENDS", 869),
    ("ParserErrorCode.IMPLEMENTS_BEFORE_ON", 870),
    ("ParserErrorCode.IMPLEMENTS_BEFORE_WITH", 871),
    ("ParserErrorCode.IMPORT_DIRECTIVE_AFTER_PART_DIRECTIVE", 872),
    ("ParserErrorCode.INITIALIZED_VARIABLE_IN_FOR_EACH", 873),
    ("ParserErrorCode.INTERFACE_ENUM", 874),
    ("ParserErrorCode.INTERFACE_MIXIN", 875),
    ("ParserErrorCode.INTERFACE_MIXIN_CLASS", 876),
    ("ParserErrorCode.INVALID_AWAIT_IN_FOR", 877),
    ("ParserErrorCode.INVALID_CODE_POINT", 878),
    ("ParserErrorCode.INVALID_COMMENT_REFERENCE", 879),
    ("ParserErrorCode.INVALID_CONSTANT_CONST_PREFIX", 880),
    ("ParserErrorCode.INVALID_CONSTANT_PATTERN_BINARY", 881),
    ("ParserErrorCode.INVALID_CONSTANT_PATTERN_DUPLICATE_CONST", 882),
    ("ParserErrorCode.INVALID_CONSTANT_PATTERN_EMPTY_RECORD_LITERAL", 883),
    ("ParserErrorCode.INVALID_CONSTANT_PATTERN_GENERIC", 884),
    ("ParserErrorCode.INVALID_CONSTANT_PATTERN_NEGATION", 885),
    ("ParserErrorCode.INVALID_CONSTANT_PATTERN_UNARY", 886),
    ("ParserErrorCode.INVALID_CONSTRUCTOR_NAME", 887),
    ("ParserErrorCode.INVALID_GENERIC_FUNCTION_TYPE", 888),
    ("ParserErrorCode.INVALID_HEX_ESCAPE", 889),
    ("ParserErrorCode.INVALID_INITIALIZER", 890),
    ("ParserErrorCode.INVALID_INSIDE_UNARY_PATTERN", 891),
    ("ParserErrorCode.INVALID_LITERAL_IN_CONFIGURATION", 892),
    ("ParserErrorCode.INVALID_OPERATOR", 893),
    ("ParserErrorCode.INVALID_OPERATOR_FOR_SUPER", 894),
    ("ParserErrorCode.INVALID_OPERATOR_QUESTIONMARK_PERIOD_FOR_SUPER", 895),
    ("ParserErrorCode.INVALID_STAR_AFTER_ASYNC", 896),
    ("ParserErrorCode.INVALID_SUPER_IN_INITIALIZER", 897),
    ("ParserErrorCode.INVALID_SYNC", 898),
    ("ParserErrorCode.INVALID_THIS_IN_INITIALIZER", 899),
    ("ParserErrorCode.INVALID_UNICODE_ESCAPE_STARTED", 900),
    ("ParserErrorCode.INVALID_UNICODE_ESCAPE_U_BRACKET", 901),
    ("ParserErrorCode.INVALID_UNICODE_ESCAPE_U_NO_BRACKET", 902),
    ("ParserErrorCode.INVALID_UNICODE_ESCAPE_U_STARTED", 903),
    ("ParserErrorCode.INVALID_USE_OF_COVARIANT_IN_EXTENSION", 904),
    ("ParserErrorCode.INVALID_USE_OF_IDENTIFIER_AUGMENTED", 905),
    ("ParserErrorCode.LATE_PATTERN_VARIABLE_DECLARATION", 906),
    ("ParserErrorCode.LIBRARY_DIRECTIVE_NOT_FIRST", 907),
    ("ParserErrorCode.LITERAL_WITH_CLASS", 908),
    ("ParserErrorCode.LITERAL_WITH_CLASS_AND_NEW", 909),
    ("ParserErrorCode.LITERAL_WITH_NEW", 910),
    ("ParserErrorCode.LOCAL_FUNCTION_DECLARATION_MODIFIER", 911),
    ("ParserErrorCode.MEMBER_WITH_CLASS_NAME", 912),
    ("ParserErrorCode.MISSING_ASSIGNABLE_SELECTOR", 913),
    ("ParserErrorCode.MISSING_ASSIGNMENT_IN_INITIALIZER", 914),
    ("ParserErrorCode.MISSING_CATCH_OR_FINALLY", 915),
    ("ParserErrorCode.MISSING_CLOSING_PARENTHESIS", 916),
    ("ParserErrorCode.MISSING_CONST_FINAL_VAR_OR_TYPE", 917),
    ("ParserErrorCode.MISSING_ENUM_BODY", 918),
    ("ParserErrorCode.MISSING_EXPRESSION_IN_INITIALIZER", 919),
    ("ParserErrorCode.MISSING_EXPRESSION_IN_THROW", 920),
    ("ParserErrorCode.MISSING_FUNCTION_BODY", 921),
    ("ParserErrorCode.MISSING_FUNCTION_KEYWORD", 922),
    ("ParserErrorCode.MISSING_FUNCTION_PARAMETERS", 923),
    ("ParserErrorCode.MISSING_GET", 924),
    ("ParserErrorCode.MISSING_IDENTIFIER", 925),
    ("ParserErrorCode.MISSING_INITIALIZER", 926),
    ("ParserErrorCode.MISSING_KEYWORD_OPERATOR", 927),
    ("ParserErrorCode.MISSING_METHOD_PARAMETERS", 928),
    ("ParserErrorCode.MISSING_NAME_FOR_NAMED_PARAMETER", 929),
    ("ParserErrorCode.MISSING_NAME_IN_LIBRARY_DIRECTIVE", 930),
    ("ParserErrorCode.MISSING_NAME_IN_PART_OF_DIRECTIVE", 931),
    ("ParserErrorCode.MISSING_PREFIX_IN_DEFERRED_IMPORT", 932),
    ("ParserErrorCode.MISSING_PRIMARY_CONSTRUCTOR", 933),
    ("ParserErrorCode.MISSING_PRIMARY_CONSTRUCTOR_PARAMETERS", 934),
    ("ParserErrorCode.MISSING_STAR_AFTER_SYNC", 935),
    ("ParserErrorCode.MISSING_STATEMENT", 936),
    ("ParserErrorCode.MISSING_TERMINATOR_FOR_PARAMETER_GROUP", 937),
    ("ParserErrorCode.MISSING_TYPEDEF_PARAMETERS", 938),
    ("ParserErrorCode.MISSING_VARIABLE_IN_FOR_EACH", 939),
    ("ParserErrorCode.MIXED_PARAMETER_GROUPS", 940),
    ("ParserErrorCode.MIXIN_DECLARES_CONSTRUCTOR", 941),
    ("ParserErrorCode.MIXIN_WITH_CLAUSE", 942),
    ("ParserErrorCode.MODIFIER_OUT_OF_ORDER", 943),
    ("ParserErrorCode.MULTIPLE_CLAUSES", 944),
    ("ParserErrorCode.MULTIPLE_EXTENDS_CLAUSES", 945),
    ("ParserErrorCode.MULTIPLE_IMPLEMENTS_CLAUSES", 946),
    ("ParserErrorCode.MULTIPLE_LIBRARY_DIRECTIVES", 947),
    ("ParserErrorCode.MULTIPLE_NAMED_PARAMETER_GROUPS", 948),
    ("ParserErrorCode.MULTIPLE_ON_CLAUSES", 949),
    ("ParserErrorCode.MULTIPLE_PART_OF_DIRECTIVES", 950),
    ("ParserErrorCode.MULTIPLE_POSITIONAL_PARAMETER_GROUPS", 951),
    ("ParserErrorCode.MULTIPLE_REPRESENTATION_FIELDS", 952),
    ("ParserErrorCode.MULTIPLE_VARIABLES_IN_FOR_EACH", 953),
    ("ParserErrorCode.MULTIPLE_VARIANCE_MODIFIERS", 954),
    ("ParserErrorCode.MULTIPLE_WITH_CLAUSES", 955),
    ("ParserErrorCode.NAMED_FUNCTION_EXPRESSION", 956),
    ("ParserErrorCode.NAMED_FUNCTION_TYPE", 957),
    ("ParserErrorCode.NAMED_PARAMETER_OUTSIDE_GROUP", 958),
    ("ParserErrorCode.NATIVE_CLAUSE_IN_NON_SDK_CODE", 959),
    ("ParserErrorCode.NATIVE_CLAUSE_SHOULD_BE_ANNOTATION", 960),
    ("ParserErrorCode.NATIVE_FUNCTION_BODY_IN_NON_SDK_CODE", 961),
    ("ParserErrorCode.NON_CONSTRUCTOR_FACTORY", 962),
    ("ParserErrorCode.NON_IDENTIFIER_LIBRARY_NAME", 963),
    ("ParserErrorCode.NON_PART_OF_DIRECTIVE_IN_PART", 964),
    ("ParserErrorCode.NON_STRING_LITERAL_AS_URI", 965),
    ("ParserErrorCode.NON_USER_DEFINABLE_OPERATOR", 966),
    ("ParserErrorCode.NORMAL_BEFORE_OPTIONAL_PARAMETERS", 967),
    ("ParserErrorCode.NULL_AWARE_CASCADE_OUT_OF_ORDER", 968),
    ("ParserErrorCode.OUT_OF_ORDER_CLAUSES", 969),
    ("ParserErrorCode.PART_OF_NAME", 970),
    ("ParserErrorCode.PATTERN_ASSIGNMENT_DECLARES_VARIABLE", 971),
    ("ParserErrorCode.PATTERN_VARIABLE_DECLARATION_OUTSIDE_FUNCTION_OR_METHOD", 972),
    ("ParserErrorCode.POSITIONAL_AFTER_NAMED_ARGUMENT", 973),
    ("ParserErrorCode.POSITIONAL_PARAMETER_OUTSIDE_GROUP", 974),
    ("ParserErrorCode.PREFIX_AFTER_COMBINATOR", 975),
    ("ParserErrorCode.RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA", 976),
    ("ParserErrorCode.RECORD_TYPE_ONE_POSITIONAL_NO_TRAILING_COMMA", 977),
    ("ParserErrorCode.REDIRECTING_CONSTRUCTOR_WITH_BODY", 978),
    ("ParserErrorCode.REDIRECTION_IN_NON_FACTORY_CONSTRUCTOR", 979),
    ("ParserErrorCode.REPRESENTATION_FIELD_MODIFIER", 980),
    ("ParserErrorCode.REPRESENTATION_FIELD_TRAILING_COMMA", 981),
    ("ParserErrorCode.SEALED_ENUM", 982),
    ("ParserErrorCode.SEALED_MIXIN", 983),
    ("ParserErrorCode.SEALED_MIXIN_CLASS", 984),
    ("ParserErrorCode.SETTER_CONSTRUCTOR", 985),
    ("ParserErrorCode.SETTER_IN_FUNCTION", 986),
    ("ParserErrorCode.STACK_OVERFLOW", 987),
    ("ParserErrorCode.STATIC_CONSTRUCTOR", 988),
    ("ParserErrorCode.STATIC_GETTER_WITHOUT_BODY", 989),
    ("ParserErrorCode.STATIC_OPERATOR", 990),
    ("ParserErrorCode.STATIC_SETTER_WITHOUT_BODY", 991),
    ("ParserErrorCode.SWITCH_HAS_CASE_AFTER_DEFAULT_CASE", 992),
    ("ParserErrorCode.SWITCH_HAS_MULTIPLE_DEFAULT_CASES", 993),
    ("ParserErrorCode.TOP_LEVEL_OPERATOR", 994),
    ("ParserErrorCode.TYPEDEF_IN_CLASS", 995),
    ("ParserErrorCode.TYPE_ARGUMENTS_ON_TYPE_VARIABLE", 996),
    ("ParserErrorCode.TYPE_BEFORE_FACTORY", 997),
    ("ParserErrorCode.TYPE_PARAMETER_ON_CONSTRUCTOR", 998),
    ("ParserErrorCode.TYPE_PARAMETER_ON_OPERATOR", 999),
    ("ParserErrorCode.UNEXPECTED_TERMINATOR_FOR_PARAMETER_GROUP", 1000),
    ("ParserErrorCode.UNEXPECTED_TOKEN", 1001),
    ("ParserErrorCode.UNEXPECTED_TOKENS", 1002),
    ("ParserErrorCode.VARIABLE_PATTERN_KEYWORD_IN_DECLARATION_CONTEXT", 1003),
    ("ParserErrorCode.VAR_AND_TYPE", 1004),
    ("ParserErrorCode.VAR_AS_TYPE_NAME", 1005),
    ("ParserErrorCode.VAR_CLASS", 1006),
    ("ParserErrorCode.VAR_ENUM", 1007),
    ("ParserErrorCode.VAR_RETURN_TYPE", 1008),
    ("ParserErrorCode.VAR_TYPEDEF", 1009),
    ("ParserErrorCode.VOID_WITH_TYPE_ARGUMENTS", 1010),
    ("ParserErrorCode.WITH_BEFORE_EXTENDS", 1011),
    ("ParserErrorCode.WRONG_SEPARATOR_FOR_POSITIONAL_PARAMETER", 1012),
    ("ParserErrorCode.WRONG_TERMINATOR_FOR_PARAMETER_GROUP", 1013),
    ("ScannerErrorCode.EXPECTED_TOKEN", 1014),
    ("ScannerErrorCode.ILLEGAL_CHARACTER", 1015),
    ("ScannerErrorCode.MISSING_DIGIT", 1016),
    ("ScannerErrorCode.MISSING_HEX_DIGIT", 1017),
    ("ScannerErrorCode.MISSING_IDENTIFIER", 1018),
    ("ScannerErrorCode.MISSING_QUOTE", 1019),
    ("ScannerErrorCode.UNABLE_GET_CONTENT", 1020),
    ("ScannerErrorCode.UNEXPECTED_DOLLAR_IN_STRING", 1021),
    ("ScannerErrorCode.UNEXPECTED_SEPARATOR_IN_NUMBER", 1022),
    ("ScannerErrorCode.UNSUPPORTED_OPERATOR", 1023),
    ("ScannerErrorCode.UNTERMINATED_MULTI_LINE_COMMENT", 1024),
    ("ScannerErrorCode.UNTERMINATED_STRING_LITERAL", 1025),
    ("StaticWarningCode.DEAD_NULL_AWARE_EXPRESSION", 542),
    ("StaticWarningCode.INVALID_NULL_AWARE_OPERATOR", 543),
    ("StaticWarningCode.INVALID_NULL_AWARE_OPERATOR_AFTER_SHORT_CIRCUIT", 544),
    ("StaticWarningCode.MISSING_ENUM_CONSTANT_IN_SWITCH", 545),
    ("StaticWarningCode.UNNECESSARY_NON_NULL_ASSERTION", 546),
    ("StaticWarningCode.UNNECESSARY_NULL_ASSERT_PATTERN", 547),
    ("StaticWarningCode.UNNECESSARY_NULL_CHECK_PATTERN", 548),
    ("TodoCode.FIXME", 1027),
    ("TodoCode.HACK", 1028),
    ("TodoCode.TODO", 1026),
    ("TodoCode.UNDONE", 1029),
    ("WarningCode.ARGUMENT_TYPE_NOT_ASSIGNABLE_TO_ERROR_HANDLER", 549),
    ("WarningCode.ASSIGNMENT_OF_DO_NOT_STORE", 550),
    ("WarningCode.BODY_MIGHT_COMPLETE_NORMALLY_CATCH_ERROR", 551),
    ("WarningCode.BODY_MIGHT_COMPLETE_NORMALLY_NULLABLE", 552),
    ("WarningCode.CAST_FROM_NULLABLE_ALWAYS_FAILS", 553),
    ("WarningCode.CAST_FROM_NULL_ALWAYS_FAILS", 554),
    ("WarningCode.CONSTANT_PATTERN_NEVER_MATCHES_VALUE_TYPE", 555),
    ("WarningCode.DEAD_CODE", 556),
    ("WarningCode.DEAD_CODE_CATCH_FOLLOWING_CATCH", 557),
    ("WarningCode.DEAD_CODE_LATE_WILDCARD_VARIABLE_INITIALIZER", 558),
    ("WarningCode.DEAD_CODE_ON_CATCH_SUBTYPE", 559),
    ("WarningCode.DEPRECATED_EXPORT_USE", 560),
    ("WarningCode.DEPRECATED_EXTENDS_FUNCTION", 561),
    ("WarningCode.DEPRECATED_IMPLEMENTS_FUNCTION", 562),
    ("WarningCode.DEPRECATED_MIXIN_FUNCTION", 563),
    ("WarningCode.DEPRECATED_NEW_IN_COMMENT_REFERENCE", 564),
    ("WarningCode.DOC_DIRECTIVE_ARGUMENT_WRONG_FORMAT", 565),
    ("WarningCode.DOC_DIRECTIVE_HAS_EXTRA_ARGUMENTS", 566),
    ("WarningCode.DOC_DIRECTIVE_HAS_UNEXPECTED_NAMED_ARGUMENT", 567),
    ("WarningCode.DOC_DIRECTIVE_MISSING_CLOSING_BRACE", 568),
    ("WarningCode.DOC_DIRECTIVE_MISSING_CLOSING_TAG", 569),
    ("WarningCode.DOC_DIRECTIVE_MISSING_ONE_ARGUMENT", 570),
    ("WarningCode.DOC_DIRECTIVE_MISSING_OPENING_TAG", 571),
    ("WarningCode.DOC_DIRECTIVE_MISSING_THREE_ARGUMENTS", 572),
    ("WarningCode.DOC_DIRECTIVE_MISSING_TWO_ARGUMENTS", 573),
    ("WarningCode.DOC_DIRECTIVE_UNKNOWN", 574),
    ("WarningCode.DOC_IMPORT_CANNOT_BE_DEFERRED", 575),
    ("WarningCode.DOC_IMPORT_CANNOT_HAVE_CONFIGURATIONS", 576),
    ("WarningCode.DUPLICATE_EXPORT", 577),
    ("WarningCode.DUPLICATE_HIDDEN_NAME", 578),
    ("WarningCode.DUPLICATE_IGNORE", 579),
    ("WarningCode.DUPLICATE_IMPORT", 580),
    ("WarningCode.DUPLICATE_SHOWN_NAME", 581),
    ("WarningCode.EQUAL_ELEMENTS_IN_SET", 582),
    ("WarningCode.EQUAL_KEYS_IN_MAP", 583),
    ("WarningCode.INFERENCE_FAILURE_ON_COLLECTION_LITERAL", 584),
    ("WarningCode.INFERENCE_FAILURE_ON_FUNCTION_INVOCATION", 585),
    ("WarningCode.INFERENCE_FAILURE_ON_FUNCTION_RETURN_TYPE", 586),
    ("WarningCode.INFERENCE_FAILURE_ON_GENERIC_INVOCATION", 587),
    ("WarningCode.INFERENCE_FAILURE_ON_INSTANCE_CREATION", 588),
    ("WarningCode.INFERENCE_FAILURE_ON_UNINITIALIZED_VARIABLE", 589),
    ("WarningCode.INFERENCE_FAILURE_ON_UNTYPED_PARAMETER", 590),
    ("WarningCode.INVALID_ANNOTATION_TARGET", 591),
    ("WarningCode.INVALID_EXPORT_OF_INTERNAL_ELEMENT", 592),
    ("WarningCode.INVALID_EXPORT_OF_INTERNAL_ELEMENT_INDIRECTLY", 593),
    ("WarningCode.INVALID_FACTORY_METHOD_DECL", 594),
    ("WarningCode.INVALID_FACTORY_METHOD_IMPL", 595),
    ("WarningCode.INVALID_INTERNAL_ANNOTATION", 596),
    ("WarningCode.INVALID_LANGUAGE_VERSION_OVERRIDE_AT_SIGN", 597),
    ("WarningCode.INVALID_LANGUAGE_VERSION_OVERRIDE_EQUALS", 598),
    ("WarningCode.INVALID_LANGUAGE_VERSION_OVERRIDE_GREATER", 599),
    ("WarningCode.INVALID_LANGUAGE_VERSION_OVERRIDE_LOCATION", 600),
    ("WarningCode.INVALID_LANGUAGE_VERSION_OVERRIDE_LOWER_CASE", 601),
    ("WarningCode.INVALID_LANGUAGE_VERSION_OVERRIDE_NUMBER", 602),
    ("WarningCode.INVALID_LANGUAGE_VERSION_OVERRIDE_PREFIX", 603),
    ("WarningCode.INVALID_LANGUAGE_VERSION_OVERRIDE_TRAILING_CHARACTERS", 604),
    ("WarningCode.INVALID_LANGUAGE_VERSION_OVERRIDE_TWO_SLASHES", 605),
    ("WarningCode.INVALID_LITERAL_ANNOTATION", 606),
    ("WarningCode.INVALID_NON_VIRTUAL_ANNOTATION", 607),
    ("WarningCode.INVALID_OVERRIDE_OF_NON_VIRTUAL_MEMBER", 608),
    ("WarningCode.INVALID_REOPEN_ANNOTATION", 609),
    ("WarningCode.INVALID_REQUIRED_NAMED_PARAM", 610),
    ("WarningCode.INVALID_REQUIRED_OPTIONAL_POSITIONAL_PARAM", 611),
    ("WarningCode.INVALID_REQUIRED_POSITIONAL_PARAM", 612),
    ("WarningCode.INVALID_USE_OF_INTERNAL_MEMBER", 613),
    ("WarningCode.INVALID_USE_OF_PROTECTED_MEMBER", 614),
    ("WarningCode.INVALID_USE_OF_VISIBLE_FOR_OVERRIDING_MEMBER", 615),
    ("WarningCode.INVALID_USE_OF_VISIBLE_FOR_TEMPLATE_MEMBER", 616),
    ("WarningCode.INVALID_USE_OF_VISIBLE_FOR_TESTING_MEMBER", 617),
    ("WarningCode.INVALID_VISIBILITY_ANNOTATION", 618),
    ("WarningCode.INVALID_VISIBLE_FOR_OVERRIDING_ANNOTATION", 619),
    ("WarningCode.INVALID_VISIBLE_OUTSIDE_TEMPLATE_ANNOTATION", 620),
    ("WarningCode.MACRO_WARNING", 621),
    ("WarningCode.MISSING_OVERRIDE_OF_MUST_BE_OVERRIDDEN_ONE", 622),
    ("WarningCode.MISSING_OVERRIDE_OF_MUST_BE_OVERRIDDEN_THREE_PLUS", 623),
    ("WarningCode.MISSING_OVERRIDE_OF_MUST_BE_OVERRIDDEN_TWO", 624),
    ("WarningCode.MISSING_REQUIRED_PARAM", 625),
    ("WarningCode.MISSING_REQUIRED_PARAM_WITH_DETAILS", 626),
    ("WarningCode.MIXIN_ON_SEALED_CLASS", 627),
    ("WarningCode.MUST_BE_IMMUTABLE", 628),
    ("WarningCode.MUST_CALL_SUPER", 629),
    ("WarningCode.NON_CONST_ARGUMENT_FOR_CONST_PARAMETER", 630),
    ("WarningCode.NON_CONST_CALL_TO_LITERAL_CONSTRUCTOR", 631),
    ("WarningCode.NON_CONST_CALL_TO_LITERAL_CONSTRUCTOR_USING_NEW", 632),
    ("WarningCode.NON_NULLABLE_EQUALS_PARAMETER", 633),
    ("WarningCode.NULLABLE_TYPE_IN_CATCH_CLAUSE", 634),
    ("WarningCode.NULL_ARGUMENT_TO_NON_NULL_TYPE", 635),
    ("WarningCode.NULL_CHECK_ALWAYS_FAILS", 636),
    ("WarningCode.OVERRIDE_ON_NON_OVERRIDING_FIELD", 637),
    ("WarningCode.OVERRIDE_ON_NON_OVERRIDING_GETTER", 638),
    ("WarningCode.OVERRIDE_ON_NON_OVERRIDING_METHOD", 639),
    ("WarningCode.OVERRIDE_ON_NON_OVERRIDING_SETTER", 640),
    ("WarningCode.PATTERN_NEVER_MATCHES_VALUE_TYPE", 641),
    ("WarningCode.RECEIVER_OF_TYPE_NEVER", 642),
    ("WarningCode.REDECLARE_ON_NON_REDECLARING_MEMBER", 643),
    ("WarningCode.REMOVED_LINT_USE", 644),
    ("WarningCode.REPLACED_LINT_USE", 645),
    ("WarningCode.RETURN_OF_DO_NOT_STORE", 646),
    ("WarningCode.RETURN_OF_INVALID_TYPE_FROM_CATCH_ERROR", 647),
    ("WarningCode.RETURN_TYPE_INVALID_FOR_CATCH_ERROR", 648),
    ("WarningCode.SDK_VERSION_CONSTRUCTOR_TEAROFFS", 649),
    ("WarningCode.SDK_VERSION_GT_GT_GT_OPERATOR", 650),
    ("WarningCode.SDK_VERSION_SINCE", 651),
    ("WarningCode.STRICT_RAW_TYPE", 652),
    ("WarningCode.SUBTYPE_OF_SEALED_CLASS", 653),
    ("WarningCode.TEXT_DIRECTION_CODE_POINT_IN_COMMENT", 654),
    ("WarningCode.TEXT_DIRECTION_CODE_POINT_IN_LITERAL", 655),
    ("WarningCode.TYPE_CHECK_IS_NOT_NULL", 656),
    ("WarningCode.TYPE_CHECK_IS_NULL", 657),
    ("WarningCode.UNDEFINED_HIDDEN_NAME", 658),
    ("WarningCode.UNDEFINED_REFERENCED_PARAMETER", 659),
    ("WarningCode.UNDEFINED_SHOWN_NAME", 660),
    ("WarningCode.UNIGNORABLE_IGNORE", 661),
    ("WarningCode.UNNECESSARY_CAST", 662),
    ("WarningCode.UNNECESSARY_CAST_PATTERN", 663),
    ("WarningCode.UNNECESSARY_FINAL", 664),
    ("WarningCode.UNNECESSARY_IGNORE", 665),
    ("WarningCode.UNNECESSARY_NAN_COMPARISON_FALSE", 666),
    ("WarningCode.UNNECESSARY_NAN_COMPARISON_TRUE", 667),
    ("WarningCode.UNNECESSARY_NO_SUCH_METHOD", 668),
    ("WarningCode.UNNECESSARY_NULL_COMPARISON_ALWAYS_NULL_FALSE", 669),
    ("WarningCode.UNNECESSARY_NULL_COMPARISON_ALWAYS_NULL_TRUE", 670),
    ("WarningCode.UNNECESSARY_NULL_COMPARISON_NEVER_NULL_FALSE", 671),
    ("WarningCode.UNNECESSARY_NULL_COMPARISON_NEVER_NULL_TRUE", 672),
    ("WarningCode.UNNECESSARY_QUESTION_MARK", 673),
    ("WarningCode.UNNECESSARY_SET_LITERAL", 674),
    ("WarningCode.UNNECESSARY_TYPE_CHECK_FALSE", 675),
    ("WarningCode.UNNECESSARY_TYPE_CHECK_TRUE", 676),
    ("WarningCode.UNNECESSARY_WILDCARD_PATTERN", 677),
    ("WarningCode.UNREACHABLE_SWITCH_CASE", 678),
    ("WarningCode.UNREACHABLE_SWITCH_DEFAULT", 679),
    ("WarningCode.UNUSED_CATCH_CLAUSE", 680),
    ("WarningCode.UNUSED_CATCH_STACK", 681),
    ("WarningCode.UNUSED_ELEMENT", 682),
    ("WarningCode.UNUSED_ELEMENT_PARAMETER", 683),
    ("WarningCode.UNUSED_FIELD", 684),
    ("WarningCode.UNUSED_IMPORT", 685),
    ("WarningCode.UNUSED_LABEL", 686),
    ("WarningCode.UNUSED_LOCAL_VARIABLE", 687),
    ("WarningCode.UNUSED_RESULT", 688),
    ("WarningCode.UNUSED_RESULT_WITH_MESSAGE", 689),
    ("WarningCode.UNUSED_SHOWN_NAME", 690),
    ("WarningCode.URI_DOES_NOT_EXIST_IN_DOC_IMPORT", 691),
    ("WarningCode.invalid_use_of_do_not_submit_member", 692),
];

pub mod modulos {
    /// `CompileTimeErrorCode`.
    pub mod compile_time_error {
        use crate::Codigo;
        pub const ABSTRACT_FIELD_CONSTRUCTOR_INITIALIZER: Codigo = Codigo(0);
        pub const ABSTRACT_FIELD_INITIALIZER: Codigo = Codigo(1);
        pub const ABSTRACT_SUPER_MEMBER_REFERENCE: Codigo = Codigo(2);
        pub const AMBIGUOUS_EXPORT: Codigo = Codigo(3);
        pub const AMBIGUOUS_EXTENSION_MEMBER_ACCESS: Codigo = Codigo(4);
        pub const AMBIGUOUS_IMPORT: Codigo = Codigo(5);
        pub const AMBIGUOUS_SET_OR_MAP_LITERAL_BOTH: Codigo = Codigo(6);
        pub const AMBIGUOUS_SET_OR_MAP_LITERAL_EITHER: Codigo = Codigo(7);
        pub const ARGUMENT_TYPE_NOT_ASSIGNABLE: Codigo = Codigo(8);
        pub const ASSERT_IN_REDIRECTING_CONSTRUCTOR: Codigo = Codigo(9);
        pub const ASSIGNMENT_TO_CONST: Codigo = Codigo(10);
        pub const ASSIGNMENT_TO_FINAL: Codigo = Codigo(11);
        pub const ASSIGNMENT_TO_FINAL_LOCAL: Codigo = Codigo(12);
        pub const ASSIGNMENT_TO_FINAL_NO_SETTER: Codigo = Codigo(13);
        pub const ASSIGNMENT_TO_FUNCTION: Codigo = Codigo(14);
        pub const ASSIGNMENT_TO_METHOD: Codigo = Codigo(15);
        pub const ASSIGNMENT_TO_TYPE: Codigo = Codigo(16);
        pub const ASYNC_FOR_IN_WRONG_CONTEXT: Codigo = Codigo(17);
        pub const AUGMENTATION_EXTENDS_CLAUSE_ALREADY_PRESENT: Codigo = Codigo(18);
        pub const AUGMENTATION_MODIFIER_EXTRA: Codigo = Codigo(19);
        pub const AUGMENTATION_MODIFIER_MISSING: Codigo = Codigo(20);
        pub const AUGMENTATION_OF_DIFFERENT_DECLARATION_KIND: Codigo = Codigo(21);
        pub const AUGMENTATION_TYPE_PARAMETER_BOUND: Codigo = Codigo(22);
        pub const AUGMENTATION_TYPE_PARAMETER_COUNT: Codigo = Codigo(23);
        pub const AUGMENTATION_TYPE_PARAMETER_NAME: Codigo = Codigo(24);
        pub const AUGMENTATION_WITHOUT_DECLARATION: Codigo = Codigo(25);
        pub const AUGMENTED_EXPRESSION_IS_NOT_SETTER: Codigo = Codigo(26);
        pub const AUGMENTED_EXPRESSION_IS_SETTER: Codigo = Codigo(27);
        pub const AUGMENTED_EXPRESSION_NOT_OPERATOR: Codigo = Codigo(28);
        pub const AWAIT_IN_LATE_LOCAL_VARIABLE_INITIALIZER: Codigo = Codigo(29);
        pub const AWAIT_IN_WRONG_CONTEXT: Codigo = Codigo(30);
        pub const AWAIT_OF_INCOMPATIBLE_TYPE: Codigo = Codigo(31);
        pub const BASE_CLASS_IMPLEMENTED_OUTSIDE_OF_LIBRARY: Codigo = Codigo(32);
        pub const BASE_MIXIN_IMPLEMENTED_OUTSIDE_OF_LIBRARY: Codigo = Codigo(33);
        pub const BODY_MIGHT_COMPLETE_NORMALLY: Codigo = Codigo(34);
        pub const BREAK_LABEL_ON_SWITCH_MEMBER: Codigo = Codigo(35);
        pub const BUILT_IN_IDENTIFIER_AS_EXTENSION_NAME: Codigo = Codigo(36);
        pub const BUILT_IN_IDENTIFIER_AS_EXTENSION_TYPE_NAME: Codigo = Codigo(37);
        pub const BUILT_IN_IDENTIFIER_AS_PREFIX_NAME: Codigo = Codigo(38);
        pub const BUILT_IN_IDENTIFIER_AS_TYPE: Codigo = Codigo(39);
        pub const BUILT_IN_IDENTIFIER_AS_TYPEDEF_NAME: Codigo = Codigo(40);
        pub const BUILT_IN_IDENTIFIER_AS_TYPE_NAME: Codigo = Codigo(41);
        pub const BUILT_IN_IDENTIFIER_AS_TYPE_PARAMETER_NAME: Codigo = Codigo(42);
        pub const CASE_EXPRESSION_TYPE_IMPLEMENTS_EQUALS: Codigo = Codigo(43);
        pub const CASE_EXPRESSION_TYPE_IS_NOT_SWITCH_EXPRESSION_SUBTYPE: Codigo = Codigo(44);
        pub const CAST_TO_NON_TYPE: Codigo = Codigo(45);
        pub const CLASS_INSTANTIATION_ACCESS_TO_INSTANCE_MEMBER: Codigo = Codigo(46);
        pub const CLASS_INSTANTIATION_ACCESS_TO_STATIC_MEMBER: Codigo = Codigo(47);
        pub const CLASS_INSTANTIATION_ACCESS_TO_UNKNOWN_MEMBER: Codigo = Codigo(48);
        pub const CLASS_USED_AS_MIXIN: Codigo = Codigo(49);
        pub const CONCRETE_CLASS_HAS_ENUM_SUPERINTERFACE: Codigo = Codigo(50);
        pub const CONCRETE_CLASS_WITH_ABSTRACT_MEMBER: Codigo = Codigo(51);
        pub const CONFLICTING_CONSTRUCTOR_AND_STATIC_FIELD: Codigo = Codigo(52);
        pub const CONFLICTING_CONSTRUCTOR_AND_STATIC_GETTER: Codigo = Codigo(53);
        pub const CONFLICTING_CONSTRUCTOR_AND_STATIC_METHOD: Codigo = Codigo(54);
        pub const CONFLICTING_CONSTRUCTOR_AND_STATIC_SETTER: Codigo = Codigo(55);
        pub const CONFLICTING_FIELD_AND_METHOD: Codigo = Codigo(56);
        pub const CONFLICTING_GENERIC_INTERFACES: Codigo = Codigo(57);
        pub const CONFLICTING_INHERITED_METHOD_AND_SETTER: Codigo = Codigo(58);
        pub const CONFLICTING_METHOD_AND_FIELD: Codigo = Codigo(59);
        pub const CONFLICTING_STATIC_AND_INSTANCE: Codigo = Codigo(60);
        pub const CONFLICTING_TYPE_VARIABLE_AND_CLASS: Codigo = Codigo(61);
        pub const CONFLICTING_TYPE_VARIABLE_AND_ENUM: Codigo = Codigo(62);
        pub const CONFLICTING_TYPE_VARIABLE_AND_EXTENSION: Codigo = Codigo(63);
        pub const CONFLICTING_TYPE_VARIABLE_AND_EXTENSION_TYPE: Codigo = Codigo(64);
        pub const CONFLICTING_TYPE_VARIABLE_AND_MEMBER_CLASS: Codigo = Codigo(65);
        pub const CONFLICTING_TYPE_VARIABLE_AND_MEMBER_ENUM: Codigo = Codigo(66);
        pub const CONFLICTING_TYPE_VARIABLE_AND_MEMBER_EXTENSION: Codigo = Codigo(67);
        pub const CONFLICTING_TYPE_VARIABLE_AND_MEMBER_EXTENSION_TYPE: Codigo = Codigo(68);
        pub const CONFLICTING_TYPE_VARIABLE_AND_MEMBER_MIXIN: Codigo = Codigo(69);
        pub const CONFLICTING_TYPE_VARIABLE_AND_MIXIN: Codigo = Codigo(70);
        pub const CONSTANT_PATTERN_WITH_NON_CONSTANT_EXPRESSION: Codigo = Codigo(71);
        pub const CONST_CONSTRUCTOR_CONSTANT_FROM_DEFERRED_LIBRARY: Codigo = Codigo(72);
        pub const CONST_CONSTRUCTOR_FIELD_TYPE_MISMATCH: Codigo = Codigo(73);
        pub const CONST_CONSTRUCTOR_PARAM_TYPE_MISMATCH: Codigo = Codigo(74);
        pub const CONST_CONSTRUCTOR_THROWS_EXCEPTION: Codigo = Codigo(75);
        pub const CONST_CONSTRUCTOR_WITH_FIELD_INITIALIZED_BY_NON_CONST: Codigo = Codigo(76);
        pub const CONST_CONSTRUCTOR_WITH_MIXIN_WITH_FIELD: Codigo = Codigo(77);
        pub const CONST_CONSTRUCTOR_WITH_MIXIN_WITH_FIELDS: Codigo = Codigo(78);
        pub const CONST_CONSTRUCTOR_WITH_NON_CONST_SUPER: Codigo = Codigo(79);
        pub const CONST_CONSTRUCTOR_WITH_NON_FINAL_FIELD: Codigo = Codigo(80);
        pub const CONST_DEFERRED_CLASS: Codigo = Codigo(81);
        pub const CONST_EVAL_ASSERTION_FAILURE: Codigo = Codigo(82);
        pub const CONST_EVAL_ASSERTION_FAILURE_WITH_MESSAGE: Codigo = Codigo(83);
        pub const CONST_EVAL_EXTENSION_METHOD: Codigo = Codigo(84);
        pub const CONST_EVAL_EXTENSION_TYPE_METHOD: Codigo = Codigo(85);
        pub const CONST_EVAL_FOR_ELEMENT: Codigo = Codigo(86);
        pub const CONST_EVAL_METHOD_INVOCATION: Codigo = Codigo(87);
        pub const CONST_EVAL_PROPERTY_ACCESS: Codigo = Codigo(88);
        pub const CONST_EVAL_THROWS_EXCEPTION: Codigo = Codigo(89);
        pub const CONST_EVAL_THROWS_IDBZE: Codigo = Codigo(90);
        pub const CONST_EVAL_TYPE_BOOL: Codigo = Codigo(91);
        pub const CONST_EVAL_TYPE_BOOL_INT: Codigo = Codigo(92);
        pub const CONST_EVAL_TYPE_BOOL_NUM_STRING: Codigo = Codigo(93);
        pub const CONST_EVAL_TYPE_INT: Codigo = Codigo(94);
        pub const CONST_EVAL_TYPE_NUM: Codigo = Codigo(95);
        pub const CONST_EVAL_TYPE_NUM_STRING: Codigo = Codigo(96);
        pub const CONST_EVAL_TYPE_STRING: Codigo = Codigo(97);
        pub const CONST_EVAL_TYPE_TYPE: Codigo = Codigo(98);
        pub const CONST_FIELD_INITIALIZER_NOT_ASSIGNABLE: Codigo = Codigo(99);
        pub const CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE: Codigo = Codigo(100);
        pub const CONST_INITIALIZED_WITH_NON_CONSTANT_VALUE_FROM_DEFERRED_LIBRARY: Codigo = Codigo(101);
        pub const CONST_INSTANCE_FIELD: Codigo = Codigo(102);
        pub const CONST_MAP_KEY_NOT_PRIMITIVE_EQUALITY: Codigo = Codigo(103);
        pub const CONST_NOT_INITIALIZED: Codigo = Codigo(104);
        pub const CONST_SET_ELEMENT_NOT_PRIMITIVE_EQUALITY: Codigo = Codigo(105);
        pub const CONST_SPREAD_EXPECTED_LIST_OR_SET: Codigo = Codigo(106);
        pub const CONST_SPREAD_EXPECTED_MAP: Codigo = Codigo(107);
        pub const CONST_TYPE_PARAMETER: Codigo = Codigo(108);
        pub const CONST_WITH_NON_CONST: Codigo = Codigo(109);
        pub const CONST_WITH_NON_CONSTANT_ARGUMENT: Codigo = Codigo(110);
        pub const CONST_WITH_NON_TYPE: Codigo = Codigo(111);
        pub const CONST_WITH_TYPE_PARAMETERS: Codigo = Codigo(112);
        pub const CONST_WITH_TYPE_PARAMETERS_CONSTRUCTOR_TEAROFF: Codigo = Codigo(113);
        pub const CONST_WITH_TYPE_PARAMETERS_FUNCTION_TEAROFF: Codigo = Codigo(114);
        pub const CONST_WITH_UNDEFINED_CONSTRUCTOR: Codigo = Codigo(115);
        pub const CONST_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT: Codigo = Codigo(116);
        pub const CONTINUE_LABEL_INVALID: Codigo = Codigo(117);
        pub const COULD_NOT_INFER: Codigo = Codigo(118);
        pub const DEFAULT_VALUE_IN_REDIRECTING_FACTORY_CONSTRUCTOR: Codigo = Codigo(119);
        pub const DEFAULT_VALUE_ON_REQUIRED_PARAMETER: Codigo = Codigo(120);
        pub const DEFERRED_IMPORT_OF_EXTENSION: Codigo = Codigo(121);
        pub const DEFINITELY_UNASSIGNED_LATE_LOCAL_VARIABLE: Codigo = Codigo(122);
        pub const DISALLOWED_TYPE_INSTANTIATION_EXPRESSION: Codigo = Codigo(123);
        pub const DUPLICATE_CONSTRUCTOR_DEFAULT: Codigo = Codigo(124);
        pub const DUPLICATE_CONSTRUCTOR_NAME: Codigo = Codigo(125);
        pub const DUPLICATE_DEFINITION: Codigo = Codigo(126);
        pub const DUPLICATE_FIELD_FORMAL_PARAMETER: Codigo = Codigo(127);
        pub const DUPLICATE_FIELD_NAME: Codigo = Codigo(128);
        pub const DUPLICATE_NAMED_ARGUMENT: Codigo = Codigo(129);
        pub const DUPLICATE_PART: Codigo = Codigo(130);
        pub const DUPLICATE_PATTERN_ASSIGNMENT_VARIABLE: Codigo = Codigo(131);
        pub const DUPLICATE_PATTERN_FIELD: Codigo = Codigo(132);
        pub const DUPLICATE_REST_ELEMENT_IN_PATTERN: Codigo = Codigo(133);
        pub const DUPLICATE_VARIABLE_PATTERN: Codigo = Codigo(134);
        pub const EMPTY_MAP_PATTERN: Codigo = Codigo(135);
        pub const ENUM_CONSTANT_INVOKES_FACTORY_CONSTRUCTOR: Codigo = Codigo(136);
        pub const ENUM_CONSTANT_SAME_NAME_AS_ENCLOSING: Codigo = Codigo(137);
        pub const ENUM_INSTANTIATED_TO_BOUNDS_IS_NOT_WELL_BOUNDED: Codigo = Codigo(138);
        pub const ENUM_MIXIN_WITH_INSTANCE_VARIABLE: Codigo = Codigo(139);
        pub const ENUM_WITHOUT_CONSTANTS: Codigo = Codigo(140);
        pub const ENUM_WITH_ABSTRACT_MEMBER: Codigo = Codigo(141);
        pub const ENUM_WITH_NAME_VALUES: Codigo = Codigo(142);
        pub const EQUAL_ELEMENTS_IN_CONST_SET: Codigo = Codigo(143);
        pub const EQUAL_KEYS_IN_CONST_MAP: Codigo = Codigo(144);
        pub const EQUAL_KEYS_IN_MAP_PATTERN: Codigo = Codigo(145);
        pub const EXPECTED_ONE_LIST_PATTERN_TYPE_ARGUMENTS: Codigo = Codigo(146);
        pub const EXPECTED_ONE_LIST_TYPE_ARGUMENTS: Codigo = Codigo(147);
        pub const EXPECTED_ONE_SET_TYPE_ARGUMENTS: Codigo = Codigo(148);
        pub const EXPECTED_TWO_MAP_PATTERN_TYPE_ARGUMENTS: Codigo = Codigo(149);
        pub const EXPECTED_TWO_MAP_TYPE_ARGUMENTS: Codigo = Codigo(150);
        pub const EXPORT_INTERNAL_LIBRARY: Codigo = Codigo(151);
        pub const EXPORT_OF_NON_LIBRARY: Codigo = Codigo(152);
        pub const EXPRESSION_IN_MAP: Codigo = Codigo(153);
        pub const EXTENDS_DEFERRED_CLASS: Codigo = Codigo(154);
        pub const EXTENDS_DISALLOWED_CLASS: Codigo = Codigo(155);
        pub const EXTENDS_NON_CLASS: Codigo = Codigo(156);
        pub const EXTENDS_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER: Codigo = Codigo(157);
        pub const EXTENSION_AS_EXPRESSION: Codigo = Codigo(158);
        pub const EXTENSION_CONFLICTING_STATIC_AND_INSTANCE: Codigo = Codigo(159);
        pub const EXTENSION_DECLARES_MEMBER_OF_OBJECT: Codigo = Codigo(160);
        pub const EXTENSION_OVERRIDE_ACCESS_TO_STATIC_MEMBER: Codigo = Codigo(161);
        pub const EXTENSION_OVERRIDE_ARGUMENT_NOT_ASSIGNABLE: Codigo = Codigo(162);
        pub const EXTENSION_OVERRIDE_WITHOUT_ACCESS: Codigo = Codigo(163);
        pub const EXTENSION_OVERRIDE_WITH_CASCADE: Codigo = Codigo(164);
        pub const EXTENSION_TYPE_CONSTRUCTOR_WITH_SUPER_FORMAL_PARAMETER: Codigo = Codigo(165);
        pub const EXTENSION_TYPE_CONSTRUCTOR_WITH_SUPER_INVOCATION: Codigo = Codigo(166);
        pub const EXTENSION_TYPE_DECLARES_INSTANCE_FIELD: Codigo = Codigo(167);
        pub const EXTENSION_TYPE_DECLARES_MEMBER_OF_OBJECT: Codigo = Codigo(168);
        pub const EXTENSION_TYPE_IMPLEMENTS_DISALLOWED_TYPE: Codigo = Codigo(169);
        pub const EXTENSION_TYPE_IMPLEMENTS_ITSELF: Codigo = Codigo(170);
        pub const EXTENSION_TYPE_IMPLEMENTS_NOT_SUPERTYPE: Codigo = Codigo(171);
        pub const EXTENSION_TYPE_IMPLEMENTS_REPRESENTATION_NOT_SUPERTYPE: Codigo = Codigo(172);
        pub const EXTENSION_TYPE_INHERITED_MEMBER_CONFLICT: Codigo = Codigo(173);
        pub const EXTENSION_TYPE_REPRESENTATION_DEPENDS_ON_ITSELF: Codigo = Codigo(174);
        pub const EXTENSION_TYPE_REPRESENTATION_TYPE_BOTTOM: Codigo = Codigo(175);
        pub const EXTENSION_TYPE_WITH_ABSTRACT_MEMBER: Codigo = Codigo(176);
        pub const EXTERNAL_FIELD_CONSTRUCTOR_INITIALIZER: Codigo = Codigo(177);
        pub const EXTERNAL_FIELD_INITIALIZER: Codigo = Codigo(178);
        pub const EXTERNAL_VARIABLE_INITIALIZER: Codigo = Codigo(179);
        pub const EXTRA_POSITIONAL_ARGUMENTS: Codigo = Codigo(180);
        pub const EXTRA_POSITIONAL_ARGUMENTS_COULD_BE_NAMED: Codigo = Codigo(181);
        pub const FIELD_INITIALIZED_BY_MULTIPLE_INITIALIZERS: Codigo = Codigo(182);
        pub const FIELD_INITIALIZED_IN_INITIALIZER_AND_DECLARATION: Codigo = Codigo(183);
        pub const FIELD_INITIALIZED_IN_PARAMETER_AND_INITIALIZER: Codigo = Codigo(184);
        pub const FIELD_INITIALIZER_FACTORY_CONSTRUCTOR: Codigo = Codigo(185);
        pub const FIELD_INITIALIZER_NOT_ASSIGNABLE: Codigo = Codigo(186);
        pub const FIELD_INITIALIZER_OUTSIDE_CONSTRUCTOR: Codigo = Codigo(187);
        pub const FIELD_INITIALIZER_REDIRECTING_CONSTRUCTOR: Codigo = Codigo(188);
        pub const FIELD_INITIALIZING_FORMAL_NOT_ASSIGNABLE: Codigo = Codigo(189);
        pub const FINAL_CLASS_EXTENDED_OUTSIDE_OF_LIBRARY: Codigo = Codigo(190);
        pub const FINAL_CLASS_IMPLEMENTED_OUTSIDE_OF_LIBRARY: Codigo = Codigo(191);
        pub const FINAL_CLASS_USED_AS_MIXIN_CONSTRAINT_OUTSIDE_OF_LIBRARY: Codigo = Codigo(192);
        pub const FINAL_INITIALIZED_IN_DECLARATION_AND_CONSTRUCTOR: Codigo = Codigo(193);
        pub const FINAL_NOT_INITIALIZED: Codigo = Codigo(194);
        pub const FINAL_NOT_INITIALIZED_CONSTRUCTOR_1: Codigo = Codigo(195);
        pub const FINAL_NOT_INITIALIZED_CONSTRUCTOR_2: Codigo = Codigo(196);
        pub const FINAL_NOT_INITIALIZED_CONSTRUCTOR_3_PLUS: Codigo = Codigo(197);
        pub const FOR_IN_OF_INVALID_ELEMENT_TYPE: Codigo = Codigo(198);
        pub const FOR_IN_OF_INVALID_TYPE: Codigo = Codigo(199);
        pub const FOR_IN_WITH_CONST_VARIABLE: Codigo = Codigo(200);
        pub const GENERIC_FUNCTION_TYPE_CANNOT_BE_BOUND: Codigo = Codigo(201);
        pub const GENERIC_FUNCTION_TYPE_CANNOT_BE_TYPE_ARGUMENT: Codigo = Codigo(202);
        pub const GENERIC_METHOD_TYPE_INSTANTIATION_ON_DYNAMIC: Codigo = Codigo(203);
        pub const GETTER_NOT_ASSIGNABLE_SETTER_TYPES: Codigo = Codigo(204);
        pub const GETTER_NOT_SUBTYPE_SETTER_TYPES: Codigo = Codigo(205);
        pub const IF_ELEMENT_CONDITION_FROM_DEFERRED_LIBRARY: Codigo = Codigo(206);
        pub const ILLEGAL_ASYNC_GENERATOR_RETURN_TYPE: Codigo = Codigo(207);
        pub const ILLEGAL_ASYNC_RETURN_TYPE: Codigo = Codigo(208);
        pub const ILLEGAL_CONCRETE_ENUM_MEMBER_DECLARATION: Codigo = Codigo(209);
        pub const ILLEGAL_CONCRETE_ENUM_MEMBER_INHERITANCE: Codigo = Codigo(210);
        pub const ILLEGAL_ENUM_VALUES_DECLARATION: Codigo = Codigo(211);
        pub const ILLEGAL_ENUM_VALUES_INHERITANCE: Codigo = Codigo(212);
        pub const ILLEGAL_LANGUAGE_VERSION_OVERRIDE: Codigo = Codigo(213);
        pub const ILLEGAL_SYNC_GENERATOR_RETURN_TYPE: Codigo = Codigo(214);
        pub const IMPLEMENTS_DEFERRED_CLASS: Codigo = Codigo(215);
        pub const IMPLEMENTS_DISALLOWED_CLASS: Codigo = Codigo(216);
        pub const IMPLEMENTS_NON_CLASS: Codigo = Codigo(217);
        pub const IMPLEMENTS_REPEATED: Codigo = Codigo(218);
        pub const IMPLEMENTS_SUPER_CLASS: Codigo = Codigo(219);
        pub const IMPLEMENTS_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER: Codigo = Codigo(220);
        pub const IMPLICIT_SUPER_INITIALIZER_MISSING_ARGUMENTS: Codigo = Codigo(221);
        pub const IMPLICIT_THIS_REFERENCE_IN_INITIALIZER: Codigo = Codigo(222);
        pub const IMPORT_INTERNAL_LIBRARY: Codigo = Codigo(223);
        pub const IMPORT_OF_NON_LIBRARY: Codigo = Codigo(224);
        pub const INCONSISTENT_CASE_EXPRESSION_TYPES: Codigo = Codigo(225);
        pub const INCONSISTENT_INHERITANCE: Codigo = Codigo(226);
        pub const INCONSISTENT_INHERITANCE_GETTER_AND_METHOD: Codigo = Codigo(227);
        pub const INCONSISTENT_LANGUAGE_VERSION_OVERRIDE: Codigo = Codigo(228);
        pub const INCONSISTENT_PATTERN_VARIABLE_LOGICAL_OR: Codigo = Codigo(229);
        pub const INITIALIZER_FOR_NON_EXISTENT_FIELD: Codigo = Codigo(230);
        pub const INITIALIZER_FOR_STATIC_FIELD: Codigo = Codigo(231);
        pub const INITIALIZING_FORMAL_FOR_NON_EXISTENT_FIELD: Codigo = Codigo(232);
        pub const INSTANCE_ACCESS_TO_STATIC_MEMBER: Codigo = Codigo(233);
        pub const INSTANCE_ACCESS_TO_STATIC_MEMBER_OF_UNNAMED_EXTENSION: Codigo = Codigo(234);
        pub const INSTANCE_MEMBER_ACCESS_FROM_FACTORY: Codigo = Codigo(235);
        pub const INSTANCE_MEMBER_ACCESS_FROM_STATIC: Codigo = Codigo(236);
        pub const INSTANTIATE_ABSTRACT_CLASS: Codigo = Codigo(237);
        pub const INSTANTIATE_ENUM: Codigo = Codigo(238);
        pub const INSTANTIATE_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER: Codigo = Codigo(239);
        pub const INTEGER_LITERAL_IMPRECISE_AS_DOUBLE: Codigo = Codigo(240);
        pub const INTEGER_LITERAL_OUT_OF_RANGE: Codigo = Codigo(241);
        pub const INTERFACE_CLASS_EXTENDED_OUTSIDE_OF_LIBRARY: Codigo = Codigo(242);
        pub const INVALID_ANNOTATION: Codigo = Codigo(243);
        pub const INVALID_ANNOTATION_CONSTANT_VALUE_FROM_DEFERRED_LIBRARY: Codigo = Codigo(244);
        pub const INVALID_ANNOTATION_FROM_DEFERRED_LIBRARY: Codigo = Codigo(245);
        pub const INVALID_ASSIGNMENT: Codigo = Codigo(246);
        pub const INVALID_CAST_FUNCTION: Codigo = Codigo(247);
        pub const INVALID_CAST_FUNCTION_EXPR: Codigo = Codigo(248);
        pub const INVALID_CAST_LITERAL: Codigo = Codigo(249);
        pub const INVALID_CAST_LITERAL_LIST: Codigo = Codigo(250);
        pub const INVALID_CAST_LITERAL_MAP: Codigo = Codigo(251);
        pub const INVALID_CAST_LITERAL_SET: Codigo = Codigo(252);
        pub const INVALID_CAST_METHOD: Codigo = Codigo(253);
        pub const INVALID_CAST_NEW_EXPR: Codigo = Codigo(254);
        pub const INVALID_CONSTANT: Codigo = Codigo(255);
        pub const INVALID_EXTENSION_ARGUMENT_COUNT: Codigo = Codigo(256);
        pub const INVALID_FACTORY_NAME_NOT_A_CLASS: Codigo = Codigo(257);
        pub const INVALID_FIELD_NAME_FROM_OBJECT: Codigo = Codigo(258);
        pub const INVALID_FIELD_NAME_POSITIONAL: Codigo = Codigo(259);
        pub const INVALID_FIELD_NAME_PRIVATE: Codigo = Codigo(260);
        pub const INVALID_IMPLEMENTATION_OVERRIDE: Codigo = Codigo(261);
        pub const INVALID_IMPLEMENTATION_OVERRIDE_SETTER: Codigo = Codigo(262);
        pub const INVALID_INLINE_FUNCTION_TYPE: Codigo = Codigo(263);
        pub const INVALID_MACRO_APPLICATION_TARGET: Codigo = Codigo(264);
        pub const INVALID_MODIFIER_ON_CONSTRUCTOR: Codigo = Codigo(265);
        pub const INVALID_MODIFIER_ON_SETTER: Codigo = Codigo(266);
        pub const INVALID_OVERRIDE: Codigo = Codigo(267);
        pub const INVALID_OVERRIDE_SETTER: Codigo = Codigo(268);
        pub const INVALID_REFERENCE_TO_GENERATIVE_ENUM_CONSTRUCTOR: Codigo = Codigo(269);
        pub const INVALID_REFERENCE_TO_THIS: Codigo = Codigo(270);
        pub const INVALID_SUPER_FORMAL_PARAMETER_LOCATION: Codigo = Codigo(271);
        pub const INVALID_TYPE_ARGUMENT_IN_CONST_LIST: Codigo = Codigo(272);
        pub const INVALID_TYPE_ARGUMENT_IN_CONST_MAP: Codigo = Codigo(273);
        pub const INVALID_TYPE_ARGUMENT_IN_CONST_SET: Codigo = Codigo(274);
        pub const INVALID_URI: Codigo = Codigo(275);
        pub const INVALID_USE_OF_COVARIANT: Codigo = Codigo(276);
        pub const INVALID_USE_OF_NULL_VALUE: Codigo = Codigo(277);
        pub const INVOCATION_OF_EXTENSION_WITHOUT_CALL: Codigo = Codigo(278);
        pub const INVOCATION_OF_NON_FUNCTION: Codigo = Codigo(279);
        pub const INVOCATION_OF_NON_FUNCTION_EXPRESSION: Codigo = Codigo(280);
        pub const LABEL_IN_OUTER_SCOPE: Codigo = Codigo(281);
        pub const LABEL_UNDEFINED: Codigo = Codigo(282);
        pub const LATE_FINAL_FIELD_WITH_CONST_CONSTRUCTOR: Codigo = Codigo(283);
        pub const LATE_FINAL_LOCAL_ALREADY_ASSIGNED: Codigo = Codigo(284);
        pub const LIST_ELEMENT_TYPE_NOT_ASSIGNABLE: Codigo = Codigo(285);
        pub const MACRO_APPLICATION_ARGUMENT_ERROR: Codigo = Codigo(286);
        pub const MACRO_DECLARATIONS_PHASE_INTROSPECTION_CYCLE: Codigo = Codigo(287);
        pub const MACRO_DEFINITION_APPLICATION_SAME_LIBRARY_CYCLE: Codigo = Codigo(288);
        pub const MACRO_ERROR: Codigo = Codigo(289);
        pub const MACRO_INTERNAL_EXCEPTION: Codigo = Codigo(290);
        pub const MACRO_NOT_ALLOWED_DECLARATION: Codigo = Codigo(291);
        pub const MAIN_FIRST_POSITIONAL_PARAMETER_TYPE: Codigo = Codigo(292);
        pub const MAIN_HAS_REQUIRED_NAMED_PARAMETERS: Codigo = Codigo(293);
        pub const MAIN_HAS_TOO_MANY_REQUIRED_POSITIONAL_PARAMETERS: Codigo = Codigo(294);
        pub const MAIN_IS_NOT_FUNCTION: Codigo = Codigo(295);
        pub const MAP_ENTRY_NOT_IN_MAP: Codigo = Codigo(296);
        pub const MAP_KEY_TYPE_NOT_ASSIGNABLE: Codigo = Codigo(297);
        pub const MAP_VALUE_TYPE_NOT_ASSIGNABLE: Codigo = Codigo(298);
        pub const MISSING_CONST_IN_LIST_LITERAL: Codigo = Codigo(299);
        pub const MISSING_CONST_IN_MAP_LITERAL: Codigo = Codigo(300);
        pub const MISSING_CONST_IN_SET_LITERAL: Codigo = Codigo(301);
        pub const MISSING_DART_LIBRARY: Codigo = Codigo(302);
        pub const MISSING_DEFAULT_VALUE_FOR_PARAMETER: Codigo = Codigo(303);
        pub const MISSING_DEFAULT_VALUE_FOR_PARAMETER_POSITIONAL: Codigo = Codigo(304);
        pub const MISSING_DEFAULT_VALUE_FOR_PARAMETER_WITH_ANNOTATION: Codigo = Codigo(305);
        pub const MISSING_NAMED_PATTERN_FIELD_NAME: Codigo = Codigo(306);
        pub const MISSING_REQUIRED_ARGUMENT: Codigo = Codigo(307);
        pub const MISSING_VARIABLE_PATTERN: Codigo = Codigo(308);
        pub const MIXINS_SUPER_CLASS: Codigo = Codigo(309);
        pub const MIXIN_APPLICATION_CONCRETE_SUPER_INVOKED_MEMBER_TYPE: Codigo = Codigo(310);
        pub const MIXIN_APPLICATION_NOT_IMPLEMENTED_INTERFACE: Codigo = Codigo(311);
        pub const MIXIN_APPLICATION_NO_CONCRETE_SUPER_INVOKED_MEMBER: Codigo = Codigo(312);
        pub const MIXIN_APPLICATION_NO_CONCRETE_SUPER_INVOKED_SETTER: Codigo = Codigo(313);
        pub const MIXIN_CLASS_DECLARATION_EXTENDS_NOT_OBJECT: Codigo = Codigo(314);
        pub const MIXIN_CLASS_DECLARES_CONSTRUCTOR: Codigo = Codigo(315);
        pub const MIXIN_DEFERRED_CLASS: Codigo = Codigo(316);
        pub const MIXIN_INHERITS_FROM_NOT_OBJECT: Codigo = Codigo(317);
        pub const MIXIN_INSTANTIATE: Codigo = Codigo(318);
        pub const MIXIN_OF_DISALLOWED_CLASS: Codigo = Codigo(319);
        pub const MIXIN_OF_NON_CLASS: Codigo = Codigo(320);
        pub const MIXIN_OF_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER: Codigo = Codigo(321);
        pub const MIXIN_ON_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER: Codigo = Codigo(322);
        pub const MIXIN_SUBTYPE_OF_BASE_IS_NOT_BASE: Codigo = Codigo(323);
        pub const MIXIN_SUBTYPE_OF_FINAL_IS_NOT_BASE: Codigo = Codigo(324);
        pub const MIXIN_SUPER_CLASS_CONSTRAINT_DEFERRED_CLASS: Codigo = Codigo(325);
        pub const MIXIN_SUPER_CLASS_CONSTRAINT_DISALLOWED_CLASS: Codigo = Codigo(326);
        pub const MIXIN_SUPER_CLASS_CONSTRAINT_NON_INTERFACE: Codigo = Codigo(327);
        pub const MIXIN_WITH_NON_CLASS_SUPERCLASS: Codigo = Codigo(328);
        pub const MULTIPLE_REDIRECTING_CONSTRUCTOR_INVOCATIONS: Codigo = Codigo(329);
        pub const MULTIPLE_SUPER_INITIALIZERS: Codigo = Codigo(330);
        pub const NEW_WITH_NON_TYPE: Codigo = Codigo(331);
        pub const NEW_WITH_UNDEFINED_CONSTRUCTOR: Codigo = Codigo(332);
        pub const NEW_WITH_UNDEFINED_CONSTRUCTOR_DEFAULT: Codigo = Codigo(333);
        pub const NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_FIVE_PLUS: Codigo = Codigo(334);
        pub const NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_FOUR: Codigo = Codigo(335);
        pub const NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_ONE: Codigo = Codigo(336);
        pub const NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_THREE: Codigo = Codigo(337);
        pub const NON_ABSTRACT_CLASS_INHERITS_ABSTRACT_MEMBER_TWO: Codigo = Codigo(338);
        pub const NON_BOOL_CONDITION: Codigo = Codigo(339);
        pub const NON_BOOL_EXPRESSION: Codigo = Codigo(340);
        pub const NON_BOOL_NEGATION_EXPRESSION: Codigo = Codigo(341);
        pub const NON_BOOL_OPERAND: Codigo = Codigo(342);
        pub const NON_CONSTANT_ANNOTATION_CONSTRUCTOR: Codigo = Codigo(343);
        pub const NON_CONSTANT_CASE_EXPRESSION: Codigo = Codigo(344);
        pub const NON_CONSTANT_CASE_EXPRESSION_FROM_DEFERRED_LIBRARY: Codigo = Codigo(345);
        pub const NON_CONSTANT_DEFAULT_VALUE: Codigo = Codigo(346);
        pub const NON_CONSTANT_DEFAULT_VALUE_FROM_DEFERRED_LIBRARY: Codigo = Codigo(347);
        pub const NON_CONSTANT_LIST_ELEMENT: Codigo = Codigo(348);
        pub const NON_CONSTANT_LIST_ELEMENT_FROM_DEFERRED_LIBRARY: Codigo = Codigo(349);
        pub const NON_CONSTANT_MAP_ELEMENT: Codigo = Codigo(350);
        pub const NON_CONSTANT_MAP_KEY: Codigo = Codigo(351);
        pub const NON_CONSTANT_MAP_KEY_FROM_DEFERRED_LIBRARY: Codigo = Codigo(352);
        pub const NON_CONSTANT_MAP_PATTERN_KEY: Codigo = Codigo(353);
        pub const NON_CONSTANT_MAP_VALUE: Codigo = Codigo(354);
        pub const NON_CONSTANT_MAP_VALUE_FROM_DEFERRED_LIBRARY: Codigo = Codigo(355);
        pub const NON_CONSTANT_RECORD_FIELD: Codigo = Codigo(356);
        pub const NON_CONSTANT_RECORD_FIELD_FROM_DEFERRED_LIBRARY: Codigo = Codigo(357);
        pub const NON_CONSTANT_RELATIONAL_PATTERN_EXPRESSION: Codigo = Codigo(358);
        pub const NON_CONSTANT_SET_ELEMENT: Codigo = Codigo(359);
        pub const NON_CONST_GENERATIVE_ENUM_CONSTRUCTOR: Codigo = Codigo(360);
        pub const NON_CONST_MAP_AS_EXPRESSION_STATEMENT: Codigo = Codigo(361);
        pub const NON_COVARIANT_TYPE_PARAMETER_POSITION_IN_REPRESENTATION_TYPE: Codigo = Codigo(362);
        pub const NON_EXHAUSTIVE_SWITCH_EXPRESSION: Codigo = Codigo(363);
        pub const NON_EXHAUSTIVE_SWITCH_STATEMENT: Codigo = Codigo(364);
        pub const NON_FINAL_FIELD_IN_ENUM: Codigo = Codigo(365);
        pub const NON_GENERATIVE_CONSTRUCTOR: Codigo = Codigo(366);
        pub const NON_GENERATIVE_IMPLICIT_CONSTRUCTOR: Codigo = Codigo(367);
        pub const NON_SYNC_FACTORY: Codigo = Codigo(368);
        pub const NON_TYPE_AS_TYPE_ARGUMENT: Codigo = Codigo(369);
        pub const NON_TYPE_IN_CATCH_CLAUSE: Codigo = Codigo(370);
        pub const NON_VOID_RETURN_FOR_OPERATOR: Codigo = Codigo(371);
        pub const NON_VOID_RETURN_FOR_SETTER: Codigo = Codigo(372);
        pub const NOT_ASSIGNED_POTENTIALLY_NON_NULLABLE_LOCAL_VARIABLE: Codigo = Codigo(373);
        pub const NOT_A_TYPE: Codigo = Codigo(374);
        pub const NOT_BINARY_OPERATOR: Codigo = Codigo(375);
        pub const NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_PLURAL: Codigo = Codigo(376);
        pub const NOT_ENOUGH_POSITIONAL_ARGUMENTS_NAME_SINGULAR: Codigo = Codigo(377);
        pub const NOT_ENOUGH_POSITIONAL_ARGUMENTS_PLURAL: Codigo = Codigo(378);
        pub const NOT_ENOUGH_POSITIONAL_ARGUMENTS_SINGULAR: Codigo = Codigo(379);
        pub const NOT_INITIALIZED_NON_NULLABLE_INSTANCE_FIELD: Codigo = Codigo(380);
        pub const NOT_INITIALIZED_NON_NULLABLE_INSTANCE_FIELD_CONSTRUCTOR: Codigo = Codigo(381);
        pub const NOT_INITIALIZED_NON_NULLABLE_VARIABLE: Codigo = Codigo(382);
        pub const NOT_INSTANTIATED_BOUND: Codigo = Codigo(383);
        pub const NOT_ITERABLE_SPREAD: Codigo = Codigo(384);
        pub const NOT_MAP_SPREAD: Codigo = Codigo(385);
        pub const NOT_NULL_AWARE_NULL_SPREAD: Codigo = Codigo(386);
        pub const NO_ANNOTATION_CONSTRUCTOR_ARGUMENTS: Codigo = Codigo(387);
        pub const NO_COMBINED_SUPER_SIGNATURE: Codigo = Codigo(388);
        pub const NO_DEFAULT_SUPER_CONSTRUCTOR_EXPLICIT: Codigo = Codigo(389);
        pub const NO_DEFAULT_SUPER_CONSTRUCTOR_IMPLICIT: Codigo = Codigo(390);
        pub const NO_GENERATIVE_CONSTRUCTORS_IN_SUPERCLASS: Codigo = Codigo(391);
        pub const NULLABLE_TYPE_IN_EXTENDS_CLAUSE: Codigo = Codigo(392);
        pub const NULLABLE_TYPE_IN_IMPLEMENTS_CLAUSE: Codigo = Codigo(393);
        pub const NULLABLE_TYPE_IN_ON_CLAUSE: Codigo = Codigo(394);
        pub const NULLABLE_TYPE_IN_WITH_CLAUSE: Codigo = Codigo(395);
        pub const OBJECT_CANNOT_EXTEND_ANOTHER_CLASS: Codigo = Codigo(396);
        pub const OBSOLETE_COLON_FOR_DEFAULT_VALUE: Codigo = Codigo(397);
        pub const ON_REPEATED: Codigo = Codigo(398);
        pub const OPTIONAL_PARAMETER_IN_OPERATOR: Codigo = Codigo(399);
        pub const PART_OF_DIFFERENT_LIBRARY: Codigo = Codigo(400);
        pub const PART_OF_NON_PART: Codigo = Codigo(401);
        pub const PART_OF_UNNAMED_LIBRARY: Codigo = Codigo(402);
        pub const PATTERN_ASSIGNMENT_NOT_LOCAL_VARIABLE: Codigo = Codigo(403);
        pub const PATTERN_CONSTANT_FROM_DEFERRED_LIBRARY: Codigo = Codigo(404);
        pub const PATTERN_TYPE_MISMATCH_IN_IRREFUTABLE_CONTEXT: Codigo = Codigo(405);
        pub const PATTERN_VARIABLE_ASSIGNMENT_INSIDE_GUARD: Codigo = Codigo(406);
        pub const PATTERN_VARIABLE_SHARED_CASE_SCOPE_DIFFERENT_FINALITY_OR_TYPE: Codigo = Codigo(407);
        pub const PATTERN_VARIABLE_SHARED_CASE_SCOPE_HAS_LABEL: Codigo = Codigo(408);
        pub const PATTERN_VARIABLE_SHARED_CASE_SCOPE_NOT_ALL_CASES: Codigo = Codigo(409);
        pub const POSITIONAL_FIELD_IN_OBJECT_PATTERN: Codigo = Codigo(410);
        pub const POSITIONAL_SUPER_FORMAL_PARAMETER_WITH_POSITIONAL_ARGUMENT: Codigo = Codigo(411);
        pub const PREFIX_COLLIDES_WITH_TOP_LEVEL_MEMBER: Codigo = Codigo(412);
        pub const PREFIX_IDENTIFIER_NOT_FOLLOWED_BY_DOT: Codigo = Codigo(413);
        pub const PREFIX_SHADOWED_BY_LOCAL_DECLARATION: Codigo = Codigo(414);
        pub const PRIVATE_COLLISION_IN_MIXIN_APPLICATION: Codigo = Codigo(415);
        pub const PRIVATE_OPTIONAL_PARAMETER: Codigo = Codigo(416);
        pub const PRIVATE_SETTER: Codigo = Codigo(417);
        pub const READ_POTENTIALLY_UNASSIGNED_FINAL: Codigo = Codigo(418);
        pub const RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA: Codigo = Codigo(419);
        pub const RECURSIVE_COMPILE_TIME_CONSTANT: Codigo = Codigo(420);
        pub const RECURSIVE_CONSTANT_CONSTRUCTOR: Codigo = Codigo(421);
        pub const RECURSIVE_CONSTRUCTOR_REDIRECT: Codigo = Codigo(422);
        pub const RECURSIVE_FACTORY_REDIRECT: Codigo = Codigo(423);
        pub const RECURSIVE_INTERFACE_INHERITANCE: Codigo = Codigo(424);
        pub const RECURSIVE_INTERFACE_INHERITANCE_EXTENDS: Codigo = Codigo(425);
        pub const RECURSIVE_INTERFACE_INHERITANCE_IMPLEMENTS: Codigo = Codigo(426);
        pub const RECURSIVE_INTERFACE_INHERITANCE_ON: Codigo = Codigo(427);
        pub const RECURSIVE_INTERFACE_INHERITANCE_WITH: Codigo = Codigo(428);
        pub const REDIRECT_GENERATIVE_TO_MISSING_CONSTRUCTOR: Codigo = Codigo(429);
        pub const REDIRECT_GENERATIVE_TO_NON_GENERATIVE_CONSTRUCTOR: Codigo = Codigo(430);
        pub const REDIRECT_TO_ABSTRACT_CLASS_CONSTRUCTOR: Codigo = Codigo(431);
        pub const REDIRECT_TO_INVALID_FUNCTION_TYPE: Codigo = Codigo(432);
        pub const REDIRECT_TO_INVALID_RETURN_TYPE: Codigo = Codigo(433);
        pub const REDIRECT_TO_MISSING_CONSTRUCTOR: Codigo = Codigo(434);
        pub const REDIRECT_TO_NON_CLASS: Codigo = Codigo(435);
        pub const REDIRECT_TO_NON_CONST_CONSTRUCTOR: Codigo = Codigo(436);
        pub const REDIRECT_TO_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER: Codigo = Codigo(437);
        pub const REFERENCED_BEFORE_DECLARATION: Codigo = Codigo(438);
        pub const REFUTABLE_PATTERN_IN_IRREFUTABLE_CONTEXT: Codigo = Codigo(439);
        pub const RELATIONAL_PATTERN_OPERAND_TYPE_NOT_ASSIGNABLE: Codigo = Codigo(440);
        pub const RELATIONAL_PATTERN_OPERATOR_RETURN_TYPE_NOT_ASSIGNABLE_TO_BOOL: Codigo = Codigo(441);
        pub const REST_ELEMENT_IN_MAP_PATTERN: Codigo = Codigo(442);
        pub const RETHROW_OUTSIDE_CATCH: Codigo = Codigo(443);
        pub const RETURN_IN_GENERATIVE_CONSTRUCTOR: Codigo = Codigo(444);
        pub const RETURN_IN_GENERATOR: Codigo = Codigo(445);
        pub const RETURN_OF_INVALID_TYPE_FROM_CLOSURE: Codigo = Codigo(446);
        pub const RETURN_OF_INVALID_TYPE_FROM_CONSTRUCTOR: Codigo = Codigo(447);
        pub const RETURN_OF_INVALID_TYPE_FROM_FUNCTION: Codigo = Codigo(448);
        pub const RETURN_OF_INVALID_TYPE_FROM_METHOD: Codigo = Codigo(449);
        pub const RETURN_WITHOUT_VALUE: Codigo = Codigo(450);
        pub const SEALED_CLASS_SUBTYPE_OUTSIDE_OF_LIBRARY: Codigo = Codigo(451);
        pub const SET_ELEMENT_FROM_DEFERRED_LIBRARY: Codigo = Codigo(452);
        pub const SET_ELEMENT_TYPE_NOT_ASSIGNABLE: Codigo = Codigo(453);
        pub const SHARED_DEFERRED_PREFIX: Codigo = Codigo(454);
        pub const SPREAD_EXPRESSION_FROM_DEFERRED_LIBRARY: Codigo = Codigo(455);
        pub const STATIC_ACCESS_TO_INSTANCE_MEMBER: Codigo = Codigo(456);
        pub const SUBTYPE_OF_BASE_IS_NOT_BASE_FINAL_OR_SEALED: Codigo = Codigo(457);
        pub const SUBTYPE_OF_FINAL_IS_NOT_BASE_FINAL_OR_SEALED: Codigo = Codigo(458);
        pub const SUPER_FORMAL_PARAMETER_TYPE_IS_NOT_SUBTYPE_OF_ASSOCIATED: Codigo = Codigo(459);
        pub const SUPER_FORMAL_PARAMETER_WITHOUT_ASSOCIATED_NAMED: Codigo = Codigo(460);
        pub const SUPER_FORMAL_PARAMETER_WITHOUT_ASSOCIATED_POSITIONAL: Codigo = Codigo(461);
        pub const SUPER_INITIALIZER_IN_OBJECT: Codigo = Codigo(462);
        pub const SUPER_INVOCATION_NOT_LAST: Codigo = Codigo(463);
        pub const SUPER_IN_ENUM_CONSTRUCTOR: Codigo = Codigo(464);
        pub const SUPER_IN_EXTENSION: Codigo = Codigo(465);
        pub const SUPER_IN_EXTENSION_TYPE: Codigo = Codigo(466);
        pub const SUPER_IN_INVALID_CONTEXT: Codigo = Codigo(467);
        pub const SUPER_IN_REDIRECTING_CONSTRUCTOR: Codigo = Codigo(468);
        pub const SWITCH_CASE_COMPLETES_NORMALLY: Codigo = Codigo(469);
        pub const TEAROFF_OF_GENERATIVE_CONSTRUCTOR_OF_ABSTRACT_CLASS: Codigo = Codigo(470);
        pub const THROW_OF_INVALID_TYPE: Codigo = Codigo(471);
        pub const TOP_LEVEL_CYCLE: Codigo = Codigo(472);
        pub const TYPE_ALIAS_CANNOT_REFERENCE_ITSELF: Codigo = Codigo(473);
        pub const TYPE_ANNOTATION_DEFERRED_CLASS: Codigo = Codigo(474);
        pub const TYPE_ARGUMENT_NOT_MATCHING_BOUNDS: Codigo = Codigo(475);
        pub const TYPE_PARAMETER_REFERENCED_BY_STATIC: Codigo = Codigo(476);
        pub const TYPE_PARAMETER_SUPERTYPE_OF_ITS_BOUND: Codigo = Codigo(477);
        pub const TYPE_TEST_WITH_NON_TYPE: Codigo = Codigo(478);
        pub const TYPE_TEST_WITH_UNDEFINED_NAME: Codigo = Codigo(479);
        pub const UNCHECKED_INVOCATION_OF_NULLABLE_VALUE: Codigo = Codigo(480);
        pub const UNCHECKED_METHOD_INVOCATION_OF_NULLABLE_VALUE: Codigo = Codigo(481);
        pub const UNCHECKED_OPERATOR_INVOCATION_OF_NULLABLE_VALUE: Codigo = Codigo(482);
        pub const UNCHECKED_PROPERTY_ACCESS_OF_NULLABLE_VALUE: Codigo = Codigo(483);
        pub const UNCHECKED_USE_OF_NULLABLE_VALUE_AS_CONDITION: Codigo = Codigo(484);
        pub const UNCHECKED_USE_OF_NULLABLE_VALUE_AS_ITERATOR: Codigo = Codigo(485);
        pub const UNCHECKED_USE_OF_NULLABLE_VALUE_IN_SPREAD: Codigo = Codigo(486);
        pub const UNCHECKED_USE_OF_NULLABLE_VALUE_IN_YIELD_EACH: Codigo = Codigo(487);
        pub const UNDEFINED_ANNOTATION: Codigo = Codigo(488);
        pub const UNDEFINED_CLASS: Codigo = Codigo(489);
        pub const UNDEFINED_CLASS_BOOLEAN: Codigo = Codigo(490);
        pub const UNDEFINED_CONSTRUCTOR_IN_INITIALIZER: Codigo = Codigo(491);
        pub const UNDEFINED_CONSTRUCTOR_IN_INITIALIZER_DEFAULT: Codigo = Codigo(492);
        pub const UNDEFINED_ENUM_CONSTANT: Codigo = Codigo(493);
        pub const UNDEFINED_ENUM_CONSTRUCTOR_NAMED: Codigo = Codigo(494);
        pub const UNDEFINED_ENUM_CONSTRUCTOR_UNNAMED: Codigo = Codigo(495);
        pub const UNDEFINED_EXTENSION_GETTER: Codigo = Codigo(496);
        pub const UNDEFINED_EXTENSION_METHOD: Codigo = Codigo(497);
        pub const UNDEFINED_EXTENSION_OPERATOR: Codigo = Codigo(498);
        pub const UNDEFINED_EXTENSION_SETTER: Codigo = Codigo(499);
        pub const UNDEFINED_FUNCTION: Codigo = Codigo(500);
        pub const UNDEFINED_GETTER: Codigo = Codigo(501);
        pub const UNDEFINED_GETTER_ON_FUNCTION_TYPE: Codigo = Codigo(502);
        pub const UNDEFINED_IDENTIFIER: Codigo = Codigo(503);
        pub const UNDEFINED_IDENTIFIER_AWAIT: Codigo = Codigo(504);
        pub const UNDEFINED_METHOD: Codigo = Codigo(505);
        pub const UNDEFINED_METHOD_ON_FUNCTION_TYPE: Codigo = Codigo(506);
        pub const UNDEFINED_NAMED_PARAMETER: Codigo = Codigo(507);
        pub const UNDEFINED_OPERATOR: Codigo = Codigo(508);
        pub const UNDEFINED_PREFIXED_NAME: Codigo = Codigo(509);
        pub const UNDEFINED_SETTER: Codigo = Codigo(510);
        pub const UNDEFINED_SETTER_ON_FUNCTION_TYPE: Codigo = Codigo(511);
        pub const UNDEFINED_SUPER_GETTER: Codigo = Codigo(512);
        pub const UNDEFINED_SUPER_METHOD: Codigo = Codigo(513);
        pub const UNDEFINED_SUPER_OPERATOR: Codigo = Codigo(514);
        pub const UNDEFINED_SUPER_SETTER: Codigo = Codigo(515);
        pub const UNQUALIFIED_REFERENCE_TO_NON_LOCAL_STATIC_MEMBER: Codigo = Codigo(516);
        pub const UNQUALIFIED_REFERENCE_TO_STATIC_MEMBER_OF_EXTENDED_TYPE: Codigo = Codigo(517);
        pub const URI_DOES_NOT_EXIST: Codigo = Codigo(518);
        pub const URI_HAS_NOT_BEEN_GENERATED: Codigo = Codigo(519);
        pub const URI_WITH_INTERPOLATION: Codigo = Codigo(520);
        pub const USE_OF_NATIVE_EXTENSION: Codigo = Codigo(521);
        pub const USE_OF_VOID_RESULT: Codigo = Codigo(522);
        pub const VALUES_DECLARATION_IN_ENUM: Codigo = Codigo(523);
        pub const VARIABLE_TYPE_MISMATCH: Codigo = Codigo(524);
        pub const WRONG_EXPLICIT_TYPE_PARAMETER_VARIANCE_IN_SUPERINTERFACE: Codigo = Codigo(525);
        pub const WRONG_NUMBER_OF_PARAMETERS_FOR_OPERATOR: Codigo = Codigo(526);
        pub const WRONG_NUMBER_OF_PARAMETERS_FOR_OPERATOR_MINUS: Codigo = Codigo(527);
        pub const WRONG_NUMBER_OF_PARAMETERS_FOR_SETTER: Codigo = Codigo(528);
        pub const WRONG_NUMBER_OF_TYPE_ARGUMENTS: Codigo = Codigo(529);
        pub const WRONG_NUMBER_OF_TYPE_ARGUMENTS_ANONYMOUS_FUNCTION: Codigo = Codigo(530);
        pub const WRONG_NUMBER_OF_TYPE_ARGUMENTS_CONSTRUCTOR: Codigo = Codigo(531);
        pub const WRONG_NUMBER_OF_TYPE_ARGUMENTS_ENUM: Codigo = Codigo(532);
        pub const WRONG_NUMBER_OF_TYPE_ARGUMENTS_EXTENSION: Codigo = Codigo(533);
        pub const WRONG_NUMBER_OF_TYPE_ARGUMENTS_FUNCTION: Codigo = Codigo(534);
        pub const WRONG_NUMBER_OF_TYPE_ARGUMENTS_METHOD: Codigo = Codigo(535);
        pub const WRONG_TYPE_PARAMETER_VARIANCE_IN_SUPERINTERFACE: Codigo = Codigo(536);
        pub const WRONG_TYPE_PARAMETER_VARIANCE_POSITION: Codigo = Codigo(537);
        pub const YIELD_EACH_IN_NON_GENERATOR: Codigo = Codigo(538);
        pub const YIELD_EACH_OF_INVALID_TYPE: Codigo = Codigo(539);
        pub const YIELD_IN_NON_GENERATOR: Codigo = Codigo(540);
        pub const YIELD_OF_INVALID_TYPE: Codigo = Codigo(541);
    }
    /// `StaticWarningCode`.
    pub mod static_warning {
        use crate::Codigo;
        pub const DEAD_NULL_AWARE_EXPRESSION: Codigo = Codigo(542);
        pub const INVALID_NULL_AWARE_OPERATOR: Codigo = Codigo(543);
        pub const INVALID_NULL_AWARE_OPERATOR_AFTER_SHORT_CIRCUIT: Codigo = Codigo(544);
        pub const MISSING_ENUM_CONSTANT_IN_SWITCH: Codigo = Codigo(545);
        pub const UNNECESSARY_NON_NULL_ASSERTION: Codigo = Codigo(546);
        pub const UNNECESSARY_NULL_ASSERT_PATTERN: Codigo = Codigo(547);
        pub const UNNECESSARY_NULL_CHECK_PATTERN: Codigo = Codigo(548);
    }
    /// `WarningCode`.
    pub mod warning {
        use crate::Codigo;
        pub const ARGUMENT_TYPE_NOT_ASSIGNABLE_TO_ERROR_HANDLER: Codigo = Codigo(549);
        pub const ASSIGNMENT_OF_DO_NOT_STORE: Codigo = Codigo(550);
        pub const BODY_MIGHT_COMPLETE_NORMALLY_CATCH_ERROR: Codigo = Codigo(551);
        pub const BODY_MIGHT_COMPLETE_NORMALLY_NULLABLE: Codigo = Codigo(552);
        pub const CAST_FROM_NULLABLE_ALWAYS_FAILS: Codigo = Codigo(553);
        pub const CAST_FROM_NULL_ALWAYS_FAILS: Codigo = Codigo(554);
        pub const CONSTANT_PATTERN_NEVER_MATCHES_VALUE_TYPE: Codigo = Codigo(555);
        pub const DEAD_CODE: Codigo = Codigo(556);
        pub const DEAD_CODE_CATCH_FOLLOWING_CATCH: Codigo = Codigo(557);
        pub const DEAD_CODE_LATE_WILDCARD_VARIABLE_INITIALIZER: Codigo = Codigo(558);
        pub const DEAD_CODE_ON_CATCH_SUBTYPE: Codigo = Codigo(559);
        pub const DEPRECATED_EXPORT_USE: Codigo = Codigo(560);
        pub const DEPRECATED_EXTENDS_FUNCTION: Codigo = Codigo(561);
        pub const DEPRECATED_IMPLEMENTS_FUNCTION: Codigo = Codigo(562);
        pub const DEPRECATED_MIXIN_FUNCTION: Codigo = Codigo(563);
        pub const DEPRECATED_NEW_IN_COMMENT_REFERENCE: Codigo = Codigo(564);
        pub const DOC_DIRECTIVE_ARGUMENT_WRONG_FORMAT: Codigo = Codigo(565);
        pub const DOC_DIRECTIVE_HAS_EXTRA_ARGUMENTS: Codigo = Codigo(566);
        pub const DOC_DIRECTIVE_HAS_UNEXPECTED_NAMED_ARGUMENT: Codigo = Codigo(567);
        pub const DOC_DIRECTIVE_MISSING_CLOSING_BRACE: Codigo = Codigo(568);
        pub const DOC_DIRECTIVE_MISSING_CLOSING_TAG: Codigo = Codigo(569);
        pub const DOC_DIRECTIVE_MISSING_ONE_ARGUMENT: Codigo = Codigo(570);
        pub const DOC_DIRECTIVE_MISSING_OPENING_TAG: Codigo = Codigo(571);
        pub const DOC_DIRECTIVE_MISSING_THREE_ARGUMENTS: Codigo = Codigo(572);
        pub const DOC_DIRECTIVE_MISSING_TWO_ARGUMENTS: Codigo = Codigo(573);
        pub const DOC_DIRECTIVE_UNKNOWN: Codigo = Codigo(574);
        pub const DOC_IMPORT_CANNOT_BE_DEFERRED: Codigo = Codigo(575);
        pub const DOC_IMPORT_CANNOT_HAVE_CONFIGURATIONS: Codigo = Codigo(576);
        pub const DUPLICATE_EXPORT: Codigo = Codigo(577);
        pub const DUPLICATE_HIDDEN_NAME: Codigo = Codigo(578);
        pub const DUPLICATE_IGNORE: Codigo = Codigo(579);
        pub const DUPLICATE_IMPORT: Codigo = Codigo(580);
        pub const DUPLICATE_SHOWN_NAME: Codigo = Codigo(581);
        pub const EQUAL_ELEMENTS_IN_SET: Codigo = Codigo(582);
        pub const EQUAL_KEYS_IN_MAP: Codigo = Codigo(583);
        pub const INFERENCE_FAILURE_ON_COLLECTION_LITERAL: Codigo = Codigo(584);
        pub const INFERENCE_FAILURE_ON_FUNCTION_INVOCATION: Codigo = Codigo(585);
        pub const INFERENCE_FAILURE_ON_FUNCTION_RETURN_TYPE: Codigo = Codigo(586);
        pub const INFERENCE_FAILURE_ON_GENERIC_INVOCATION: Codigo = Codigo(587);
        pub const INFERENCE_FAILURE_ON_INSTANCE_CREATION: Codigo = Codigo(588);
        pub const INFERENCE_FAILURE_ON_UNINITIALIZED_VARIABLE: Codigo = Codigo(589);
        pub const INFERENCE_FAILURE_ON_UNTYPED_PARAMETER: Codigo = Codigo(590);
        pub const INVALID_ANNOTATION_TARGET: Codigo = Codigo(591);
        pub const INVALID_EXPORT_OF_INTERNAL_ELEMENT: Codigo = Codigo(592);
        pub const INVALID_EXPORT_OF_INTERNAL_ELEMENT_INDIRECTLY: Codigo = Codigo(593);
        pub const INVALID_FACTORY_METHOD_DECL: Codigo = Codigo(594);
        pub const INVALID_FACTORY_METHOD_IMPL: Codigo = Codigo(595);
        pub const INVALID_INTERNAL_ANNOTATION: Codigo = Codigo(596);
        pub const INVALID_LANGUAGE_VERSION_OVERRIDE_AT_SIGN: Codigo = Codigo(597);
        pub const INVALID_LANGUAGE_VERSION_OVERRIDE_EQUALS: Codigo = Codigo(598);
        pub const INVALID_LANGUAGE_VERSION_OVERRIDE_GREATER: Codigo = Codigo(599);
        pub const INVALID_LANGUAGE_VERSION_OVERRIDE_LOCATION: Codigo = Codigo(600);
        pub const INVALID_LANGUAGE_VERSION_OVERRIDE_LOWER_CASE: Codigo = Codigo(601);
        pub const INVALID_LANGUAGE_VERSION_OVERRIDE_NUMBER: Codigo = Codigo(602);
        pub const INVALID_LANGUAGE_VERSION_OVERRIDE_PREFIX: Codigo = Codigo(603);
        pub const INVALID_LANGUAGE_VERSION_OVERRIDE_TRAILING_CHARACTERS: Codigo = Codigo(604);
        pub const INVALID_LANGUAGE_VERSION_OVERRIDE_TWO_SLASHES: Codigo = Codigo(605);
        pub const INVALID_LITERAL_ANNOTATION: Codigo = Codigo(606);
        pub const INVALID_NON_VIRTUAL_ANNOTATION: Codigo = Codigo(607);
        pub const INVALID_OVERRIDE_OF_NON_VIRTUAL_MEMBER: Codigo = Codigo(608);
        pub const INVALID_REOPEN_ANNOTATION: Codigo = Codigo(609);
        pub const INVALID_REQUIRED_NAMED_PARAM: Codigo = Codigo(610);
        pub const INVALID_REQUIRED_OPTIONAL_POSITIONAL_PARAM: Codigo = Codigo(611);
        pub const INVALID_REQUIRED_POSITIONAL_PARAM: Codigo = Codigo(612);
        pub const INVALID_USE_OF_INTERNAL_MEMBER: Codigo = Codigo(613);
        pub const INVALID_USE_OF_PROTECTED_MEMBER: Codigo = Codigo(614);
        pub const INVALID_USE_OF_VISIBLE_FOR_OVERRIDING_MEMBER: Codigo = Codigo(615);
        pub const INVALID_USE_OF_VISIBLE_FOR_TEMPLATE_MEMBER: Codigo = Codigo(616);
        pub const INVALID_USE_OF_VISIBLE_FOR_TESTING_MEMBER: Codigo = Codigo(617);
        pub const INVALID_VISIBILITY_ANNOTATION: Codigo = Codigo(618);
        pub const INVALID_VISIBLE_FOR_OVERRIDING_ANNOTATION: Codigo = Codigo(619);
        pub const INVALID_VISIBLE_OUTSIDE_TEMPLATE_ANNOTATION: Codigo = Codigo(620);
        pub const MACRO_WARNING: Codigo = Codigo(621);
        pub const MISSING_OVERRIDE_OF_MUST_BE_OVERRIDDEN_ONE: Codigo = Codigo(622);
        pub const MISSING_OVERRIDE_OF_MUST_BE_OVERRIDDEN_THREE_PLUS: Codigo = Codigo(623);
        pub const MISSING_OVERRIDE_OF_MUST_BE_OVERRIDDEN_TWO: Codigo = Codigo(624);
        pub const MISSING_REQUIRED_PARAM: Codigo = Codigo(625);
        pub const MISSING_REQUIRED_PARAM_WITH_DETAILS: Codigo = Codigo(626);
        pub const MIXIN_ON_SEALED_CLASS: Codigo = Codigo(627);
        pub const MUST_BE_IMMUTABLE: Codigo = Codigo(628);
        pub const MUST_CALL_SUPER: Codigo = Codigo(629);
        pub const NON_CONST_ARGUMENT_FOR_CONST_PARAMETER: Codigo = Codigo(630);
        pub const NON_CONST_CALL_TO_LITERAL_CONSTRUCTOR: Codigo = Codigo(631);
        pub const NON_CONST_CALL_TO_LITERAL_CONSTRUCTOR_USING_NEW: Codigo = Codigo(632);
        pub const NON_NULLABLE_EQUALS_PARAMETER: Codigo = Codigo(633);
        pub const NULLABLE_TYPE_IN_CATCH_CLAUSE: Codigo = Codigo(634);
        pub const NULL_ARGUMENT_TO_NON_NULL_TYPE: Codigo = Codigo(635);
        pub const NULL_CHECK_ALWAYS_FAILS: Codigo = Codigo(636);
        pub const OVERRIDE_ON_NON_OVERRIDING_FIELD: Codigo = Codigo(637);
        pub const OVERRIDE_ON_NON_OVERRIDING_GETTER: Codigo = Codigo(638);
        pub const OVERRIDE_ON_NON_OVERRIDING_METHOD: Codigo = Codigo(639);
        pub const OVERRIDE_ON_NON_OVERRIDING_SETTER: Codigo = Codigo(640);
        pub const PATTERN_NEVER_MATCHES_VALUE_TYPE: Codigo = Codigo(641);
        pub const RECEIVER_OF_TYPE_NEVER: Codigo = Codigo(642);
        pub const REDECLARE_ON_NON_REDECLARING_MEMBER: Codigo = Codigo(643);
        pub const REMOVED_LINT_USE: Codigo = Codigo(644);
        pub const REPLACED_LINT_USE: Codigo = Codigo(645);
        pub const RETURN_OF_DO_NOT_STORE: Codigo = Codigo(646);
        pub const RETURN_OF_INVALID_TYPE_FROM_CATCH_ERROR: Codigo = Codigo(647);
        pub const RETURN_TYPE_INVALID_FOR_CATCH_ERROR: Codigo = Codigo(648);
        pub const SDK_VERSION_CONSTRUCTOR_TEAROFFS: Codigo = Codigo(649);
        pub const SDK_VERSION_GT_GT_GT_OPERATOR: Codigo = Codigo(650);
        pub const SDK_VERSION_SINCE: Codigo = Codigo(651);
        pub const STRICT_RAW_TYPE: Codigo = Codigo(652);
        pub const SUBTYPE_OF_SEALED_CLASS: Codigo = Codigo(653);
        pub const TEXT_DIRECTION_CODE_POINT_IN_COMMENT: Codigo = Codigo(654);
        pub const TEXT_DIRECTION_CODE_POINT_IN_LITERAL: Codigo = Codigo(655);
        pub const TYPE_CHECK_IS_NOT_NULL: Codigo = Codigo(656);
        pub const TYPE_CHECK_IS_NULL: Codigo = Codigo(657);
        pub const UNDEFINED_HIDDEN_NAME: Codigo = Codigo(658);
        pub const UNDEFINED_REFERENCED_PARAMETER: Codigo = Codigo(659);
        pub const UNDEFINED_SHOWN_NAME: Codigo = Codigo(660);
        pub const UNIGNORABLE_IGNORE: Codigo = Codigo(661);
        pub const UNNECESSARY_CAST: Codigo = Codigo(662);
        pub const UNNECESSARY_CAST_PATTERN: Codigo = Codigo(663);
        pub const UNNECESSARY_FINAL: Codigo = Codigo(664);
        pub const UNNECESSARY_IGNORE: Codigo = Codigo(665);
        pub const UNNECESSARY_NAN_COMPARISON_FALSE: Codigo = Codigo(666);
        pub const UNNECESSARY_NAN_COMPARISON_TRUE: Codigo = Codigo(667);
        pub const UNNECESSARY_NO_SUCH_METHOD: Codigo = Codigo(668);
        pub const UNNECESSARY_NULL_COMPARISON_ALWAYS_NULL_FALSE: Codigo = Codigo(669);
        pub const UNNECESSARY_NULL_COMPARISON_ALWAYS_NULL_TRUE: Codigo = Codigo(670);
        pub const UNNECESSARY_NULL_COMPARISON_NEVER_NULL_FALSE: Codigo = Codigo(671);
        pub const UNNECESSARY_NULL_COMPARISON_NEVER_NULL_TRUE: Codigo = Codigo(672);
        pub const UNNECESSARY_QUESTION_MARK: Codigo = Codigo(673);
        pub const UNNECESSARY_SET_LITERAL: Codigo = Codigo(674);
        pub const UNNECESSARY_TYPE_CHECK_FALSE: Codigo = Codigo(675);
        pub const UNNECESSARY_TYPE_CHECK_TRUE: Codigo = Codigo(676);
        pub const UNNECESSARY_WILDCARD_PATTERN: Codigo = Codigo(677);
        pub const UNREACHABLE_SWITCH_CASE: Codigo = Codigo(678);
        pub const UNREACHABLE_SWITCH_DEFAULT: Codigo = Codigo(679);
        pub const UNUSED_CATCH_CLAUSE: Codigo = Codigo(680);
        pub const UNUSED_CATCH_STACK: Codigo = Codigo(681);
        pub const UNUSED_ELEMENT: Codigo = Codigo(682);
        pub const UNUSED_ELEMENT_PARAMETER: Codigo = Codigo(683);
        pub const UNUSED_FIELD: Codigo = Codigo(684);
        pub const UNUSED_IMPORT: Codigo = Codigo(685);
        pub const UNUSED_LABEL: Codigo = Codigo(686);
        pub const UNUSED_LOCAL_VARIABLE: Codigo = Codigo(687);
        pub const UNUSED_RESULT: Codigo = Codigo(688);
        pub const UNUSED_RESULT_WITH_MESSAGE: Codigo = Codigo(689);
        pub const UNUSED_SHOWN_NAME: Codigo = Codigo(690);
        pub const URI_DOES_NOT_EXIST_IN_DOC_IMPORT: Codigo = Codigo(691);
        pub const INVALID_USE_OF_DO_NOT_SUBMIT_MEMBER: Codigo = Codigo(692);
    }
    /// `HintCode`.
    pub mod hint {
        use crate::Codigo;
        pub const DEPRECATED_COLON_FOR_DEFAULT_VALUE: Codigo = Codigo(693);
        pub const DEPRECATED_MEMBER_USE: Codigo = Codigo(694);
        pub const DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE: Codigo = Codigo(695);
        pub const DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE_WITH_MESSAGE: Codigo = Codigo(696);
        pub const DEPRECATED_MEMBER_USE_WITH_MESSAGE: Codigo = Codigo(697);
        pub const IMPORT_DEFERRED_LIBRARY_WITH_LOAD_FUNCTION: Codigo = Codigo(698);
        pub const MACRO_INFO: Codigo = Codigo(699);
        pub const UNNECESSARY_IMPORT: Codigo = Codigo(700);
    }
    /// `FfiCode`.
    pub mod ffi {
        use crate::Codigo;
        pub const ABI_SPECIFIC_INTEGER_INVALID: Codigo = Codigo(701);
        pub const ABI_SPECIFIC_INTEGER_MAPPING_EXTRA: Codigo = Codigo(702);
        pub const ABI_SPECIFIC_INTEGER_MAPPING_MISSING: Codigo = Codigo(703);
        pub const ABI_SPECIFIC_INTEGER_MAPPING_UNSUPPORTED: Codigo = Codigo(704);
        pub const ADDRESS_POSITION: Codigo = Codigo(705);
        pub const ADDRESS_RECEIVER: Codigo = Codigo(706);
        pub const ANNOTATION_ON_POINTER_FIELD: Codigo = Codigo(707);
        pub const ARGUMENT_MUST_BE_A_CONSTANT: Codigo = Codigo(708);
        pub const ARGUMENT_MUST_BE_NATIVE: Codigo = Codigo(709);
        pub const COMPOUND_IMPLEMENTS_FINALIZABLE: Codigo = Codigo(710);
        pub const CREATION_OF_STRUCT_OR_UNION: Codigo = Codigo(711);
        pub const EMPTY_STRUCT: Codigo = Codigo(712);
        pub const EXTRA_ANNOTATION_ON_STRUCT_FIELD: Codigo = Codigo(713);
        pub const EXTRA_SIZE_ANNOTATION_CARRAY: Codigo = Codigo(714);
        pub const FFI_NATIVE_INVALID_DUPLICATE_DEFAULT_ASSET: Codigo = Codigo(715);
        pub const FFI_NATIVE_INVALID_MULTIPLE_ANNOTATIONS: Codigo = Codigo(716);
        pub const FFI_NATIVE_MUST_BE_EXTERNAL: Codigo = Codigo(717);
        pub const FFI_NATIVE_ONLY_CLASSES_EXTENDING_NATIVEFIELDWRAPPERCLASS1_CAN_BE_POINTER: Codigo = Codigo(718);
        pub const FFI_NATIVE_UNEXPECTED_NUMBER_OF_PARAMETERS: Codigo = Codigo(719);
        pub const FFI_NATIVE_UNEXPECTED_NUMBER_OF_PARAMETERS_WITH_RECEIVER: Codigo = Codigo(720);
        pub const FIELD_MUST_BE_EXTERNAL_IN_STRUCT: Codigo = Codigo(721);
        pub const GENERIC_STRUCT_SUBCLASS: Codigo = Codigo(722);
        pub const INVALID_EXCEPTION_VALUE: Codigo = Codigo(723);
        pub const INVALID_FIELD_TYPE_IN_STRUCT: Codigo = Codigo(724);
        pub const LEAF_CALL_MUST_NOT_RETURN_HANDLE: Codigo = Codigo(725);
        pub const LEAF_CALL_MUST_NOT_TAKE_HANDLE: Codigo = Codigo(726);
        pub const MISMATCHED_ANNOTATION_ON_STRUCT_FIELD: Codigo = Codigo(727);
        pub const MISSING_ANNOTATION_ON_STRUCT_FIELD: Codigo = Codigo(728);
        pub const MISSING_EXCEPTION_VALUE: Codigo = Codigo(729);
        pub const MISSING_FIELD_TYPE_IN_STRUCT: Codigo = Codigo(730);
        pub const MISSING_SIZE_ANNOTATION_CARRAY: Codigo = Codigo(731);
        pub const MUST_BE_A_NATIVE_FUNCTION_TYPE: Codigo = Codigo(732);
        pub const MUST_BE_A_SUBTYPE: Codigo = Codigo(733);
        pub const MUST_RETURN_VOID: Codigo = Codigo(734);
        pub const NATIVE_FIELD_INVALID_TYPE: Codigo = Codigo(735);
        pub const NATIVE_FIELD_MISSING_TYPE: Codigo = Codigo(736);
        pub const NATIVE_FIELD_NOT_STATIC: Codigo = Codigo(737);
        pub const NON_CONSTANT_TYPE_ARGUMENT: Codigo = Codigo(738);
        pub const NON_NATIVE_FUNCTION_TYPE_ARGUMENT_TO_POINTER: Codigo = Codigo(739);
        pub const NON_POSITIVE_ARRAY_DIMENSION: Codigo = Codigo(740);
        pub const NON_SIZED_TYPE_ARGUMENT: Codigo = Codigo(741);
        pub const PACKED_ANNOTATION: Codigo = Codigo(742);
        pub const PACKED_ANNOTATION_ALIGNMENT: Codigo = Codigo(743);
        pub const SIZE_ANNOTATION_DIMENSIONS: Codigo = Codigo(744);
        pub const SUBTYPE_OF_STRUCT_CLASS_IN_EXTENDS: Codigo = Codigo(745);
        pub const SUBTYPE_OF_STRUCT_CLASS_IN_IMPLEMENTS: Codigo = Codigo(746);
        pub const SUBTYPE_OF_STRUCT_CLASS_IN_WITH: Codigo = Codigo(747);
        pub const VARIABLE_LENGTH_ARRAY_NOT_LAST: Codigo = Codigo(748);
    }
    /// `ParserErrorCode`.
    pub mod parser {
        use crate::Codigo;
        pub const ABSTRACT_CLASS_MEMBER: Codigo = Codigo(749);
        pub const ABSTRACT_EXTERNAL_FIELD: Codigo = Codigo(750);
        pub const ABSTRACT_FINAL_BASE_CLASS: Codigo = Codigo(751);
        pub const ABSTRACT_FINAL_INTERFACE_CLASS: Codigo = Codigo(752);
        pub const ABSTRACT_LATE_FIELD: Codigo = Codigo(753);
        pub const ABSTRACT_SEALED_CLASS: Codigo = Codigo(754);
        pub const ABSTRACT_STATIC_FIELD: Codigo = Codigo(755);
        pub const ABSTRACT_STATIC_METHOD: Codigo = Codigo(756);
        pub const ANNOTATION_ON_TYPE_ARGUMENT: Codigo = Codigo(757);
        pub const ANNOTATION_SPACE_BEFORE_PARENTHESIS: Codigo = Codigo(758);
        pub const ANNOTATION_WITH_TYPE_ARGUMENTS: Codigo = Codigo(759);
        pub const ANNOTATION_WITH_TYPE_ARGUMENTS_UNINSTANTIATED: Codigo = Codigo(760);
        pub const ASYNC_KEYWORD_USED_AS_IDENTIFIER: Codigo = Codigo(761);
        pub const BASE_ENUM: Codigo = Codigo(762);
        pub const BINARY_OPERATOR_WRITTEN_OUT: Codigo = Codigo(763);
        pub const BREAK_OUTSIDE_OF_LOOP: Codigo = Codigo(764);
        pub const CATCH_SYNTAX: Codigo = Codigo(765);
        pub const CATCH_SYNTAX_EXTRA_PARAMETERS: Codigo = Codigo(766);
        pub const CLASS_IN_CLASS: Codigo = Codigo(767);
        pub const COLON_IN_PLACE_OF_IN: Codigo = Codigo(768);
        pub const CONFLICTING_MODIFIERS: Codigo = Codigo(769);
        pub const CONSTRUCTOR_WITH_RETURN_TYPE: Codigo = Codigo(770);
        pub const CONSTRUCTOR_WITH_TYPE_ARGUMENTS: Codigo = Codigo(771);
        pub const CONST_AND_FINAL: Codigo = Codigo(772);
        pub const CONST_CLASS: Codigo = Codigo(773);
        pub const CONST_CONSTRUCTOR_WITH_BODY: Codigo = Codigo(774);
        pub const CONST_FACTORY: Codigo = Codigo(775);
        pub const CONST_METHOD: Codigo = Codigo(776);
        pub const CONTINUE_OUTSIDE_OF_LOOP: Codigo = Codigo(777);
        pub const CONTINUE_WITHOUT_LABEL_IN_CASE: Codigo = Codigo(778);
        pub const COVARIANT_AND_STATIC: Codigo = Codigo(779);
        pub const COVARIANT_CONSTRUCTOR: Codigo = Codigo(780);
        pub const COVARIANT_MEMBER: Codigo = Codigo(781);
        pub const DECLARATION_NAMED_AUGMENTED_INSIDE_AUGMENTATION: Codigo = Codigo(782);
        pub const DEFAULT_IN_SWITCH_EXPRESSION: Codigo = Codigo(783);
        pub const DEFAULT_VALUE_IN_FUNCTION_TYPE: Codigo = Codigo(784);
        pub const DEFERRED_AFTER_PREFIX: Codigo = Codigo(785);
        pub const DIRECTIVE_AFTER_DECLARATION: Codigo = Codigo(786);
        pub const DUPLICATED_MODIFIER: Codigo = Codigo(787);
        pub const DUPLICATE_DEFERRED: Codigo = Codigo(788);
        pub const DUPLICATE_LABEL_IN_SWITCH_STATEMENT: Codigo = Codigo(789);
        pub const DUPLICATE_PREFIX: Codigo = Codigo(790);
        pub const EMPTY_ENUM_BODY: Codigo = Codigo(791);
        pub const EMPTY_RECORD_LITERAL_WITH_COMMA: Codigo = Codigo(792);
        pub const EMPTY_RECORD_TYPE_NAMED_FIELDS_LIST: Codigo = Codigo(793);
        pub const EMPTY_RECORD_TYPE_WITH_COMMA: Codigo = Codigo(794);
        pub const ENUM_IN_CLASS: Codigo = Codigo(795);
        pub const EQUALITY_CANNOT_BE_EQUALITY_OPERAND: Codigo = Codigo(796);
        pub const EXPECTED_CASE_OR_DEFAULT: Codigo = Codigo(797);
        pub const EXPECTED_CATCH_CLAUSE_BODY: Codigo = Codigo(798);
        pub const EXPECTED_CLASS_BODY: Codigo = Codigo(799);
        pub const EXPECTED_CLASS_MEMBER: Codigo = Codigo(800);
        pub const EXPECTED_ELSE_OR_COMMA: Codigo = Codigo(801);
        pub const EXPECTED_EXECUTABLE: Codigo = Codigo(802);
        pub const EXPECTED_EXTENSION_BODY: Codigo = Codigo(803);
        pub const EXPECTED_EXTENSION_TYPE_BODY: Codigo = Codigo(804);
        pub const EXPECTED_FINALLY_CLAUSE_BODY: Codigo = Codigo(805);
        pub const EXPECTED_IDENTIFIER_BUT_GOT_KEYWORD: Codigo = Codigo(806);
        pub const EXPECTED_INSTEAD: Codigo = Codigo(807);
        pub const EXPECTED_LIST_OR_MAP_LITERAL: Codigo = Codigo(808);
        pub const EXPECTED_MIXIN_BODY: Codigo = Codigo(809);
        pub const EXPECTED_NAMED_TYPE_EXTENDS: Codigo = Codigo(810);
        pub const EXPECTED_NAMED_TYPE_IMPLEMENTS: Codigo = Codigo(811);
        pub const EXPECTED_NAMED_TYPE_ON: Codigo = Codigo(812);
        pub const EXPECTED_NAMED_TYPE_WITH: Codigo = Codigo(813);
        pub const EXPECTED_REPRESENTATION_FIELD: Codigo = Codigo(814);
        pub const EXPECTED_REPRESENTATION_TYPE: Codigo = Codigo(815);
        pub const EXPECTED_STRING_LITERAL: Codigo = Codigo(816);
        pub const EXPECTED_SWITCH_EXPRESSION_BODY: Codigo = Codigo(817);
        pub const EXPECTED_SWITCH_STATEMENT_BODY: Codigo = Codigo(818);
        pub const EXPECTED_TOKEN: Codigo = Codigo(819);
        pub const EXPECTED_TRY_STATEMENT_BODY: Codigo = Codigo(820);
        pub const EXPECTED_TYPE_NAME: Codigo = Codigo(821);
        pub const EXPERIMENT_NOT_ENABLED: Codigo = Codigo(822);
        pub const EXPERIMENT_NOT_ENABLED_OFF_BY_DEFAULT: Codigo = Codigo(823);
        pub const EXPORT_DIRECTIVE_AFTER_PART_DIRECTIVE: Codigo = Codigo(824);
        pub const EXTENSION_AUGMENTATION_HAS_ON_CLAUSE: Codigo = Codigo(825);
        pub const EXTENSION_DECLARES_ABSTRACT_MEMBER: Codigo = Codigo(826);
        pub const EXTENSION_DECLARES_CONSTRUCTOR: Codigo = Codigo(827);
        pub const EXTENSION_DECLARES_INSTANCE_FIELD: Codigo = Codigo(828);
        pub const EXTENSION_TYPE_EXTENDS: Codigo = Codigo(829);
        pub const EXTENSION_TYPE_WITH: Codigo = Codigo(830);
        pub const EXTERNAL_CLASS: Codigo = Codigo(831);
        pub const EXTERNAL_CONSTRUCTOR_WITH_BODY: Codigo = Codigo(832);
        pub const EXTERNAL_CONSTRUCTOR_WITH_FIELD_INITIALIZERS: Codigo = Codigo(833);
        pub const EXTERNAL_CONSTRUCTOR_WITH_INITIALIZER: Codigo = Codigo(834);
        pub const EXTERNAL_ENUM: Codigo = Codigo(835);
        pub const EXTERNAL_FACTORY_REDIRECTION: Codigo = Codigo(836);
        pub const EXTERNAL_FACTORY_WITH_BODY: Codigo = Codigo(837);
        pub const EXTERNAL_FIELD: Codigo = Codigo(838);
        pub const EXTERNAL_GETTER_WITH_BODY: Codigo = Codigo(839);
        pub const EXTERNAL_LATE_FIELD: Codigo = Codigo(840);
        pub const EXTERNAL_METHOD_WITH_BODY: Codigo = Codigo(841);
        pub const EXTERNAL_OPERATOR_WITH_BODY: Codigo = Codigo(842);
        pub const EXTERNAL_SETTER_WITH_BODY: Codigo = Codigo(843);
        pub const EXTERNAL_TYPEDEF: Codigo = Codigo(844);
        pub const EXTRANEOUS_MODIFIER: Codigo = Codigo(845);
        pub const EXTRANEOUS_MODIFIER_IN_EXTENSION_TYPE: Codigo = Codigo(846);
        pub const EXTRANEOUS_MODIFIER_IN_PRIMARY_CONSTRUCTOR: Codigo = Codigo(847);
        pub const FACTORY_TOP_LEVEL_DECLARATION: Codigo = Codigo(848);
        pub const FACTORY_WITHOUT_BODY: Codigo = Codigo(849);
        pub const FACTORY_WITH_INITIALIZERS: Codigo = Codigo(850);
        pub const FIELD_INITIALIZED_OUTSIDE_DECLARING_CLASS: Codigo = Codigo(851);
        pub const FIELD_INITIALIZER_OUTSIDE_CONSTRUCTOR: Codigo = Codigo(852);
        pub const FINAL_AND_COVARIANT: Codigo = Codigo(853);
        pub const FINAL_AND_COVARIANT_LATE_WITH_INITIALIZER: Codigo = Codigo(854);
        pub const FINAL_AND_VAR: Codigo = Codigo(855);
        pub const FINAL_CONSTRUCTOR: Codigo = Codigo(856);
        pub const FINAL_ENUM: Codigo = Codigo(857);
        pub const FINAL_METHOD: Codigo = Codigo(858);
        pub const FINAL_MIXIN: Codigo = Codigo(859);
        pub const FINAL_MIXIN_CLASS: Codigo = Codigo(860);
        pub const FUNCTION_TYPED_PARAMETER_VAR: Codigo = Codigo(861);
        pub const GETTER_CONSTRUCTOR: Codigo = Codigo(862);
        pub const GETTER_IN_FUNCTION: Codigo = Codigo(863);
        pub const GETTER_WITH_PARAMETERS: Codigo = Codigo(864);
        pub const ILLEGAL_ASSIGNMENT_TO_NON_ASSIGNABLE: Codigo = Codigo(865);
        pub const ILLEGAL_PATTERN_ASSIGNMENT_VARIABLE_NAME: Codigo = Codigo(866);
        pub const ILLEGAL_PATTERN_IDENTIFIER_NAME: Codigo = Codigo(867);
        pub const ILLEGAL_PATTERN_VARIABLE_NAME: Codigo = Codigo(868);
        pub const IMPLEMENTS_BEFORE_EXTENDS: Codigo = Codigo(869);
        pub const IMPLEMENTS_BEFORE_ON: Codigo = Codigo(870);
        pub const IMPLEMENTS_BEFORE_WITH: Codigo = Codigo(871);
        pub const IMPORT_DIRECTIVE_AFTER_PART_DIRECTIVE: Codigo = Codigo(872);
        pub const INITIALIZED_VARIABLE_IN_FOR_EACH: Codigo = Codigo(873);
        pub const INTERFACE_ENUM: Codigo = Codigo(874);
        pub const INTERFACE_MIXIN: Codigo = Codigo(875);
        pub const INTERFACE_MIXIN_CLASS: Codigo = Codigo(876);
        pub const INVALID_AWAIT_IN_FOR: Codigo = Codigo(877);
        pub const INVALID_CODE_POINT: Codigo = Codigo(878);
        pub const INVALID_COMMENT_REFERENCE: Codigo = Codigo(879);
        pub const INVALID_CONSTANT_CONST_PREFIX: Codigo = Codigo(880);
        pub const INVALID_CONSTANT_PATTERN_BINARY: Codigo = Codigo(881);
        pub const INVALID_CONSTANT_PATTERN_DUPLICATE_CONST: Codigo = Codigo(882);
        pub const INVALID_CONSTANT_PATTERN_EMPTY_RECORD_LITERAL: Codigo = Codigo(883);
        pub const INVALID_CONSTANT_PATTERN_GENERIC: Codigo = Codigo(884);
        pub const INVALID_CONSTANT_PATTERN_NEGATION: Codigo = Codigo(885);
        pub const INVALID_CONSTANT_PATTERN_UNARY: Codigo = Codigo(886);
        pub const INVALID_CONSTRUCTOR_NAME: Codigo = Codigo(887);
        pub const INVALID_GENERIC_FUNCTION_TYPE: Codigo = Codigo(888);
        pub const INVALID_HEX_ESCAPE: Codigo = Codigo(889);
        pub const INVALID_INITIALIZER: Codigo = Codigo(890);
        pub const INVALID_INSIDE_UNARY_PATTERN: Codigo = Codigo(891);
        pub const INVALID_LITERAL_IN_CONFIGURATION: Codigo = Codigo(892);
        pub const INVALID_OPERATOR: Codigo = Codigo(893);
        pub const INVALID_OPERATOR_FOR_SUPER: Codigo = Codigo(894);
        pub const INVALID_OPERATOR_QUESTIONMARK_PERIOD_FOR_SUPER: Codigo = Codigo(895);
        pub const INVALID_STAR_AFTER_ASYNC: Codigo = Codigo(896);
        pub const INVALID_SUPER_IN_INITIALIZER: Codigo = Codigo(897);
        pub const INVALID_SYNC: Codigo = Codigo(898);
        pub const INVALID_THIS_IN_INITIALIZER: Codigo = Codigo(899);
        pub const INVALID_UNICODE_ESCAPE_STARTED: Codigo = Codigo(900);
        pub const INVALID_UNICODE_ESCAPE_U_BRACKET: Codigo = Codigo(901);
        pub const INVALID_UNICODE_ESCAPE_U_NO_BRACKET: Codigo = Codigo(902);
        pub const INVALID_UNICODE_ESCAPE_U_STARTED: Codigo = Codigo(903);
        pub const INVALID_USE_OF_COVARIANT_IN_EXTENSION: Codigo = Codigo(904);
        pub const INVALID_USE_OF_IDENTIFIER_AUGMENTED: Codigo = Codigo(905);
        pub const LATE_PATTERN_VARIABLE_DECLARATION: Codigo = Codigo(906);
        pub const LIBRARY_DIRECTIVE_NOT_FIRST: Codigo = Codigo(907);
        pub const LITERAL_WITH_CLASS: Codigo = Codigo(908);
        pub const LITERAL_WITH_CLASS_AND_NEW: Codigo = Codigo(909);
        pub const LITERAL_WITH_NEW: Codigo = Codigo(910);
        pub const LOCAL_FUNCTION_DECLARATION_MODIFIER: Codigo = Codigo(911);
        pub const MEMBER_WITH_CLASS_NAME: Codigo = Codigo(912);
        pub const MISSING_ASSIGNABLE_SELECTOR: Codigo = Codigo(913);
        pub const MISSING_ASSIGNMENT_IN_INITIALIZER: Codigo = Codigo(914);
        pub const MISSING_CATCH_OR_FINALLY: Codigo = Codigo(915);
        pub const MISSING_CLOSING_PARENTHESIS: Codigo = Codigo(916);
        pub const MISSING_CONST_FINAL_VAR_OR_TYPE: Codigo = Codigo(917);
        pub const MISSING_ENUM_BODY: Codigo = Codigo(918);
        pub const MISSING_EXPRESSION_IN_INITIALIZER: Codigo = Codigo(919);
        pub const MISSING_EXPRESSION_IN_THROW: Codigo = Codigo(920);
        pub const MISSING_FUNCTION_BODY: Codigo = Codigo(921);
        pub const MISSING_FUNCTION_KEYWORD: Codigo = Codigo(922);
        pub const MISSING_FUNCTION_PARAMETERS: Codigo = Codigo(923);
        pub const MISSING_GET: Codigo = Codigo(924);
        pub const MISSING_IDENTIFIER: Codigo = Codigo(925);
        pub const MISSING_INITIALIZER: Codigo = Codigo(926);
        pub const MISSING_KEYWORD_OPERATOR: Codigo = Codigo(927);
        pub const MISSING_METHOD_PARAMETERS: Codigo = Codigo(928);
        pub const MISSING_NAME_FOR_NAMED_PARAMETER: Codigo = Codigo(929);
        pub const MISSING_NAME_IN_LIBRARY_DIRECTIVE: Codigo = Codigo(930);
        pub const MISSING_NAME_IN_PART_OF_DIRECTIVE: Codigo = Codigo(931);
        pub const MISSING_PREFIX_IN_DEFERRED_IMPORT: Codigo = Codigo(932);
        pub const MISSING_PRIMARY_CONSTRUCTOR: Codigo = Codigo(933);
        pub const MISSING_PRIMARY_CONSTRUCTOR_PARAMETERS: Codigo = Codigo(934);
        pub const MISSING_STAR_AFTER_SYNC: Codigo = Codigo(935);
        pub const MISSING_STATEMENT: Codigo = Codigo(936);
        pub const MISSING_TERMINATOR_FOR_PARAMETER_GROUP: Codigo = Codigo(937);
        pub const MISSING_TYPEDEF_PARAMETERS: Codigo = Codigo(938);
        pub const MISSING_VARIABLE_IN_FOR_EACH: Codigo = Codigo(939);
        pub const MIXED_PARAMETER_GROUPS: Codigo = Codigo(940);
        pub const MIXIN_DECLARES_CONSTRUCTOR: Codigo = Codigo(941);
        pub const MIXIN_WITH_CLAUSE: Codigo = Codigo(942);
        pub const MODIFIER_OUT_OF_ORDER: Codigo = Codigo(943);
        pub const MULTIPLE_CLAUSES: Codigo = Codigo(944);
        pub const MULTIPLE_EXTENDS_CLAUSES: Codigo = Codigo(945);
        pub const MULTIPLE_IMPLEMENTS_CLAUSES: Codigo = Codigo(946);
        pub const MULTIPLE_LIBRARY_DIRECTIVES: Codigo = Codigo(947);
        pub const MULTIPLE_NAMED_PARAMETER_GROUPS: Codigo = Codigo(948);
        pub const MULTIPLE_ON_CLAUSES: Codigo = Codigo(949);
        pub const MULTIPLE_PART_OF_DIRECTIVES: Codigo = Codigo(950);
        pub const MULTIPLE_POSITIONAL_PARAMETER_GROUPS: Codigo = Codigo(951);
        pub const MULTIPLE_REPRESENTATION_FIELDS: Codigo = Codigo(952);
        pub const MULTIPLE_VARIABLES_IN_FOR_EACH: Codigo = Codigo(953);
        pub const MULTIPLE_VARIANCE_MODIFIERS: Codigo = Codigo(954);
        pub const MULTIPLE_WITH_CLAUSES: Codigo = Codigo(955);
        pub const NAMED_FUNCTION_EXPRESSION: Codigo = Codigo(956);
        pub const NAMED_FUNCTION_TYPE: Codigo = Codigo(957);
        pub const NAMED_PARAMETER_OUTSIDE_GROUP: Codigo = Codigo(958);
        pub const NATIVE_CLAUSE_IN_NON_SDK_CODE: Codigo = Codigo(959);
        pub const NATIVE_CLAUSE_SHOULD_BE_ANNOTATION: Codigo = Codigo(960);
        pub const NATIVE_FUNCTION_BODY_IN_NON_SDK_CODE: Codigo = Codigo(961);
        pub const NON_CONSTRUCTOR_FACTORY: Codigo = Codigo(962);
        pub const NON_IDENTIFIER_LIBRARY_NAME: Codigo = Codigo(963);
        pub const NON_PART_OF_DIRECTIVE_IN_PART: Codigo = Codigo(964);
        pub const NON_STRING_LITERAL_AS_URI: Codigo = Codigo(965);
        pub const NON_USER_DEFINABLE_OPERATOR: Codigo = Codigo(966);
        pub const NORMAL_BEFORE_OPTIONAL_PARAMETERS: Codigo = Codigo(967);
        pub const NULL_AWARE_CASCADE_OUT_OF_ORDER: Codigo = Codigo(968);
        pub const OUT_OF_ORDER_CLAUSES: Codigo = Codigo(969);
        pub const PART_OF_NAME: Codigo = Codigo(970);
        pub const PATTERN_ASSIGNMENT_DECLARES_VARIABLE: Codigo = Codigo(971);
        pub const PATTERN_VARIABLE_DECLARATION_OUTSIDE_FUNCTION_OR_METHOD: Codigo = Codigo(972);
        pub const POSITIONAL_AFTER_NAMED_ARGUMENT: Codigo = Codigo(973);
        pub const POSITIONAL_PARAMETER_OUTSIDE_GROUP: Codigo = Codigo(974);
        pub const PREFIX_AFTER_COMBINATOR: Codigo = Codigo(975);
        pub const RECORD_LITERAL_ONE_POSITIONAL_NO_TRAILING_COMMA: Codigo = Codigo(976);
        pub const RECORD_TYPE_ONE_POSITIONAL_NO_TRAILING_COMMA: Codigo = Codigo(977);
        pub const REDIRECTING_CONSTRUCTOR_WITH_BODY: Codigo = Codigo(978);
        pub const REDIRECTION_IN_NON_FACTORY_CONSTRUCTOR: Codigo = Codigo(979);
        pub const REPRESENTATION_FIELD_MODIFIER: Codigo = Codigo(980);
        pub const REPRESENTATION_FIELD_TRAILING_COMMA: Codigo = Codigo(981);
        pub const SEALED_ENUM: Codigo = Codigo(982);
        pub const SEALED_MIXIN: Codigo = Codigo(983);
        pub const SEALED_MIXIN_CLASS: Codigo = Codigo(984);
        pub const SETTER_CONSTRUCTOR: Codigo = Codigo(985);
        pub const SETTER_IN_FUNCTION: Codigo = Codigo(986);
        pub const STACK_OVERFLOW: Codigo = Codigo(987);
        pub const STATIC_CONSTRUCTOR: Codigo = Codigo(988);
        pub const STATIC_GETTER_WITHOUT_BODY: Codigo = Codigo(989);
        pub const STATIC_OPERATOR: Codigo = Codigo(990);
        pub const STATIC_SETTER_WITHOUT_BODY: Codigo = Codigo(991);
        pub const SWITCH_HAS_CASE_AFTER_DEFAULT_CASE: Codigo = Codigo(992);
        pub const SWITCH_HAS_MULTIPLE_DEFAULT_CASES: Codigo = Codigo(993);
        pub const TOP_LEVEL_OPERATOR: Codigo = Codigo(994);
        pub const TYPEDEF_IN_CLASS: Codigo = Codigo(995);
        pub const TYPE_ARGUMENTS_ON_TYPE_VARIABLE: Codigo = Codigo(996);
        pub const TYPE_BEFORE_FACTORY: Codigo = Codigo(997);
        pub const TYPE_PARAMETER_ON_CONSTRUCTOR: Codigo = Codigo(998);
        pub const TYPE_PARAMETER_ON_OPERATOR: Codigo = Codigo(999);
        pub const UNEXPECTED_TERMINATOR_FOR_PARAMETER_GROUP: Codigo = Codigo(1000);
        pub const UNEXPECTED_TOKEN: Codigo = Codigo(1001);
        pub const UNEXPECTED_TOKENS: Codigo = Codigo(1002);
        pub const VARIABLE_PATTERN_KEYWORD_IN_DECLARATION_CONTEXT: Codigo = Codigo(1003);
        pub const VAR_AND_TYPE: Codigo = Codigo(1004);
        pub const VAR_AS_TYPE_NAME: Codigo = Codigo(1005);
        pub const VAR_CLASS: Codigo = Codigo(1006);
        pub const VAR_ENUM: Codigo = Codigo(1007);
        pub const VAR_RETURN_TYPE: Codigo = Codigo(1008);
        pub const VAR_TYPEDEF: Codigo = Codigo(1009);
        pub const VOID_WITH_TYPE_ARGUMENTS: Codigo = Codigo(1010);
        pub const WITH_BEFORE_EXTENDS: Codigo = Codigo(1011);
        pub const WRONG_SEPARATOR_FOR_POSITIONAL_PARAMETER: Codigo = Codigo(1012);
        pub const WRONG_TERMINATOR_FOR_PARAMETER_GROUP: Codigo = Codigo(1013);
    }
    /// `ScannerErrorCode`.
    pub mod scanner {
        use crate::Codigo;
        pub const EXPECTED_TOKEN: Codigo = Codigo(1014);
        pub const ILLEGAL_CHARACTER: Codigo = Codigo(1015);
        pub const MISSING_DIGIT: Codigo = Codigo(1016);
        pub const MISSING_HEX_DIGIT: Codigo = Codigo(1017);
        pub const MISSING_IDENTIFIER: Codigo = Codigo(1018);
        pub const MISSING_QUOTE: Codigo = Codigo(1019);
        pub const UNABLE_GET_CONTENT: Codigo = Codigo(1020);
        pub const UNEXPECTED_DOLLAR_IN_STRING: Codigo = Codigo(1021);
        pub const UNEXPECTED_SEPARATOR_IN_NUMBER: Codigo = Codigo(1022);
        pub const UNSUPPORTED_OPERATOR: Codigo = Codigo(1023);
        pub const UNTERMINATED_MULTI_LINE_COMMENT: Codigo = Codigo(1024);
        pub const UNTERMINATED_STRING_LITERAL: Codigo = Codigo(1025);
    }
    /// `TodoCode`.
    pub mod todo {
        use crate::Codigo;
        pub const TODO: Codigo = Codigo(1026);
        pub const FIXME: Codigo = Codigo(1027);
        pub const HACK: Codigo = Codigo(1028);
        pub const UNDONE: Codigo = Codigo(1029);
    }
}
