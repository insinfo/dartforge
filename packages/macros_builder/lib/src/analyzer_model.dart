/// First slice of the `BuildStep.resolver` -> `macro.execute` adapter:
/// simple classes and their fields, with the same vocabulary as `dfexec/1`.
/// The other declaration kinds come in when there is a directed oracle.
library;

import 'package:analyzer/dart/element/element.dart';
import 'package:analyzer/dart/element/nullability_suffix.dart';
import 'package:analyzer/dart/element/type.dart';

import 'identifier_table.dart';

Map<String, Object?> classModel(ClassElement classElement, [IdentifierTable? sharedTable]) {
  final table = sharedTable ?? IdentifierTable();
  if (classElement.typeParameters.isNotEmpty ||
      classElement.interfaces.isNotEmpty ||
      classElement.mixins.isNotEmpty ||
      (classElement.supertype != null &&
          (classElement.supertype!.element.name != 'Object' ||
              classElement.supertype!.element.librarySource.uri.toString() != 'dart:core')) ||
      classElement.methods.isNotEmpty) {
    throw UnsupportedError(
        'the initial model covers classes without inheritance, type parameters or executable members');
  }
  final uri = classElement.librarySource.uri.toString();
  final version = classElement.library.languageVersion.effective;
  final library = <String, Object?>{
    'k': 'library',
    'id': table.library(uri),
    'uri': uri,
    'version': [version.major, version.minor]
  };
  final identifier = <String, Object?>{'id': table.id(classElement), 'name': classElement.name};

  Map<String, Object?> type(DartType input) {
    if (input is! InterfaceType) throw UnsupportedError('not an interface type: $input');
    final element = input.element;
    final id = table.id(element);
    final output = <String, Object?>{
      't': 'named',
      'ident': {'id': id, 'name': element.name},
      'args': [for (final argument in input.typeArguments) type(argument)],
    };
    if (input.nullabilitySuffix == NullabilitySuffix.question) output['nullable'] = true;
    return output;
  }

  final fields = <Map<String, Object?>>[];
  for (final field in classElement.fields.where((c) => !c.isSynthetic)) {
    final name = field.name;
    final fieldType = type(field.type);
    fields.add({
      'k': 'field',
      'ident': {'id': table.id(field), 'name': name},
      'lib': library,
      'owner': identifier,
      'type': fieldType,
      'abstract': field.isAbstract,
      'const': field.isConst,
      'external': field.isExternal,
      'final': field.isFinal,
      'initializer': field.hasInitializer,
      'late': field.isLate,
      'static': field.isStatic,
    });
  }
  final constructors = <Map<String, Object?>>[];
  for (final ctor in classElement.constructors.where((c) => !c.isSynthetic)) {
    if (ctor.isFactory ||
        ctor.isExternal ||
        ctor.parameters.any((p) => !p.isInitializingFormal || p.isNamed || p.hasDefaultValue)) {
      throw UnsupportedError('constructor outside the initial model: $ctor');
    }
    final key = '$uri#${classElement.name}.${ctor.name}';
    final omittedReturn = table.omitted('constructor:$key:return');
    final positional = <Map<String, Object?>>[];
    for (final parameter in ctor.parameters) {
      positional.add({
        'k': 'parameter',
        'ident': {
          'id': table.generatedId('parameter:$key:${parameter.name}'),
          'name': parameter.name,
        },
        'lib': library,
        'type': {
          't': 'omitted',
          'key': table.omitted('parameter:$key:${parameter.name}:type'),
        },
        'named': false,
        'required': parameter.isRequired,
        'style': 'this',
      });
    }
    constructors.add({
      'k': 'constructor',
      'ident': {
        'id': table.generatedId('constructor:$key'),
        'name': ctor.name,
      },
      'lib': library,
      'owner': identifier,
      'body': true,
      'external': false,
      'const': ctor.isConst,
      'factory': false,
      'return': {'t': 'omitted', 'key': omittedReturn},
      'positional': positional,
      'named': <Object?>[],
      'tparams': <Object?>[],
    });
  }
  return {
    'target': {
      'k': 'class',
      'ident': identifier,
      'lib': library,
      'tparams': <Object?>[],
      'abstract': classElement.isAbstract,
      'base': classElement.isBase,
      'external': false,
      'final': classElement.isFinal,
      'interface': classElement.isInterface,
      'mixin': classElement.isMixinClass,
      'sealed': classElement.isSealed,
      'superclass': null,
      'interfaces': <Object?>[],
      'mixins': <Object?>[],
    },
    'model': {
      'libraries': [library],
      'members': {
        '${identifier['id']}': {'fields': fields, 'constructors': constructors, 'methods': <Object?>[]},
      },
    },
  };
}
