import 'dart:convert';
import 'dart:io';

import 'package:macros/src/executor/assembly.dart';

/// Replays the real `package:json` 0.20.4 results recorded in
/// `session.dfexec`, including the identifier queries to the host. The oracle
/// is the augmentation extracted from CFE 3.6.2, not code written here.
final class _SessionTable implements AssemblyResolver {
  final ids = <int, ResolvedIdentifier>{};
  final types = <int, AugmentedType>{};
  final queries = <int, Map<String, Object?>>{};

  void register(Object? value) {
    if (value is List) {
      for (final item in value) register(item);
      return;
    }
    if (value is! Map) return;
    final m = Map<String, Object?>.from(value);
    final ident = m['ident'];
    if (ident is Map && ident['id'] is int && ident['name'] is String) {
      final id = ident['id'] as int;
      final name = ident['name'] as String;
      final lib = m['lib'];
      final uri = lib is Map ? lib['uri'] as String? : null;
      final owner = m['owner'];
      final scope = owner is Map ? owner['name'] as String? : null;
      switch (m['k']) {
        case 'class' || 'enum' || 'mixin':
          ids[id] = ResolvedIdentifier(name, IdentifierKind.topLevel, uri: uri);
          types[id] = AugmentedType(
            m['k'] as String,
            [
              for (final modifier in ['abstract', 'base', 'final', 'interface', 'mixin', 'sealed'])
                if (m[modifier] == true) modifier,
            ],
            name,
          );
        case 'field' || 'method':
          ids[id] = m['static'] == true
              ? ResolvedIdentifier(name, IdentifierKind.staticMember, uri: uri, scope: scope)
              : ResolvedIdentifier(name, IdentifierKind.instanceMember);
        case 'constructor':
          ids[id] = ResolvedIdentifier(name, IdentifierKind.staticMember, uri: uri, scope: scope);
        case 'parameter':
          ids[id] = ResolvedIdentifier(name, IdentifierKind.local);
      }
    }
    for (final child in m.values) register(child);
  }

  void message(Map<String, Object?> m) {
    if (m['t'] == 'macro.query' && m['kind'] == 'resolveIdentifier') {
      queries[m['id'] as int] = Map<String, Object?>.from(m['args'] as Map);
    } else if (m['t'] == 'macro.response' && queries.containsKey(m['id'])) {
      final answer = m['value'] as Map;
      final args = queries.remove(m['id'])!;
      ids[answer['id'] as int] = ResolvedIdentifier(
        answer['name'] as String,
        IdentifierKind.topLevel,
        uri: args['uri'] as String,
      );
    }
    register(m);
  }

  @override
  ResolvedIdentifier identifier(int id) => ids[id] ?? (throw StateError('identifier $id missing'));

  @override
  AugmentedType augmentedType(int id) => types[id] ?? (throw StateError('type $id missing'));

  @override
  Map<String, Object?>? inferredType(int key) => null;
}

void main() {
  final root = Directory('../../corpus/macros/410_json_codable/esperado');
  final table = _SessionTable();
  final results = <Map<String, Object?>>[];
  for (final line in File('${root.path}/session.dfexec').readAsLinesSync()) {
    final message = Map<String, Object?>.from(jsonDecode(line.substring(2)) as Map);
    table.message(message);
    if (message['t'] == 'macro.result') {
      results.add(Map<String, Object?>.from(message['result'] as Map));
    }
  }
  final actual = assembleAugmentation(
    results,
    table,
    header: 'augment library',
    uri: 'package:caso_json/modelos.dart',
  );
  final expected = File('${root.path}/modelos.augmentation.dart').readAsStringSync();
  if (actual != expected) {
    var position = 0;
    while (position < actual.length && position < expected.length && actual[position] == expected[position]) {
      position++;
    }
    throw StateError('augmentation differs from the CFE at character $position '
        '(actual ${actual.length}, expected ${expected.length})');
  }
  print('Dart assembly: ${results.length} macro results identical to CFE 3.6.2 (${actual.length} characters)');
}
