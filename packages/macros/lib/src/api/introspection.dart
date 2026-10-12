part of '../api.dart';

// O que uma macro vê do programa (spec, "Introspection"). Só declarações:
// corpos não são visíveis. Os objetos vêm do modelo que o hospedeiro serve
// (docs/MACROS-PROTOCOLO.md §5).

/// Algo a que uma macro pode ser aplicada: uma [Declaration] ou [Library].
abstract interface class MacroTarget {}

/// Algo que pode ter anotações.
abstract interface class Annotatable {
  Iterable<MetadataAnnotation> get metadata;
}

/// Referência a uma declaração nomeada. Um [Code] pode contê-la: o
/// hospedeiro escreve o nome com o prefixo de import certo. Igualdade de
/// identificadores não é especificada; para comparar tipos, use
/// [StaticType].
abstract interface class Identifier {
  String get name;
}

/// Uma anotação de tipo, ainda não resolvida.
abstract interface class TypeAnnotation {
  /// Tem `?` no fim?
  bool get isNullable;

  /// O [Code] equivalente.
  TypeAnnotationCode get code;
}

abstract interface class FunctionTypeAnnotation implements TypeAnnotation {
  TypeAnnotation get returnType;

  Iterable<FormalParameter> get positionalParameters;

  Iterable<FormalParameter> get namedParameters;

  Iterable<TypeParameter> get typeParameters;
}

/// Um tipo nomeado, resolvível a [TypeDeclaration] pelos builders.
abstract interface class NamedTypeAnnotation implements TypeAnnotation {
  Identifier get identifier;

  Iterable<TypeAnnotation> get typeArguments;
}

abstract interface class RecordTypeAnnotation implements TypeAnnotation {
  Iterable<RecordField> get positionalFields;

  Iterable<RecordField> get namedFields;
}

/// Tipo não escrito. Vira o tipo inferido (ou `dynamic`) no texto gerado; na
/// fase de definições, `inferType` o lê.
abstract interface class OmittedTypeAnnotation implements TypeAnnotation {}

/// Um tipo resolvido, comparável.
abstract interface class StaticType {
  Future<bool> isSubtypeOf(covariant StaticType other);

  Future<bool> isExactly(covariant StaticType other);

  /// O supertipo deste tipo cuja declaração é [declaration] (com os
  /// argumentos de tipo), ou `null` se não houver.
  Future<NamedStaticType?> asInstanceOf(TypeDeclaration declaration);
}

abstract interface class NamedStaticType implements StaticType {
  ParameterizedTypeDeclaration get declaration;

  List<StaticType> get typeArguments;
}

/// Toda declaração.
abstract interface class Declaration implements Annotatable, MacroTarget {
  Library get library;

  Identifier get identifier;
}

/// Membro de um tipo.
abstract interface class MemberDeclaration implements Declaration {
  Identifier get definingType;

  bool get hasStatic;
}

/// Declaração que introduz um tipo.
abstract interface class TypeDeclaration implements Declaration {}

abstract interface class ParameterizedTypeDeclaration
    implements TypeDeclaration {
  Iterable<TypeParameterDeclaration> get typeParameters;
}

/// Uma classe. Membros vêm dos builders (`fieldsOf`…).
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

/// Um valor de enum (não introspectável além disso).
abstract interface class EnumValueDeclaration implements Declaration {
  Identifier get definingEnum;
}

/// Uma extension (modelada como declaração de tipo, embora não introduza
/// tipo).
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

/// Uma função, um método ou um construtor.
abstract interface class FunctionDeclaration implements Declaration {
  /// Tem corpo? (`external` pode dizer `false` e ganhar corpo depois.)
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

/// Parâmetro, de função comum ou de tipo de função.
abstract interface class FormalParameter implements Annotatable {
  TypeAnnotation get type;

  bool get isNamed;

  /// Posicional obrigatório, ou nomeado com `required`.
  bool get isRequired;

  /// Parâmetro de tipo de função pode não ter nome.
  String? get name;

  ParameterStyle get style;

  /// O [ParameterCode] equivalente (sem o valor padrão).
  ParameterCode get code;
}

enum ParameterStyle {
  /// Parâmetro comum.
  normal,

  /// `this.x`.
  fieldFormal,

  /// `super.x`.
  superFormal,
}

/// Parâmetro de função comum: sempre tem nome e declara variável.
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

/// Parâmetro de tipo que declara um tipo referenciável (não os de tipos de
/// função).
abstract interface class TypeParameterDeclaration
    implements TypeDeclaration, TypeParameter {}

/// Campo de um tipo record (`$1`… nos posicionais).
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

/// Uma anotação.
abstract interface class MetadataAnnotation {}

/// `@constante`.
abstract interface class IdentifierMetadataAnnotation
    implements MetadataAnnotation {
  Identifier get identifier;
}

/// `@Tipo.construtor(args)`.
abstract interface class ConstructorMetadataAnnotation
    implements MetadataAnnotation {
  NamedTypeAnnotation get type;

  /// Nome vazio para o construtor sem nome.
  Identifier get constructor;

  Iterable<ExpressionCode> get positionalArguments;

  Map<String, ExpressionCode> get namedArguments;
}
