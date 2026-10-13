/// The macro host's `resolveIdentifier` query, answered by the analyzer
/// session that `BuildStep.resolver` already uses.
library;

import 'package:analyzer/dart/analysis/results.dart';
import 'package:analyzer/dart/element/element.dart';
import 'package:analyzer/dart/element/type.dart';

import 'identifier_table.dart';

final class IdentifierResolver {
  final ClassElement classElement;
  final IdentifierTable table;

  IdentifierResolver(this.classElement, Map<String, Object?> execution, [IdentifierTable? sharedTable])
      : table = sharedTable ?? IdentifierTable() {
    final target = Map<String, Object?>.from(execution['target'] as Map);
    final ident = Map<String, Object?>.from(target['ident'] as Map);
    _register(classElement, ident['id'] as int);
    final model = Map<String, Object?>.from(execution['model'] as Map);
    final members = Map<String, Object?>.from(model['members'] as Map);
    final targetId = ident['id'] as int;
    final fields = Map<String, Object?>.from(members['$targetId'] as Map)['fields'] as List;
    final elements = classElement.fields.where((field) => !field.isSynthetic).toList();
    if (fields.length != elements.length) throw StateError('the model and analyzer fields differ');
    for (var i = 0; i < fields.length; i++) {
      final field = Map<String, Object?>.from(fields[i] as Map);
      _register(elements[i], (field['ident'] as Map)['id'] as int);
      _registerType(elements[i].type, Map<String, Object?>.from(field['type'] as Map));
    }
  }

  void _register(Element element, int id) {
    table.bind(element, id);
  }

  void _registerType(DartType type, Map<String, Object?> j) {
    if (type is! InterfaceType || j['t'] != 'named') {
      throw UnsupportedError('type outside the initial model: $type');
    }
    _register(type.element, (j['ident'] as Map)['id'] as int);
    final arguments = j['args'] as List;
    if (arguments.length != type.typeArguments.length) throw StateError('different type arguments');
    for (var i = 0; i < arguments.length; i++) {
      _registerType(type.typeArguments[i], Map<String, Object?>.from(arguments[i] as Map));
    }
  }

  Future<Map<String, Object?>> resolve(String uri, String name) async {
    final result = await classElement.library.session.getLibraryByUri(uri);
    if (result is! LibraryElementResult) throw StateError('library $uri not resolved by the analyzer');
    // The 1st-generation CFE looks up declarations of the library itself;
    // re-exports do not take part in `resolveIdentifier`.
    final locals = result.element.topLevelElements.where((e) => e.name == name);
    final element = locals.isEmpty ? null : locals.first;
    if (element == null) throw StateError('$uri does not export $name');
    final id = table.id(element);
    return {'id': id, 'name': element.name};
  }
}
