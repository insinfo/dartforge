/// Type queries used by `package:json`'s definitions phase. Each `resolve`
/// allocates new `StaticType` keys, as CFE 3.6.2 does.
library;

import 'package:analyzer/dart/element/element.dart';
import 'package:analyzer/dart/element/nullability_suffix.dart';
import 'package:analyzer/dart/element/type.dart';

import 'identifier_resolver.dart';
import 'identifier_table.dart';

final class _StaticType {
  final ClassElement element;
  final List<_StaticType> args;
  final bool nullable;
  _StaticType(this.element, this.args, this.nullable);

  bool equals(_StaticType other) {
    if (element.librarySource.uri != other.element.librarySource.uri ||
        element.name != other.element.name ||
        nullable != other.nullable ||
        args.length != other.args.length) return false;
    for (var i = 0; i < args.length; i++) {
      if (!args[i].equals(other.args[i])) return false;
    }
    return true;
  }
}

final class DefinitionQueries {
  final IdentifierResolver resolver;
  final Map<int, Map<String, Object?>> membersById;
  IdentifierTable get table => resolver.table;
  final statics = <_StaticType>[];

  DefinitionQueries(this.resolver, [Map<int, Map<String, Object?>>? membersById])
      : membersById = membersById ?? {};

  Future<Object?> query(String kind, Map<String, Object?> args) async => switch (kind) {
        'resolveIdentifier' => await resolver.resolve(args['uri'] as String, args['name'] as String),
        'declarationOf' => declaration(args['ident'] as int),
        'members' => _members(args['owner'] as int, args['kind'] as String),
        'resolve' => _staticJson(_readStatic(Map<String, Object?>.from(args['type'] as Map))),
        'isExactly' => _static(args['a'] as int).equals(_static(args['b'] as int)),
        _ => throw UnsupportedError('query $kind not implemented yet'),
      };

  List<Object?> _members(int owner, String kind) {
    final classMembers = membersById[owner];
    if (classMembers == null) {
      throw UnsupportedError('members of $owner not available yet');
    }
    final members = classMembers[kind];
    if (members is! List) {
      throw UnsupportedError('member category $kind not covered');
    }
    return List<Object?>.from(members);
  }

  _StaticType _readStatic(Map<String, Object?> j) {
    if (j['t'] != 'named') {
      throw UnsupportedError('StaticType not named yet: ${j['t']}');
    }
    final id = (j['ident'] as Map)['id'] as int;
    final element = table.element(id);
    if (element is! ClassElement) {
      throw StateError('unknown static type $id');
    }
    return _StaticType(
      element,
      [for (final arg in j['args'] as List) _readStatic(Map<String, Object?>.from(arg as Map))],
      j['nullable'] == true,
    );
  }

  _StaticType _static(int key) {
    if (key < 1 || key > statics.length) {
      throw StateError('unknown static type $key');
    }
    return statics[key - 1];
  }

  Map<String, Object?> _staticJson(_StaticType type) {
    final typeDeclaration = _classDeclaration(type.element);
    final args = [for (final arg in type.args) _staticJson(arg)];
    statics.add(type);
    return {
      'key': statics.length,
      'declaration': typeDeclaration,
      'args': args,
    };
  }

  Map<String, Object?> declaration(int id) {
    final element = table.element(id);
    if (element is! ClassElement) {
      throw UnsupportedError('declaration $id not covered yet');
    }
    return _classDeclaration(element);
  }

  Map<String, Object?> _classDeclaration(ClassElement classElement) {
    final uri = classElement.librarySource.uri.toString();
    final version = classElement.library.languageVersion.effective;
    final lib = <String, Object?>{
      'k': 'library',
      'id': table.library(uri),
      'uri': uri,
      'version': [version.major, version.minor],
    };
    final ident = {'id': table.id(classElement), 'name': classElement.name};
    final tparams = <Map<String, Object?>>[];
    for (final parameter in classElement.typeParameters) {
      if (parameter.bound != null) {
        throw UnsupportedError('type parameter bound not covered yet');
      }
      tparams.add({
        'k': 'tparam',
        'ident': {
          'id': table.generatedId('tparam:$uri#${classElement.name}.${parameter.name}'),
          'name': parameter.name,
        },
        'lib': lib,
        'bound': null,
      });
    }
    final supertype = classElement.supertype;
    final superclass = supertype == null ||
            (supertype.element.name == 'Object' && supertype.element.librarySource.uri.toString() == 'dart:core')
        ? null
        : _analyzerType(supertype);
    return {
      'k': 'class',
      'ident': ident,
      'lib': lib,
      'tparams': tparams,
      'abstract': classElement.isAbstract,
      'base': classElement.isBase,
      'external': false,
      'final': classElement.isFinal,
      'interface': classElement.isInterface,
      'mixin': classElement.isMixinClass,
      'sealed': classElement.isSealed,
      'superclass': superclass,
      'interfaces': [for (final t in classElement.interfaces) _analyzerType(t)],
      'mixins': [for (final t in classElement.mixins) _analyzerType(t)],
    };
  }

  Map<String, Object?> _analyzerType(DartType type) {
    if (type is TypeParameterType) {
      final parameter = type.element;
      final owner = parameter.enclosingElement3;
      if (owner is! ClassElement) {
        throw UnsupportedError('type parameter without a class: $parameter');
      }
      final uri = owner.librarySource.uri.toString();
      return {
        't': 'named',
        'ident': {
          'id': table.generatedId('tparam:$uri#${owner.name}.${parameter.name}'),
          'name': parameter.name,
        },
        'args': <Object?>[],
        if (type.nullabilitySuffix == NullabilitySuffix.question) 'nullable': true,
      };
    }
    if (type is! InterfaceType) {
      throw UnsupportedError('declaration type not covered yet: $type');
    }
    return {
      't': 'named',
      'ident': {'id': table.id(type.element), 'name': type.element.name},
      'args': [for (final arg in type.typeArguments) _analyzerType(arg)],
      if (type.nullabilitySuffix == NullabilitySuffix.question) 'nullable': true,
    };
  }
}
