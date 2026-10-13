/// What a macro application produces in one phase (the spec's
/// `MacroExecutionResult`) and the builders that accumulate it. The text of
/// the augmentation library is assembled **in the host** from this structured
/// result (docs/MACROS-PROTOCOLO.md §4); here only the `augment` declarations
/// that the definitions-phase builders generate are formed, with the same
/// text CFE 3.6.2 generates (checked byte for byte).
library;

import 'dart:async';

import '../api.dart';
import 'model.dart';

/// The result of an application in one phase. The maps keep insertion
/// order: it is the order in which the host assembles the text (Rule 2).
final class Result {
  final diagnostics = <Diagnostic>[];
  MacroException? exception;
  final enumValues = <IdentifierImpl, List<DeclarationCode>>{};
  final extends_ = <IdentifierImpl, NamedTypeAnnotationCode>{};
  final interfaces = <IdentifierImpl, List<TypeAnnotationCode>>{};
  final library = <DeclarationCode>[];
  final mixins = <IdentifierImpl, List<TypeAnnotationCode>>{};
  final newTypes = <String>[];
  final types = <IdentifierImpl, List<DeclarationCode>>{};

  /// The protocol's `result`. Maps become lists of pairs so the order
  /// survives JSON.
  Map<String, Object?> toJson() => {
        'diagnostics': [for (final d in diagnostics) _diagnostic(d)],
        'exception': exception == null
            ? null
            : {'kind': exceptionKind(exception!), 'message': exception!.message, 'stackTrace': exception!.stackTrace},
        'enumValues': _pairs(enumValues, (l) => [for (final c in l) codeToJson(c)]),
        'extends': _pairs(extends_, codeToJson),
        'interfaces': _pairs(interfaces, (l) => [for (final c in l) codeToJson(c)]),
        'library': [for (final c in library) codeToJson(c)],
        'mixins': _pairs(mixins, (l) => [for (final c in l) codeToJson(c)]),
        'newTypes': newTypes,
        'types': _pairs(types, (l) => [for (final c in l) codeToJson(c)]),
      };

  static List<Object?> _pairs<V>(Map<IdentifierImpl, V> m, Object? Function(V) f) => [
        for (final MapEntry(:key, :value) in m.entries) [key.id, f(value)],
      ];

  static Map<String, Object?> _diagnostic(Diagnostic d) => {
        'severity': d.severity.name,
        'message': _message(d.message),
        'context': [for (final m in d.contextMessages) _message(m)],
        'correction': d.correctionMessage,
      };

  static Map<String, Object?> _message(DiagnosticMessage m) => {
        'text': m.message,
        'target': switch (m.target) {
          null => null,
          DeclarationDiagnosticTarget(:final declaration) => {
              'declaration': (declaration as DeclarationImpl).identifier.id
            },
          TypeAnnotationDiagnosticTarget(:final typeAnnotation) => {
              'annotation': typeAnnotation is TypeAnnotationImpl ? typeAnnotation.id : null
            },
          MetadataAnnotationDiagnosticTarget() => {'metadata': null},
        },
      };
}

// ------------------------------------------------------------------ builders

/// Base of every builder: the shared result (a nested builder —
/// `buildMethod` etc. — writes to the same result) and the introspector.
abstract class _Base implements Builder, TypePhaseIntrospector {
  final Result result;
  final Introspector introspector;
  _Base(this.result, this.introspector);

  @override
  void report(Diagnostic diagnostic) => result.diagnostics.add(diagnostic);

  @override
  Future<Identifier> resolveIdentifier(Uri library, String name) =>
      // ignore: deprecated_member_use_from_same_package
      introspector.resolveIdentifier(library, name);
}

class TypeBuilderImpl extends _Base implements TypeBuilder {
  TypeBuilderImpl(super.result, super.introspector);

  @override
  void declareType(String name, DeclarationCode typeDeclaration) {
    result.newTypes.add(name);
    result.library.add(typeDeclaration);
  }
}

/// The types phase of a class, enum or mixin: supertypes added to the target.
class TargetTypeBuilderImpl extends TypeBuilderImpl
    implements ClassTypeBuilder, EnumTypeBuilder, MixinTypeBuilder {
  final IdentifierImpl target;
  TargetTypeBuilderImpl(this.target, super.result, super.introspector);

  @override
  void extendsType(NamedTypeAnnotationCode superclass) {
    if (result.extends_.containsKey(target)) {
      throw ArgumentError.value(target.name, null, 'A type cannot extend multiple types');
    }
    result.extends_[target] = superclass;
  }

  @override
  void appendInterfaces(Iterable<TypeAnnotationCode> interfaces) =>
      (result.interfaces[target] ??= []).addAll(interfaces);

  @override
  void appendMixins(Iterable<TypeAnnotationCode> mixins) => (result.mixins[target] ??= []).addAll(mixins);
}

abstract class _DeclarationsBase extends _Base implements DeclarationPhaseIntrospector {
  _DeclarationsBase(super.result, super.introspector);

  @override
  Future<StaticType> resolve(TypeAnnotationCode type) => introspector.resolve(type);
  @override
  Future<List<EnumValueDeclaration>> valuesOf(EnumDeclaration enuum) => introspector.valuesOf(enuum);
  @override
  Future<List<FieldDeclaration>> fieldsOf(TypeDeclaration type) => introspector.fieldsOf(type);
  @override
  Future<List<MethodDeclaration>> methodsOf(TypeDeclaration type) => introspector.methodsOf(type);
  @override
  Future<List<ConstructorDeclaration>> constructorsOf(TypeDeclaration type) =>
      introspector.constructorsOf(type);
  @override
  Future<List<TypeDeclaration>> typesOf(Library library) => introspector.typesOf(library);
  @override
  Future<TypeDeclaration> typeDeclarationOf(Identifier identifier) =>
      introspector.typeDeclarationOf(identifier);
}

class DeclarationBuilderImpl extends _DeclarationsBase implements DeclarationBuilder {
  DeclarationBuilderImpl(super.result, super.introspector);

  @override
  void declareInLibrary(DeclarationCode declaration) => result.library.add(declaration);
}

/// Members (and, for an enum, values) added to [type].
class MemberDeclarationBuilderImpl extends DeclarationBuilderImpl implements EnumDeclarationBuilder {
  final IdentifierImpl type;
  MemberDeclarationBuilderImpl(this.type, super.result, super.introspector);

  @override
  void declareInType(DeclarationCode declaration) => (result.types[type] ??= []).add(declaration);

  @override
  void declareEnumValue(DeclarationCode declaration) => (result.enumValues[type] ??= []).add(declaration);
}

abstract class _DefinitionsBase extends _DeclarationsBase implements DefinitionBuilder {
  _DefinitionsBase(super.result, super.introspector);

  @override
  Future<Declaration> declarationOf(Identifier identifier) => introspector.declarationOf(identifier);
  @override
  Future<TypeAnnotation> inferType(OmittedTypeAnnotation omittedType) => introspector.inferType(omittedType);
  @override
  Future<List<Declaration>> topLevelDeclarationsOf(Library library) =>
      introspector.topLevelDeclarationsOf(library);

  /// The element of [list] with the identifier [id] (the lookup by identity
  /// that the member builders do).
  T _withId<T extends Declaration>(List<T> list, Identifier id, String what) {
    for (final d in list) {
      if (d.identifier == id) return d;
    }
    throw ImplementationException('$what ${id.name} not found');
  }
}

class TypeDefinitionBuilderImpl extends _DefinitionsBase implements EnumDefinitionBuilder {
  final TypeDeclaration declaration;
  TypeDefinitionBuilderImpl(this.declaration, super.result, super.introspector);

  @override
  Future<ConstructorDefinitionBuilder> buildConstructor(Identifier identifier) async =>
      ConstructorDefinitionBuilderImpl(
          _withId(await introspector.constructorsOf(declaration), identifier, 'constructor') as ConstructorImpl,
          result,
          introspector);

  @override
  Future<VariableDefinitionBuilder> buildField(Identifier identifier) async => VariableDefinitionBuilderImpl(
      _withId(await introspector.fieldsOf(declaration), identifier, 'field'), result, introspector);

  @override
  Future<FunctionDefinitionBuilder> buildMethod(Identifier identifier) async => FunctionDefinitionBuilderImpl(
      _withId(await introspector.methodsOf(declaration), identifier, 'method') as FunctionImpl, result, introspector);

  @override
  Future<EnumValueDefinitionBuilder> buildEnumValue(Identifier identifier) async =>
      EnumValueDefinitionBuilderImpl(
          _withId(await introspector.valuesOf(declaration as EnumDeclaration), identifier, 'value') as EnumValueImpl,
          result,
          introspector);
}

class LibraryDefinitionBuilderImpl extends _DefinitionsBase implements LibraryDefinitionBuilder {
  final Library library;
  LibraryDefinitionBuilderImpl(this.library, super.result, super.introspector);

  Future<Declaration> _topLevel(Identifier id) async =>
      _withId(await introspector.topLevelDeclarationsOf(library), id, 'declaration');

  @override
  Future<FunctionDefinitionBuilder> buildFunction(Identifier identifier) async =>
      FunctionDefinitionBuilderImpl(await _topLevel(identifier) as FunctionImpl, result, introspector);

  @override
  Future<TypeDefinitionBuilder> buildType(Identifier identifier) async =>
      TypeDefinitionBuilderImpl(await _topLevel(identifier) as TypeDeclaration, result, introspector);

  @override
  Future<VariableDefinitionBuilder> buildVariable(Identifier identifier) async =>
      VariableDefinitionBuilderImpl(await _topLevel(identifier) as VariableDeclaration, result, introspector);
}

class EnumValueDefinitionBuilderImpl extends _DefinitionsBase implements EnumValueDefinitionBuilder {
  final EnumValueImpl declaration;
  EnumValueDefinitionBuilderImpl(this.declaration, super.result, super.introspector);

  @override
  void augment(DeclarationCode entry) => (result.enumValues[declaration.definingEnum] ??= []).add(entry);
}

class FunctionDefinitionBuilderImpl extends _DefinitionsBase implements FunctionDefinitionBuilder {
  final FunctionImpl declaration;
  FunctionDefinitionBuilderImpl(this.declaration, super.result, super.introspector);

  @override
  void augment(FunctionBodyCode body, {CommentCode? docComments}) {
    final code = functionAugmentation(declaration, body: body, docComments: docComments);
    final d = declaration;
    if (d is MethodImpl) {
      (result.types[d.definingType] ??= []).add(code);
    } else {
      result.library.add(code);
    }
  }
}

class ConstructorDefinitionBuilderImpl extends _DefinitionsBase implements ConstructorDefinitionBuilder {
  final ConstructorImpl declaration;
  ConstructorDefinitionBuilderImpl(this.declaration, super.result, super.introspector);

  @override
  void augment({FunctionBodyCode? body, List<Code>? initializers, CommentCode? docComments}) {
    final code = functionAugmentation(declaration, body: body, initializers: initializers, docComments: docComments);
    (result.types[declaration.definingType] ??= []).add(code);
  }
}

class VariableDefinitionBuilderImpl extends _DefinitionsBase implements VariableDefinitionBuilder {
  final VariableDeclaration declaration;
  VariableDefinitionBuilderImpl(this.declaration, super.result, super.introspector);

  @override
  void augment(
      {DeclarationCode? getter,
      DeclarationCode? setter,
      ExpressionCode? initializer,
      CommentCode? initializerDocComments}) {
    final codes = variableAugmentations(declaration,
        getter: getter, setter: setter, initializer: initializer, initializerDocComments: initializerDocComments);
    final d = declaration;
    if (d is FieldImpl) {
      (result.types[d.definingType] ??= []).addAll(codes);
    } else {
      result.library.addAll(codes);
    }
  }
}

// --------------------------------------------------------- `augment` text

/// The `augment` declarations that complete a variable or a field: one
/// `augment` per requested getter, setter and initializer, in that order.
/// Fields get two spaces of indentation (the text goes inside the class
/// body).
List<DeclarationCode> variableAugmentations(VariableDeclaration d,
    {DeclarationCode? getter,
    DeclarationCode? setter,
    ExpressionCode? initializer,
    CommentCode? initializerDocComments}) {
  if (initializerDocComments != null && initializer == null) {
    throw ArgumentError('initializerDocComments cannot be provided if an initializer is not provided.');
  }
  final isField = d is FieldDeclaration;
  final isStatic = d is FieldDeclaration && d.hasStatic;
  List<Object> head() => [if (isField) '  ', 'augment ', if (isStatic) 'static '];
  return [
    if (getter != null) DeclarationCode.fromParts([...head(), getter]),
    if (setter != null) DeclarationCode.fromParts([...head(), setter]),
    if (initializer != null)
      DeclarationCode.fromParts([
        if (initializerDocComments != null) initializerDocComments,
        ...head(),
        if (d.hasFinal) 'final ',
        d.type.code,
        ' ',
        d.identifier.name,
        ' = ',
        initializer,
        ';',
      ]),
  ];
}

/// The `augment` declaration that completes a function, a method or a
/// constructor: the repeated signature (parameters as introspection sees
/// them, each followed by `, `), the initializer list broken into lines and
/// the body (or `;`). It is CFE 3.6.2's text, byte for byte.
DeclarationCode functionAugmentation(FunctionDeclaration d,
    {FunctionBodyCode? body, List<Code>? initializers, CommentCode? docComments}) {
  assert(initializers == null || d is ConstructorDeclaration);
  final positional = d.positionalParameters.toList();
  final required = positional.takeWhile((p) => p.isRequired);
  final optional = positional.where((p) => !p.isRequired).toList();
  final tparams = d.typeParameters.toList();
  return DeclarationCode.fromParts([
    if (docComments != null) ...[docComments, '\n'],
    if (d is MethodDeclaration) '  ',
    'augment ',
    if (d is ConstructorDeclaration) ...[
      if (d.isConst) 'const ',
      if (d.isFactory) 'factory ',
      d.definingType.name,
      if (d.identifier.name.isNotEmpty) '.',
    ] else ...[
      if (d is MethodDeclaration && d.hasStatic) 'static ',
      d.returnType.code,
      ' ',
      if (d.isOperator) 'operator ',
    ],
    if (d.isGetter) 'get ',
    if (d.isSetter) 'set ',
    d.identifier.name,
    if (!d.isGetter) ...[
      if (tparams.isNotEmpty) ...[
        '<',
        for (final (i, t) in tparams.indexed) ...[
          t.identifier.name,
          if (t.bound != null) ...[' extends ', t.bound!.code],
          if (i < tparams.length - 1) ', ',
        ],
        '>',
      ],
      '(',
      for (final p in required) ...[p.code, ', '],
      if (optional.isNotEmpty) ...['[', for (final p in optional) ...[p.code, ', '], ']'],
      if (d.namedParameters.isNotEmpty) ...['{', for (final p in d.namedParameters) ...[p.code, ', '], '}'],
      ')',
    ],
    if (initializers != null && initializers.isNotEmpty) ...[
      '\n      : ',
      initializers.first,
      for (final i in initializers.skip(1)) ...[',\n        ', i],
    ],
    if (body == null) ';' else ...[' ', body],
  ]);
}

// -------------------------------------------------------------- introspector

/// The introspector of an execution: answers from the [Model] (prefetch) or
/// through the host. It serves all three phases; the host rejects what the
/// phase does not allow.
final class Introspector implements DefinitionPhaseIntrospector {
  final Model model;
  Introspector(this.model);

  IdentifierImpl _id(Object d) => switch (d) {
        DeclarationImpl() => d.identifier,
        IdentifierImpl() => d,
        _ => throw ImplementationException('declaration that did not come from the host: $d'),
      };

  @override
  Future<Identifier> resolveIdentifier(Uri library, String name) async {
    final r = await model.query('resolveIdentifier', {'uri': '$library', 'name': name});
    return model.identifier(r as Map<String, Object?>);
  }

  @override
  Future<StaticType> resolve(TypeAnnotationCode type) async {
    final r = await model.query('resolve', {'type': typeCodeToJson(type)});
    return model.staticType(r as Map<String, Object?>);
  }

  @override
  Future<List<EnumValueDeclaration>> valuesOf(EnumDeclaration enuum) async =>
      (await model.members(_id(enuum), 'values')).cast();

  @override
  Future<List<FieldDeclaration>> fieldsOf(TypeDeclaration type) async =>
      (await model.members(_id(type), 'fields')).cast();

  @override
  Future<List<MethodDeclaration>> methodsOf(TypeDeclaration type) async =>
      (await model.members(_id(type), 'methods')).cast();

  @override
  Future<List<ConstructorDeclaration>> constructorsOf(TypeDeclaration type) async =>
      (await model.members(_id(type), 'constructors')).cast();

  @override
  Future<List<TypeDeclaration>> typesOf(Library library) async {
    final r = await model.query('typesOf', {'library': (library as LibraryImpl).id}) as List;
    return [for (final d in r) model.declaration(d as Map<String, Object?>) as TypeDeclaration];
  }

  @override
  Future<List<Declaration>> topLevelDeclarationsOf(Library library) async {
    final r = await model.query('topLevelDeclarationsOf', {'library': (library as LibraryImpl).id}) as List;
    return [for (final d in r) model.declaration(d as Map<String, Object?>)];
  }

  @override
  Future<TypeDeclaration> typeDeclarationOf(Identifier identifier) async {
    final d = await declarationOf(identifier);
    if (d is TypeDeclaration) return d;
    throw ImplementationException('${identifier.name} is not a type declaration');
  }

  @override
  Future<Declaration> declarationOf(Identifier identifier) async {
    final r = await model.query('declarationOf', {'ident': _id(identifier).id});
    return model.declaration(r as Map<String, Object?>);
  }

  @override
  Future<TypeAnnotation> inferType(OmittedTypeAnnotation omittedType) async {
    final r = await model.query('inferType', {'key': (omittedType as OmittedTypeImpl).key});
    return model.type(r as Map<String, Object?>);
  }
}
