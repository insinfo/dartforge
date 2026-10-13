part of '../api.dart';

// What a macro sees of the program (spec, "Introspection"). Declarations
// only: bodies are not visible. The objects come from the model the host
// serves (docs/MACROS-PROTOCOLO.md §5).

/// Something a macro can be applied to: a [Declaration] or a [Library].
abstract interface class MacroTarget {}

/// Something that can have annotations.
abstract interface class Annotatable {
  Iterable<MetadataAnnotation> get metadata;
}

/// Reference to a named declaration. A [Code] can contain it: the host
/// writes the name with the right import prefix. Identifier equality is
/// not specified; to compare types, use
/// [StaticType].
abstract interface class Identifier {
  String get name;
}

/// A type annotation, not yet resolved.
abstract interface class TypeAnnotation {
  /// Does it end in `?`?
  bool get isNullable;

  /// The equivalent [Code].
  TypeAnnotationCode get code;
}

abstract interface class FunctionTypeAnnotation implements TypeAnnotation {
  TypeAnnotation get returnType;

  Iterable<FormalParameter> get positionalParameters;

  Iterable<FormalParameter> get namedParameters;

  Iterable<TypeParameter> get typeParameters;
}

/// A named type, resolvable to a [TypeDeclaration] through the builders.
abstract interface class NamedTypeAnnotation implements TypeAnnotation {
  Identifier get identifier;

  Iterable<TypeAnnotation> get typeArguments;
}

abstract interface class RecordTypeAnnotation implements TypeAnnotation {
  Iterable<RecordField> get positionalFields;

  Iterable<RecordField> get namedFields;
}

/// An unwritten type. It becomes the inferred type (or `dynamic`) in the generated text; in
/// the definitions phase, `inferType` reads it.
abstract interface class OmittedTypeAnnotation implements TypeAnnotation {}

/// A resolved, comparable type.
abstract interface class StaticType {
  Future<bool> isSubtypeOf(covariant StaticType other);

  Future<bool> isExactly(covariant StaticType other);

  /// The supertype of this type whose declaration is [declaration] (with the
  /// type arguments), or `null` if there is none.
  Future<NamedStaticType?> asInstanceOf(TypeDeclaration declaration);
}

abstract interface class NamedStaticType implements StaticType {
  ParameterizedTypeDeclaration get declaration;

  List<StaticType> get typeArguments;
}

/// Every declaration.
abstract interface class Declaration implements Annotatable, MacroTarget {
  Library get library;

  Identifier get identifier;
}

/// A member of a type.
abstract interface class MemberDeclaration implements Declaration {
  Identifier get definingType;

  bool get hasStatic;
}

/// A declaration that introduces a type.
abstract interface class TypeDeclaration implements Declaration {}

abstract interface class ParameterizedTypeDeclaration
    implements TypeDeclaration {
  Iterable<TypeParameterDeclaration> get typeParameters;
}

/// A class. Members come from the builders (`fieldsOf`…).
abstract interface class ClassDeclaration
    implements ParameterizedTypeDeclaration {
  bool get hasAbstract;

  bool get hasBase;

  bool get hasExternal;

  bool get hasFinal;

  bool get hasInterface;

  bool get hasMixin;

  bool get hasSealed;

  NamedTypeAnnotation? get superclass;

  Iterable<NamedTypeAnnotation> get interfaces;

  Iterable<NamedTypeAnnotation> get mixins;
}

abstract interface class EnumDeclaration
    implements ParameterizedTypeDeclaration {
  Iterable<NamedTypeAnnotation> get interfaces;

  Iterable<NamedTypeAnnotation> get mixins;
}

/// An enum value (not introspectable beyond this).
abstract interface class EnumValueDeclaration implements Declaration {
  Identifier get definingEnum;
}

/// An extension (modeled as a type declaration, although it does not
/// introduce a type).
abstract interface class ExtensionDeclaration
    implements ParameterizedTypeDeclaration, Declaration {
  TypeAnnotation get onType;
}

abstract interface class ExtensionTypeDeclaration
    implements ParameterizedTypeDeclaration, Declaration {
  TypeAnnotation get representationType;
}

abstract interface class MixinDeclaration
    implements ParameterizedTypeDeclaration {
  bool get hasBase;

  Iterable<NamedTypeAnnotation> get interfaces;

  Iterable<NamedTypeAnnotation> get superclassConstraints;
}

abstract interface class TypeAliasDeclaration
    implements ParameterizedTypeDeclaration {
  TypeAnnotation get aliasedType;
}

/// A function, a method or a constructor.
abstract interface class FunctionDeclaration implements Declaration {
  /// Does it have a body? (`external` may say `false` and get a body later.)
  bool get hasBody;

  bool get hasExternal;

  bool get isOperator;

  bool get isGetter;

  bool get isSetter;

  TypeAnnotation get returnType;

  Iterable<FormalParameterDeclaration> get positionalParameters;

  Iterable<FormalParameterDeclaration> get namedParameters;

  Iterable<TypeParameterDeclaration> get typeParameters;
}

abstract interface class MethodDeclaration
    implements FunctionDeclaration, MemberDeclaration {}

abstract interface class ConstructorDeclaration implements MethodDeclaration {
  bool get isConst;

  bool get isFactory;
}

abstract interface class VariableDeclaration implements Declaration {
  bool get hasConst;

  bool get hasExternal;

  bool get hasFinal;

  bool get hasInitializer;

  bool get hasLate;

  TypeAnnotation get type;
}

abstract interface class FieldDeclaration
    implements VariableDeclaration, MemberDeclaration {
  bool get hasAbstract;
}

/// A parameter, of a regular function or of a function type.
abstract interface class FormalParameter implements Annotatable {
  TypeAnnotation get type;

  bool get isNamed;

  /// Required positional, or named with `required`.
  bool get isRequired;

  /// A function-type parameter may have no name.
  String? get name;

  ParameterStyle get style;

  /// The equivalent [ParameterCode] (without the default value).
  ParameterCode get code;
}

enum ParameterStyle {
  /// A regular parameter.
  normal,

  /// `this.x`.
  fieldFormal,

  /// `super.x`.
  superFormal,
}

/// A regular function parameter: always has a name and declares a variable.
abstract interface class FormalParameterDeclaration
    implements FormalParameter, Declaration {
  @override
  String get name;
}

abstract interface class TypeParameter implements Annotatable {
  TypeAnnotation? get bound;

  String get name;

  TypeParameterCode get code;
}

/// A type parameter that declares a referenceable type (not those of function
/// types).
abstract interface class TypeParameterDeclaration
    implements TypeDeclaration, TypeParameter {}

/// A field of a record type (`$1`… for positional ones).
abstract interface class RecordField {
  RecordFieldCode get code;

  String? get name;

  TypeAnnotation get type;
}

abstract interface class Library implements Annotatable, MacroTarget {
  LanguageVersion get languageVersion;

  Uri get uri;
}

abstract interface class LanguageVersion {
  int get major;

  int get minor;
}

/// An annotation.
abstract interface class MetadataAnnotation {}

/// `@constant`.
abstract interface class IdentifierMetadataAnnotation
    implements MetadataAnnotation {
  Identifier get identifier;
}

/// `@Type.constructor(args)`.
abstract interface class ConstructorMetadataAnnotation
    implements MetadataAnnotation {
  NamedTypeAnnotation get type;

  /// Empty name for the unnamed constructor.
  Identifier get constructor;

  Iterable<ExpressionCode> get positionalArguments;

  Map<String, ExpressionCode> get namedArguments;
}
