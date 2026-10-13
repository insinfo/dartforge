/// The executor side: the introspection objects built from the **model** the
/// host sends (docs/MACROS-PROTOCOLO.md §5), and the introspector that
/// answers a macro's queries — from the model already received when it can
/// (the prefetch), from the host when it cannot.
///
/// Plain Dart, with nothing only the DartForge compiler has: the same code
/// runs in the native executor and on any VM (the materialization builder,
/// docs/MACROS-COMPATIBILIDADE.md).
library;

import 'dart:async';

import '../api.dart';

/// Who answers the queries: the host through the `macro.*` protocol of
/// `dfexec/1`, or, in tests, an in-memory universe.
abstract interface class Host {
  /// A query of [kind] with [args]; the answer is the protocol's `value`.
  /// A host error becomes a [HostError].
  Future<Object?> query(String kind, Map<String, Object?> args);
}

/// An error answer from the host (`{"error": {"kind", "message"}}`).
class HostError implements Exception {
  /// `implementation` (wrong use of the API), `cycle` (introspection cycle in
  /// the declarations phase) or `unexpected`.
  final String kind;
  final String message;
  HostError(this.kind, this.message);

  /// The API exception the macro sees.
  MacroException asMacroException() => switch (kind) {
        'implementation' => ImplementationException(message),
        'cycle' => CycleException(message),
        _ => UnexpectedException(message),
      };

  @override
  String toString() => 'HostError($kind): $message';
}

// ---------------------------------------------------------------- exceptions

abstract class _ExceptionBase implements MacroException {
  @override
  final String message;
  @override
  final String? stackTrace;
  _ExceptionBase(this.message, [this.stackTrace]);

  /// The name in the protocol (`result.exception.kind`).
  String get kind;

  @override
  String toString() => '$runtimeType: $message';
}

class ImplementationException extends _ExceptionBase
    implements MacroImplementationException {
  ImplementationException(super.message, [super.stackTrace]);
  @override
  String get kind => 'implementation';
}

class CycleException extends _ExceptionBase
    implements MacroIntrospectionCycleException {
  CycleException(super.message, [super.stackTrace]);
  @override
  String get kind => 'cycle';
}

class UnexpectedException extends _ExceptionBase
    implements UnexpectedMacroException {
  UnexpectedException(super.message, [super.stackTrace]);
  @override
  String get kind => 'unexpected';
}

/// The protocol kind of any [MacroException].
String exceptionKind(MacroException e) => switch (e) {
      _ExceptionBase() => e.kind,
      MacroIntrospectionCycleException() => 'cycle',
      MacroImplementationException() => 'implementation',
      _ => 'unexpected',
    };

// --------------------------------------------------------------- identifiers

/// An identifier from the host: `id` is the key it resolves it with when
/// assembling the text. One object per `id` in each [Model], so `==` is
/// identity (as a macro expects when looking a member up by identifier).
final class IdentifierImpl implements Identifier {
  final int id;
  @override
  final String name;
  IdentifierImpl(this.id, this.name);

  @override
  String toString() => 'Identifier($name#$id)';
}

// --------------------------------------------------------------------- types

sealed class TypeAnnotationImpl implements TypeAnnotation {
  /// Position of the annotation in the source, when the host gave it (a
  /// diagnostic target).
  final int? id;
  @override
  final bool isNullable;
  TypeAnnotationImpl(this.id, this.isNullable);
}

final class NamedTypeImpl extends TypeAnnotationImpl implements NamedTypeAnnotation {
  @override
  final IdentifierImpl identifier;
  @override
  final List<TypeAnnotationImpl> typeArguments;
  NamedTypeImpl(super.id, super.isNullable, this.identifier, this.typeArguments);

  @override
  TypeAnnotationCode get code {
    final base = NamedTypeAnnotationCode(
        name: identifier, typeArguments: [for (final a in typeArguments) a.code]);
    return isNullable ? base.asNullable : base;
  }
}

final class FunctionTypeImpl extends TypeAnnotationImpl implements FunctionTypeAnnotation {
  @override
  final TypeAnnotationImpl returnType;
  @override
  final List<ParameterImpl> positionalParameters;
  @override
  final List<ParameterImpl> namedParameters;
  @override
  final List<TypeParameterImpl> typeParameters;
  FunctionTypeImpl(super.id, super.isNullable, this.returnType, this.positionalParameters,
      this.namedParameters, this.typeParameters);

  @override
  TypeAnnotationCode get code {
    final base = FunctionTypeAnnotationCode(
      returnType: returnType.code,
      typeParameters: [for (final t in typeParameters) t.code],
      positionalParameters: [
        for (final p in positionalParameters)
          if (p.isRequired) p.code
      ],
      optionalPositionalParameters: [
        for (final p in positionalParameters)
          if (!p.isRequired) p.code
      ],
      namedParameters: [for (final p in namedParameters) p.code],
    );
    return isNullable ? base.asNullable : base;
  }
}

final class RecordTypeImpl extends TypeAnnotationImpl implements RecordTypeAnnotation {
  @override
  final List<RecordFieldImpl> positionalFields;
  @override
  final List<RecordFieldImpl> namedFields;
  RecordTypeImpl(super.id, super.isNullable, this.positionalFields, this.namedFields);

  @override
  TypeAnnotationCode get code {
    final base = RecordTypeAnnotationCode(
      namedFields: [for (final c in namedFields) c.code],
      positionalFields: [for (final c in positionalFields) c.code],
    );
    return isNullable ? base.asNullable : base;
  }
}

final class OmittedTypeImpl extends TypeAnnotationImpl implements OmittedTypeAnnotation {
  /// The host's key for the omitted type (`inferType` and the assembly).
  final int key;
  OmittedTypeImpl(this.key) : super(null, false);

  @override
  TypeAnnotationCode get code => OmittedTypeAnnotationCode(this);
}

final class RecordFieldImpl implements RecordField {
  @override
  final String? name;
  @override
  final TypeAnnotationImpl type;
  RecordFieldImpl(this.name, this.type);

  @override
  RecordFieldCode get code => RecordFieldCode(type: type.code, name: name);
}

/// A parameter of a function type (no declaration).
final class ParameterImpl implements FormalParameter {
  @override
  final String? name;
  @override
  final TypeAnnotationImpl type;
  @override
  final bool isNamed;
  @override
  final bool isRequired;
  @override
  final ParameterStyle style;
  @override
  final List<MetadataAnnotation> metadata;
  ParameterImpl(this.name, this.type, this.isNamed, this.isRequired, this.style, this.metadata);

  @override
  ParameterCode get code => ParameterCode(
      name: name, type: type.code, keywords: [if (isNamed && isRequired) 'required']);
}

/// A type parameter of a function type (no declaration).
final class TypeParameterImpl implements TypeParameter {
  @override
  final String name;
  @override
  final TypeAnnotationImpl? bound;
  @override
  final List<MetadataAnnotation> metadata;
  TypeParameterImpl(this.name, this.bound, this.metadata);

  @override
  TypeParameterCode get code => TypeParameterCode(name: name, bound: bound?.code);
}

// ----------------------------------------------------------------- libraries

final class LanguageVersionImpl implements LanguageVersion {
  @override
  final int major;
  @override
  final int minor;
  LanguageVersionImpl(this.major, this.minor);
}

final class LibraryImpl implements Library {
  final int id;
  @override
  final Uri uri;
  @override
  final LanguageVersion languageVersion;
  @override
  final List<MetadataAnnotation> metadata;
  LibraryImpl(this.id, this.uri, this.languageVersion, this.metadata);
}

// --------------------------------------------------------------- annotations

final class IdentifierAnnotationImpl implements IdentifierMetadataAnnotation {
  @override
  final IdentifierImpl identifier;
  IdentifierAnnotationImpl(this.identifier);
}

final class ConstructorAnnotationImpl implements ConstructorMetadataAnnotation {
  @override
  final NamedTypeImpl type;
  @override
  final IdentifierImpl constructor;
  @override
  final List<ExpressionCode> positionalArguments;
  @override
  final Map<String, ExpressionCode> namedArguments;
  ConstructorAnnotationImpl(this.type, this.constructor, this.positionalArguments, this.namedArguments);
}

// -------------------------------------------------------------- declarations

/// Base of every declaration coming from the model.
sealed class DeclarationImpl implements Declaration {
  @override
  final IdentifierImpl identifier;
  @override
  final LibraryImpl library;
  @override
  final List<MetadataAnnotation> metadata;
  DeclarationImpl(this.identifier, this.library, this.metadata);
}

final class TypeParameterDeclImpl extends DeclarationImpl implements TypeParameterDeclaration {
  @override
  final TypeAnnotationImpl? bound;
  TypeParameterDeclImpl(super.identifier, super.library, super.metadata, this.bound);

  @override
  String get name => identifier.name;

  @override
  TypeParameterCode get code => TypeParameterCode(name: identifier.name, bound: bound?.code);
}

sealed class TypeDeclImpl extends DeclarationImpl implements ParameterizedTypeDeclaration {
  @override
  final List<TypeParameterDeclImpl> typeParameters;
  TypeDeclImpl(super.identifier, super.library, super.metadata, this.typeParameters);
}

final class ClassImpl extends TypeDeclImpl implements ClassDeclaration {
  @override
  final bool hasAbstract, hasBase, hasExternal, hasFinal, hasInterface, hasMixin, hasSealed;
  @override
  final NamedTypeImpl? superclass;
  @override
  final List<NamedTypeImpl> interfaces;
  @override
  final List<NamedTypeImpl> mixins;
  ClassImpl(super.identifier, super.library, super.metadata, super.typeParameters,
      {required this.hasAbstract,
      required this.hasBase,
      required this.hasExternal,
      required this.hasFinal,
      required this.hasInterface,
      required this.hasMixin,
      required this.hasSealed,
      required this.superclass,
      required this.interfaces,
      required this.mixins});
}

final class EnumImpl extends TypeDeclImpl implements EnumDeclaration {
  @override
  final List<NamedTypeImpl> interfaces;
  @override
  final List<NamedTypeImpl> mixins;
  EnumImpl(super.identifier, super.library, super.metadata, super.typeParameters, this.interfaces, this.mixins);
}

final class MixinImpl extends TypeDeclImpl implements MixinDeclaration {
  @override
  final bool hasBase;
  @override
  final List<NamedTypeImpl> interfaces;
  @override
  final List<NamedTypeImpl> superclassConstraints;
  MixinImpl(super.identifier, super.library, super.metadata, super.typeParameters, this.hasBase,
      this.interfaces, this.superclassConstraints);
}

final class ExtensionImpl extends TypeDeclImpl implements ExtensionDeclaration {
  @override
  final TypeAnnotationImpl onType;
  ExtensionImpl(super.identifier, super.library, super.metadata, super.typeParameters, this.onType);
}

final class ExtensionTypeImpl extends TypeDeclImpl implements ExtensionTypeDeclaration {
  @override
  final TypeAnnotationImpl representationType;
  ExtensionTypeImpl(super.identifier, super.library, super.metadata, super.typeParameters, this.representationType);
}

final class TypedefImpl extends TypeDeclImpl implements TypeAliasDeclaration {
  @override
  final TypeAnnotationImpl aliasedType;
  TypedefImpl(super.identifier, super.library, super.metadata, super.typeParameters, this.aliasedType);
}

final class EnumValueImpl extends DeclarationImpl implements EnumValueDeclaration {
  @override
  final IdentifierImpl definingEnum;
  EnumValueImpl(super.identifier, super.library, super.metadata, this.definingEnum);
}

final class ParameterDeclImpl extends DeclarationImpl implements FormalParameterDeclaration {
  @override
  final TypeAnnotationImpl type;
  @override
  final bool isNamed;
  @override
  final bool isRequired;
  @override
  final ParameterStyle style;
  ParameterDeclImpl(super.identifier, super.library, super.metadata, this.type, this.isNamed, this.isRequired, this.style);

  @override
  String get name => identifier.name;

  @override
  ParameterCode get code => ParameterCode(
      name: identifier.name,
      style: style,
      type: type.code,
      keywords: [if (isNamed && isRequired) 'required']);
}

/// A top-level function; base of method and constructor.
base class FunctionImpl extends DeclarationImpl implements FunctionDeclaration {
  @override
  final bool hasBody, hasExternal, isOperator, isGetter, isSetter;
  @override
  final TypeAnnotationImpl returnType;
  @override
  final List<ParameterDeclImpl> positionalParameters;
  @override
  final List<ParameterDeclImpl> namedParameters;
  @override
  final List<TypeParameterDeclImpl> typeParameters;
  FunctionImpl(super.identifier, super.library, super.metadata,
      {required this.hasBody,
      required this.hasExternal,
      required this.isOperator,
      required this.isGetter,
      required this.isSetter,
      required this.returnType,
      required this.positionalParameters,
      required this.namedParameters,
      required this.typeParameters});
}

base class MethodImpl extends FunctionImpl implements MethodDeclaration {
  @override
  final IdentifierImpl definingType;
  @override
  final bool hasStatic;
  MethodImpl(super.identifier, super.library, super.metadata,
      {required super.hasBody,
      required super.hasExternal,
      required super.isOperator,
      required super.isGetter,
      required super.isSetter,
      required super.returnType,
      required super.positionalParameters,
      required super.namedParameters,
      required super.typeParameters,
      required this.definingType,
      required this.hasStatic});
}

final class ConstructorImpl extends MethodImpl implements ConstructorDeclaration {
  @override
  final bool isConst, isFactory;
  ConstructorImpl(super.identifier, super.library, super.metadata,
      {required super.hasBody,
      required super.hasExternal,
      required super.returnType,
      required super.positionalParameters,
      required super.namedParameters,
      required super.typeParameters,
      required super.definingType,
      required this.isConst,
      required this.isFactory})
      : super(isOperator: false, isGetter: false, isSetter: false, hasStatic: true);
}

base class VariableImpl extends DeclarationImpl implements VariableDeclaration {
  @override
  final bool hasConst, hasExternal, hasFinal, hasInitializer, hasLate;
  @override
  final TypeAnnotationImpl type;
  VariableImpl(super.identifier, super.library, super.metadata,
      {required this.hasConst,
      required this.hasExternal,
      required this.hasFinal,
      required this.hasInitializer,
      required this.hasLate,
      required this.type});
}

final class FieldImpl extends VariableImpl implements FieldDeclaration {
  @override
  final IdentifierImpl definingType;
  @override
  final bool hasStatic, hasAbstract;
  FieldImpl(super.identifier, super.library, super.metadata,
      {required super.hasConst,
      required super.hasExternal,
      required super.hasFinal,
      required super.hasInitializer,
      required super.hasLate,
      required super.type,
      required this.definingType,
      required this.hasStatic,
      required this.hasAbstract});
}

// -------------------------------------------------------------- static types

final class StaticTypeImpl implements NamedStaticType {
  /// The host's key for the type.
  final int key;
  final Model _model;
  final ParameterizedTypeDeclaration? _declaration;
  final List<StaticType> _arguments;
  StaticTypeImpl(this.key, this._model, this._declaration, this._arguments);

  @override
  ParameterizedTypeDeclaration get declaration =>
      _declaration ?? (throw ImplementationException('the type is not a named type'));

  @override
  List<StaticType> get typeArguments => _arguments;

  @override
  Future<bool> isExactly(StaticTypeImpl other) async =>
      await _model.query('isExactly', {'a': key, 'b': other.key}) as bool;

  @override
  Future<bool> isSubtypeOf(StaticTypeImpl other) async =>
      await _model.query('isSubtypeOf', {'a': key, 'b': other.key}) as bool;

  @override
  Future<NamedStaticType?> asInstanceOf(TypeDeclaration declaration) async {
    final r = await _model.query('asInstanceOf',
        {'type': key, 'declaration': (declaration as DeclarationImpl).identifier.id});
    return r == null ? null : _model.staticType(r as Map<String, Object?>);
  }
}

// ----------------------------------------------------------------- the model

/// The objects of an execution session: one per `id`, decoded from the
/// model's JSON, with the member lists already received kept.
final class Model {
  final Host host;
  final _identifiers = <int, IdentifierImpl>{};
  final _declarations = <int, DeclarationImpl>{};
  final _libraries = <int, LibraryImpl>{};
  final _omitted = <int, OmittedTypeImpl>{};
  final _members = <(int, String), List<DeclarationImpl>>{};

  Model(this.host);

  Future<Object?> query(String kind, Map<String, Object?> args) async {
    try {
      return await host.query(kind, args);
    } on HostError catch (e) {
      throw e.asMacroException();
    }
  }

  /// Receives the `model` part of a `macro.execute`: libraries, declarations
  /// and the prefetched member lists.
  void receive(Map<String, Object?> model) {
    for (final b in (model['libraries'] as List? ?? const [])) {
      library(b as Map<String, Object?>);
    }
    for (final d in (model['declarations'] as List? ?? const [])) {
      declaration(d as Map<String, Object?>);
    }
    final members = model['members'] as Map<String, Object?>? ?? const {};
    for (final MapEntry(key: owner, value: lists) in members.entries) {
      for (final MapEntry(key: kind, value: list) in (lists as Map<String, Object?>).entries) {
        _members[(int.parse(owner), kind)] = [
          for (final d in list as List) declaration(d as Map<String, Object?>),
        ];
      }
    }
  }

  IdentifierImpl identifier(Map<String, Object?> j) {
    final id = j['id'] as int;
    return _identifiers.putIfAbsent(id, () => IdentifierImpl(id, j['name'] as String));
  }

  LibraryImpl library(Object? j) {
    if (j is int) {
      return _libraries[j] ?? (throw UnexpectedException('library $j is not in the model'));
    }
    final m = j as Map<String, Object?>;
    final id = m['id'] as int;
    return _libraries.putIfAbsent(id, () {
      final v = m['version'] as List;
      return LibraryImpl(id, Uri.parse(m['uri'] as String), LanguageVersionImpl(v[0] as int, v[1] as int), []);
    });
  }

  TypeAnnotationImpl type(Map<String, Object?> j) {
    final id = j['id'] as int?;
    final nullable = j['nullable'] as bool? ?? false;
    return switch (j['t']) {
      'named' => NamedTypeImpl(id, nullable, identifier(j['ident'] as Map<String, Object?>),
          [for (final a in j['args'] as List? ?? const []) type(a as Map<String, Object?>)]),
      'function' => FunctionTypeImpl(
          id,
          nullable,
          type(j['return'] as Map<String, Object?>),
          [for (final p in j['positional'] as List? ?? const []) _parameter(p as Map<String, Object?>)],
          [for (final p in j['named'] as List? ?? const []) _parameter(p as Map<String, Object?>)],
          [for (final t in j['tparams'] as List? ?? const []) _typeParameter(t as Map<String, Object?>)]),
      'record' => RecordTypeImpl(
          id,
          nullable,
          [for (final c in j['positional'] as List? ?? const []) _recordField(c as Map<String, Object?>)],
          [for (final c in j['named'] as List? ?? const []) _recordField(c as Map<String, Object?>)]),
      'omitted' => _omitted.putIfAbsent(j['key'] as int, () => OmittedTypeImpl(j['key'] as int)),
      final t => throw UnexpectedException('unknown type in the model: $t'),
    };
  }

  ParameterImpl _parameter(Map<String, Object?> j) => ParameterImpl(
      j['name'] as String?,
      type(j['type'] as Map<String, Object?>),
      j['named'] as bool? ?? false,
      j['required'] as bool? ?? false,
      _style(j['style']),
      _metadata(j));

  TypeParameterImpl _typeParameter(Map<String, Object?> j) => TypeParameterImpl(
      j['name'] as String,
      j['bound'] == null ? null : type(j['bound'] as Map<String, Object?>),
      _metadata(j));

  RecordFieldImpl _recordField(Map<String, Object?> j) =>
      RecordFieldImpl(j['name'] as String?, type(j['type'] as Map<String, Object?>));

  ParameterStyle _style(Object? e) => switch (e) {
        'this' => ParameterStyle.fieldFormal,
        'super' => ParameterStyle.superFormal,
        _ => ParameterStyle.normal,
      };

  List<MetadataAnnotation> _metadata(Map<String, Object?> j) => [
        for (final m in j['meta'] as List? ?? const [])
          switch ((m as Map<String, Object?>)['k']) {
            'ident' => IdentifierAnnotationImpl(identifier(m['ident'] as Map<String, Object?>)),
            _ => ConstructorAnnotationImpl(
                type(m['type'] as Map<String, Object?>) as NamedTypeImpl,
                identifier(m['constructor'] as Map<String, Object?>),
                [for (final a in m['positional'] as List? ?? const []) codeFromJson(a, this) as ExpressionCode],
                {
                  for (final MapEntry(:key, :value) in (m['named'] as Map<String, Object?>? ?? const {}).entries)
                    key: codeFromJson(value, this) as ExpressionCode,
                }),
          },
      ];

  bool _b(Map<String, Object?> j, String k) => j[k] as bool? ?? false;

  List<NamedTypeImpl> _namedTypes(Object? l) =>
      [for (final t in l as List? ?? const []) type(t as Map<String, Object?>) as NamedTypeImpl];

  List<TypeParameterDeclImpl> _tparams(Map<String, Object?> j) =>
      [for (final t in j['tparams'] as List? ?? const []) declaration(t as Map<String, Object?>) as TypeParameterDeclImpl];

  List<ParameterDeclImpl> _pdecls(Object? l) =>
      [for (final p in l as List? ?? const []) declaration(p as Map<String, Object?>) as ParameterDeclImpl];

  /// The declaration of `j`, one per identifier.
  DeclarationImpl declaration(Map<String, Object?> j) {
    final ident = identifier(j['ident'] as Map<String, Object?>);
    final known = _declarations[ident.id];
    if (known != null) return known;
    final lib = library(j['lib']);
    final meta = _metadata(j);
    IdentifierImpl owner() => identifier(j['owner'] as Map<String, Object?>);
    final DeclarationImpl d = switch (j['k']) {
      'class' => ClassImpl(ident, lib, meta, _tparams(j),
          hasAbstract: _b(j, 'abstract'),
          hasBase: _b(j, 'base'),
          hasExternal: _b(j, 'external'),
          hasFinal: _b(j, 'final'),
          hasInterface: _b(j, 'interface'),
          hasMixin: _b(j, 'mixin'),
          hasSealed: _b(j, 'sealed'),
          superclass: j['superclass'] == null ? null : type(j['superclass'] as Map<String, Object?>) as NamedTypeImpl,
          interfaces: _namedTypes(j['interfaces']),
          mixins: _namedTypes(j['mixins'])),
      'enum' => EnumImpl(ident, lib, meta, _tparams(j), _namedTypes(j['interfaces']), _namedTypes(j['mixins'])),
      'mixin' => MixinImpl(ident, lib, meta, _tparams(j), _b(j, 'base'), _namedTypes(j['interfaces']),
          _namedTypes(j['constraints'])),
      'extension' => ExtensionImpl(ident, lib, meta, _tparams(j), type(j['on'] as Map<String, Object?>)),
      'extensionType' =>
        ExtensionTypeImpl(ident, lib, meta, _tparams(j), type(j['representation'] as Map<String, Object?>)),
      'typedef' => TypedefImpl(ident, lib, meta, _tparams(j), type(j['alias'] as Map<String, Object?>)),
      'tparam' => TypeParameterDeclImpl(
          ident, lib, meta, j['bound'] == null ? null : type(j['bound'] as Map<String, Object?>)),
      'enumValue' => EnumValueImpl(ident, lib, meta, identifier(j['enum'] as Map<String, Object?>)),
      'parameter' => ParameterDeclImpl(ident, lib, meta, type(j['type'] as Map<String, Object?>), _b(j, 'named'),
          _b(j, 'required'), _style(j['style'])),
      'function' => FunctionImpl(ident, lib, meta,
          hasBody: _b(j, 'body'),
          hasExternal: _b(j, 'external'),
          isOperator: _b(j, 'operator'),
          isGetter: _b(j, 'getter'),
          isSetter: _b(j, 'setter'),
          returnType: type(j['return'] as Map<String, Object?>),
          positionalParameters: _pdecls(j['positional']),
          namedParameters: _pdecls(j['named']),
          typeParameters: _tparams(j)),
      'method' => MethodImpl(ident, lib, meta,
          hasBody: _b(j, 'body'),
          hasExternal: _b(j, 'external'),
          isOperator: _b(j, 'operator'),
          isGetter: _b(j, 'getter'),
          isSetter: _b(j, 'setter'),
          returnType: type(j['return'] as Map<String, Object?>),
          positionalParameters: _pdecls(j['positional']),
          namedParameters: _pdecls(j['named']),
          typeParameters: _tparams(j),
          definingType: owner(),
          hasStatic: _b(j, 'static')),
      'constructor' => ConstructorImpl(ident, lib, meta,
          hasBody: _b(j, 'body'),
          hasExternal: _b(j, 'external'),
          returnType: type(j['return'] as Map<String, Object?>),
          positionalParameters: _pdecls(j['positional']),
          namedParameters: _pdecls(j['named']),
          typeParameters: _tparams(j),
          definingType: owner(),
          isConst: _b(j, 'const'),
          isFactory: _b(j, 'factory')),
      'variable' => VariableImpl(ident, lib, meta,
          hasConst: _b(j, 'const'),
          hasExternal: _b(j, 'external'),
          hasFinal: _b(j, 'final'),
          hasInitializer: _b(j, 'initializer'),
          hasLate: _b(j, 'late'),
          type: type(j['type'] as Map<String, Object?>)),
      'field' => FieldImpl(ident, lib, meta,
          hasConst: _b(j, 'const'),
          hasExternal: _b(j, 'external'),
          hasFinal: _b(j, 'final'),
          hasInitializer: _b(j, 'initializer'),
          hasLate: _b(j, 'late'),
          type: type(j['type'] as Map<String, Object?>),
          definingType: owner(),
          hasStatic: _b(j, 'static'),
          hasAbstract: _b(j, 'abstract')),
      final k => throw UnexpectedException('unknown declaration in the model: $k'),
    };
    _declarations[ident.id] = d;
    return d;
  }

  StaticTypeImpl staticType(Map<String, Object?> j) {
    final decl = j['declaration'];
    return StaticTypeImpl(
      j['key'] as int,
      this,
      decl == null ? null : declaration(decl as Map<String, Object?>) as ParameterizedTypeDeclaration,
      [for (final a in j['args'] as List? ?? const []) staticType(a as Map<String, Object?>)],
    );
  }

  /// Members of [owner] of one [kind] (`fields`, `methods`, `constructors`,
  /// `values`): from the prefetch, otherwise from the host. The host sends
  /// them in lexicographic name order.
  Future<List<DeclarationImpl>> members(IdentifierImpl owner, String kind) async {
    final known = _members[(owner.id, kind)];
    if (known != null) return known;
    final r = await query('members', {'owner': owner.id, 'kind': kind}) as List;
    return _members[(owner.id, kind)] = [for (final d in r) declaration(d as Map<String, Object?>)];
  }
}

// ------------------------------------------------------------- Code ⇄ JSON

/// `Code` in the protocol: `{"k": kind, "p": [parts]}`; a part is text,
/// `{"i": id, "n": name}` (identifier), `{"o": key}` (omitted type) or
/// another code.
Map<String, Object?> codeToJson(Code c) => {
      'k': c.kind.name,
      'p': [for (final p in c.parts) _partToJson(p)],
    };

Object? _partToJson(Object p) => switch (p) {
      String() => p,
      IdentifierImpl() => {'i': p.id, 'n': p.name},
      Identifier() => throw ImplementationException(
          'identifier that did not come from the host: ${p.name} (${p.runtimeType})'),
      OmittedTypeImpl() => {'o': p.key},
      Code() => codeToJson(p),
      _ => throw ImplementationException('invalid code part: ${p.runtimeType}'),
    };

/// A type annotation in code (`TypeAnnotationCode`) in the model's type
/// format, for `resolve`: the structure (name, arguments, `?`) that the
/// flattened `Code` loses. Raw code is not resolvable (spec: `resolve`
/// throws for [RawTypeAnnotationCode]).
Map<String, Object?> typeCodeToJson(TypeAnnotationCode c) => switch (c) {
      NullableTypeAnnotationCode(:final underlyingType) => {...typeCodeToJson(underlyingType), 'nullable': true},
      NamedTypeAnnotationCode(:final name, :final typeArguments) => {
          't': 'named',
          'ident': switch (name) {
            IdentifierImpl() => {'id': name.id, 'name': name.name},
            _ => throw ImplementationException('identifier that did not come from the host: ${name.name}'),
          },
          'args': [for (final a in typeArguments) typeCodeToJson(a)],
        },
      FunctionTypeAnnotationCode() => {
          't': 'function',
          'text': [for (final p in c.parts) _partToJson(p)],
        },
      RecordTypeAnnotationCode() => {
          't': 'record',
          'text': [for (final p in c.parts) _partToJson(p)],
        },
      OmittedTypeAnnotationCode(:final typeAnnotation) => {
          't': 'omitted',
          'key': (typeAnnotation as OmittedTypeImpl).key,
        },
      RawTypeAnnotationCode() => throw ImplementationException(
          'resolve does not accept RawTypeAnnotationCode: use a specific subtype of TypeAnnotationCode'),
    };

/// The inverse of [codeToJson], for annotation arguments.
Code codeFromJson(Object? j, Model m) {
  final map = j as Map<String, Object?>;
  final parts = <Object>[
    for (final p in map['p'] as List)
      switch (p) {
        String() => p,
        {'i': final int id, 'n': final String n} => m.identifier({'id': id, 'name': n}),
        {'o': final int key} => m.type({'t': 'omitted', 'key': key}),
        _ => codeFromJson(p, m),
      },
  ];
  return switch (map['k']) {
    'expression' => ExpressionCode.fromParts(parts),
    'declaration' => DeclarationCode.fromParts(parts),
    'functionBody' => FunctionBodyCode.fromParts(parts),
    'comment' => CommentCode.fromParts(parts),
    _ => RawCode.fromParts(parts),
  };
}
