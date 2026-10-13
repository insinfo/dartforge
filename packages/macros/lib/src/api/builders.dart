part of '../api.dart';

// What each phase can query (introspectors) and produce (builders). Each
// phase sees everything the previous one saw; what a macro produces goes back
// to the host as a structured result and becomes augmentation text there
// (docs/MACROS-PROTOCOLO.md §4).

/// Base of every builder: accumulates diagnostics for the end of the phase.
abstract interface class Builder {
  /// Attaches [diagnostic] to the result of this application in this phase
  /// (delivered with the rest, when the macro finishes).
  void report(Diagnostic diagnostic);
}

/// Introspection of the types phase (and the following ones).
abstract interface class TypePhaseIntrospector {
  /// An [Identifier] for the top-level name [name] of the library [library].
  ///
  /// [library] must be in the import closure of the library that receives the
  /// code (the safe way is for the macro's library to import it). A name that
  /// does not exist throws. For a field, returns the identifier of the
  /// declaration (not that of the synthetic getter).
  @Deprecated(
      'This API should eventually be replaced with a different, safer API.')
  Future<Identifier> resolveIdentifier(Uri library, String name);
}

/// Contributes new types to the current library.
abstract interface class TypeBuilder implements Builder, TypePhaseIntrospector {
  /// Adds the type declaration [typeDeclaration] (whose name, without type
  /// parameters, is [name]) to the library.
  void declareType(String name, DeclarationCode typeDeclaration);
}

/// Adds interfaces to an existing type (types phase).
abstract interface class InterfaceTypesBuilder implements TypeBuilder {
  void appendInterfaces(Iterable<TypeAnnotationCode> interfaces);
}

/// Adds mixins to an existing type (types phase).
abstract interface class MixinTypesBuilder implements TypeBuilder {
  void appendMixins(Iterable<TypeAnnotationCode> mixins);
}

/// Gives an `extends` clause to a type that has none (types phase).
abstract interface class ExtendsTypeBuilder implements TypeBuilder {
  void extendsType(NamedTypeAnnotationCode superclass);
}

abstract interface class ClassTypeBuilder
    implements
        TypeBuilder,
        ExtendsTypeBuilder,
        InterfaceTypesBuilder,
        MixinTypesBuilder {}

abstract interface class EnumTypeBuilder
    implements TypeBuilder, InterfaceTypesBuilder, MixinTypesBuilder {}

/// Mixins have no mixins, only interfaces.
abstract interface class MixinTypeBuilder
    implements TypeBuilder, InterfaceTypesBuilder {}

/// Introspection of the declarations phase (and the next one).
///
/// Lists come in **lexicographic name order** (spec, "Introspection
/// API ordering"): reordering members in the source does not change what the macro sees.
abstract interface class DeclarationPhaseIntrospector
    implements TypePhaseIntrospector {
  /// The [StaticType] of a type annotation. [RawTypeAnnotationCode] is not
  /// accepted (raw identifiers are not resolvable); an identifier that does
  /// not resolve throws.
  Future<StaticType> resolve(TypeAnnotationCode type);

  /// The values of [enuum] (may be incomplete if other declaration macros
  /// will still run on it).
  Future<List<EnumValueDeclaration>> valuesOf(covariant EnumDeclaration enuum);

  Future<List<FieldDeclaration>> fieldsOf(covariant TypeDeclaration type);

  Future<List<MethodDeclaration>> methodsOf(covariant TypeDeclaration type);

  Future<List<ConstructorDeclaration>> constructorsOf(
      covariant TypeDeclaration type);

  /// The types declared in [library], extensions included.
  Future<List<TypeDeclaration>> typesOf(covariant Library library);

  /// The type declaration of [identifier]; if it is not a type, throws
  /// [MacroImplementationException].
  Future<TypeDeclaration> typeDeclarationOf(covariant Identifier identifier);
}

/// Contributes declarations (not types) to the current library.
abstract interface class DeclarationBuilder
    implements Builder, DeclarationPhaseIntrospector {
  void declareInLibrary(DeclarationCode declaration);
}

/// Contributes members to the target type.
abstract interface class MemberDeclarationBuilder
    implements DeclarationBuilder {
  void declareInType(DeclarationCode declaration);
}

/// Contributes members or values to the target enum.
abstract interface class EnumDeclarationBuilder
    implements MemberDeclarationBuilder {
  void declareEnumValue(DeclarationCode declaration);
}

/// Introspection of the definitions phase.
abstract interface class DefinitionPhaseIntrospector
    implements DeclarationPhaseIntrospector {
  Future<Declaration> declarationOf(covariant Identifier identifier);

  @override
  Future<TypeDeclaration> typeDeclarationOf(covariant Identifier identifier);

  /// The inferred type of an omitted annotation (`dynamic` if nothing was
  /// inferred). Only exists in this phase (spec, "Omitted type annotations").
  Future<TypeAnnotation> inferType(covariant OmittedTypeAnnotation omittedType);

  Future<List<Declaration>> topLevelDeclarationsOf(covariant Library library);
}

abstract interface class DefinitionBuilder
    implements Builder, DefinitionPhaseIntrospector {}

/// Library macro in the definitions phase: builders of its declarations.
abstract interface class LibraryDefinitionBuilder implements DefinitionBuilder {
  Future<TypeDefinitionBuilder> buildType(Identifier identifier);

  Future<FunctionDefinitionBuilder> buildFunction(Identifier identifier);

  Future<VariableDefinitionBuilder> buildVariable(Identifier identifier);
}

/// Type macro in the definitions phase: builders of its members.
abstract interface class TypeDefinitionBuilder implements DefinitionBuilder {
  Future<VariableDefinitionBuilder> buildField(Identifier identifier);

  Future<FunctionDefinitionBuilder> buildMethod(Identifier identifier);

  Future<ConstructorDefinitionBuilder> buildConstructor(Identifier identifier);
}

abstract interface class EnumDefinitionBuilder
    implements TypeDefinitionBuilder {
  Future<EnumValueDefinitionBuilder> buildEnumValue(Identifier identifier);
}

/// Completes a constructor: body and/or initializer list.
abstract interface class ConstructorDefinitionBuilder
    implements DefinitionBuilder {
  /// [initializers] without commas at the ends; [docComments] go above the
  /// `augment` declaration.
  void augment({
    FunctionBodyCode? body,
    List<Code>? initializers,
    CommentCode? docComments,
  });
}

/// Completes a function or a method with a body.
abstract interface class FunctionDefinitionBuilder
    implements DefinitionBuilder {
  void augment(
    FunctionBodyCode body, {
    CommentCode? docComments,
  });
}

/// Completes a top-level variable or a field.
abstract interface class VariableDefinitionBuilder
    implements DefinitionBuilder {
  /// [getter] and [setter] are complete declarations without the `augment`
  /// (which is added); [initializerDocComments] requires [initializer].
  void augment({
    DeclarationCode? getter,
    DeclarationCode? setter,
    ExpressionCode? initializer,
    CommentCode? initializerDocComments,
  });
}

/// Replaces an enum value with another of the same name.
abstract interface class EnumValueDefinitionBuilder
    implements DefinitionBuilder {
  void augment(DeclarationCode entry);
}
