part of '../api.dart';

// O que cada fase pode consultar (introspectores) e produzir (builders).
// Cada fase vê tudo o que a anterior via; o que uma macro produz volta ao
// hospedeiro como resultado estruturado e vira texto de augmentation lá
// (docs/MACROS-PROTOCOLO.md §4).

/// Base de todo builder: acumula diagnósticos para o fim da fase.
abstract interface class Builder {
  /// Anexa [diagnostic] ao resultado desta aplicação nesta fase (entregue
  /// junto com o resto, quando a macro termina).
  void report(Diagnostic diagnostic);
}

/// Introspecção da fase de tipos (e das seguintes).
abstract interface class TypePhaseIntrospector {
  /// Um [Identifier] para o nome de topo [name] da biblioteca [library].
  ///
  /// [library] deve estar no fecho de imports da biblioteca que recebe o
  /// código (o jeito seguro é a biblioteca da macro importá-la). Nome que não
  /// existe lança exceção. Para um campo, devolve o identificador da
  /// declaração (não o do getter sintético).
  @Deprecated(
      'This API should eventually be replaced with a different, safer API.')
  Future<Identifier> resolveIdentifier(Uri library, String name);
}

/// Contribui tipos novos à biblioteca corrente.
abstract interface class TypeBuilder implements Builder, TypePhaseIntrospector {
  /// Acrescenta a declaração de tipo [typeDeclaration] (cujo nome, sem
  /// parâmetros de tipo, é [name]) à biblioteca.
  void declareType(String name, DeclarationCode typeDeclaration);
}

/// Acrescenta interfaces a um tipo existente (fase de tipos).
abstract interface class InterfaceTypesBuilder implements TypeBuilder {
  void appendInterfaces(Iterable<TypeAnnotationCode> interfaces);
}

/// Acrescenta mixins a um tipo existente (fase de tipos).
abstract interface class MixinTypesBuilder implements TypeBuilder {
  void appendMixins(Iterable<TypeAnnotationCode> mixins);
}

/// Dá uma cláusula `extends` a um tipo que não tem (fase de tipos).
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

/// Mixins não têm mixins, só interfaces.
abstract interface class MixinTypeBuilder
    implements TypeBuilder, InterfaceTypesBuilder {}

/// Introspecção da fase de declarações (e da seguinte).
///
/// As listas vêm em **ordem lexicográfica de nome** (spec, "Introspection
/// API ordering"): reordenar membros no fonte não muda o que a macro vê.
abstract interface class DeclarationPhaseIntrospector
    implements TypePhaseIntrospector {
  /// O [StaticType] de uma anotação de tipo. [RawTypeAnnotationCode] não é
  /// aceito (identificadores crus não são resolvíveis); identificador que
  /// não resolve lança exceção.
  Future<StaticType> resolve(TypeAnnotationCode type);

  /// Os valores de [enuum] (podem estar incompletos se outras macros de
  /// declaração ainda vão rodar nele).
  Future<List<EnumValueDeclaration>> valuesOf(covariant EnumDeclaration enuum);

  Future<List<FieldDeclaration>> fieldsOf(covariant TypeDeclaration type);

  Future<List<MethodDeclaration>> methodsOf(covariant TypeDeclaration type);

  Future<List<ConstructorDeclaration>> constructorsOf(
      covariant TypeDeclaration type);

  /// Os tipos declarados em [library], extensions incluídas.
  Future<List<TypeDeclaration>> typesOf(covariant Library library);

  /// A declaração de tipo de [identifier]; se não for tipo, lança
  /// [MacroImplementationException].
  Future<TypeDeclaration> typeDeclarationOf(covariant Identifier identifier);
}

/// Contribui declarações (não tipos) à biblioteca corrente.
abstract interface class DeclarationBuilder
    implements Builder, DeclarationPhaseIntrospector {
  void declareInLibrary(DeclarationCode declaration);
}

/// Contribui membros ao tipo alvo.
abstract interface class MemberDeclarationBuilder
    implements DeclarationBuilder {
  void declareInType(DeclarationCode declaration);
}

/// Contribui membros ou valores ao enum alvo.
abstract interface class EnumDeclarationBuilder
    implements MemberDeclarationBuilder {
  void declareEnumValue(DeclarationCode declaration);
}

/// Introspecção da fase de definições.
abstract interface class DefinitionPhaseIntrospector
    implements DeclarationPhaseIntrospector {
  Future<Declaration> declarationOf(covariant Identifier identifier);

  @override
  Future<TypeDeclaration> typeDeclarationOf(covariant Identifier identifier);

  /// O tipo inferido de uma anotação omitida (`dynamic` se nada foi
  /// inferido). Só existe nesta fase (spec, "Omitted type annotations").
  Future<TypeAnnotation> inferType(covariant OmittedTypeAnnotation omittedType);

  Future<List<Declaration>> topLevelDeclarationsOf(covariant Library library);
}

abstract interface class DefinitionBuilder
    implements Builder, DefinitionPhaseIntrospector {}

/// Macro de biblioteca na fase de definições: builders das declarações dela.
abstract interface class LibraryDefinitionBuilder implements DefinitionBuilder {
  Future<TypeDefinitionBuilder> buildType(Identifier identifier);

  Future<FunctionDefinitionBuilder> buildFunction(Identifier identifier);

  Future<VariableDefinitionBuilder> buildVariable(Identifier identifier);
}

/// Macro de tipo na fase de definições: builders dos membros dele.
abstract interface class TypeDefinitionBuilder implements DefinitionBuilder {
  Future<VariableDefinitionBuilder> buildField(Identifier identifier);

  Future<FunctionDefinitionBuilder> buildMethod(Identifier identifier);

  Future<ConstructorDefinitionBuilder> buildConstructor(Identifier identifier);
}

abstract interface class EnumDefinitionBuilder
    implements TypeDefinitionBuilder {
  Future<EnumValueDefinitionBuilder> buildEnumValue(Identifier identifier);
}

/// Completa um construtor: corpo e/ou lista de inicialização.
abstract interface class ConstructorDefinitionBuilder
    implements DefinitionBuilder {
  /// [initializers] sem vírgulas nas pontas; [docComments] vão acima da
  /// declaração `augment`.
  void augment({
    FunctionBodyCode? body,
    List<Code>? initializers,
    CommentCode? docComments,
  });
}

/// Completa uma função ou um método com um corpo.
abstract interface class FunctionDefinitionBuilder
    implements DefinitionBuilder {
  void augment(
    FunctionBodyCode body, {
    CommentCode? docComments,
  });
}

/// Completa uma variável de topo ou um campo.
abstract interface class VariableDefinitionBuilder
    implements DefinitionBuilder {
  /// [getter] e [setter] são declarações completas sem o `augment` (que é
  /// acrescentado); [initializerDocComments] exige [initializer].
  void augment({
    DeclarationCode? getter,
    DeclarationCode? setter,
    ExpressionCode? initializer,
    CommentCode? initializerDocComments,
  });
}

/// Troca um valor de enum por outro de mesmo nome.
abstract interface class EnumValueDefinitionBuilder
    implements DefinitionBuilder {
  void augment(DeclarationCode entry);
}
