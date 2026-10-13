import 'dart:convert';
import 'dart:io';

import 'package:json/json.dart';
import 'package:macros/src/executor/execute.dart';
import 'package:macros/src/executor/model.dart';
import 'package:macros/src/executor/result.dart';

bool equal(Object? a, Object? b) {
  if (a is Map && b is Map) {
    return a.length == b.length && a.keys.every((key) => b.containsKey(key) && equal(a[key], b[key]));
  }
  if (a is List && b is List) {
    return a.length == b.length && List.generate(a.length, (i) => i).every((i) => equal(a[i], b[i]));
  }
  return a == b;
}

final class _SessionHost implements Host {
  final List<(Map<String, Object?>, Object?)> pending;
  _SessionHost(this.pending);

  @override
  Future<Object?> query(String kind, Map<String, Object?> args) async {
    if (kind != 'resolveIdentifier') throw StateError('query not covered: $kind');
    final index = pending.indexWhere((pair) => equal(pair.$1, args));
    if (index < 0) throw StateError('CFE answer missing: $args');
    return pending.removeAt(index).$2;
  }
}

Future<void> main() async {
  final manifest = jsonDecode(
      File('../../corpus/builders/macros_discovery/lib/modelos.macro_uses.json').readAsStringSync()) as Map;
  final application = (manifest['applications'] as List).singleWhere((a) => a['target'] == 'Endereco') as Map;
  final execution = Map<String, Object?>.from(application['execution'] as Map);
  final lines = File('../../corpus/macros/410_json_codable/esperado/session.dfexec').readAsLinesSync();
  final queries = <int, Map<String, Object?>>{};
  final answers = <(Map<String, Object?>, Object?)>[];
  Map<String, Object?>? expected;
  for (final line in lines) {
    final m = Map<String, Object?>.from(jsonDecode(line.substring(2)) as Map);
    if (m['t'] == 'macro.query' && m['execution'] == 4) {
      queries[m['id'] as int] = Map<String, Object?>.from(m['args'] as Map);
    } else if (m['t'] == 'macro.response' && queries.containsKey(m['id'])) {
      answers.add((queries.remove(m['id'])!, m['value']));
    } else if (m['t'] == 'macro.result' && m['id'] == 4) {
      expected = Map<String, Object?>.from(m['result'] as Map);
    }
  }
  final host = _SessionHost(answers);
  final model = Model(host)..receive(Map<String, Object?>.from(execution['model'] as Map));
  final target = model.declaration(Map<String, Object?>.from(execution['target'] as Map));
  final actual =
      (await executePhase(const JsonCodable(), Phase.declarations, target, Introspector(model))).toJson();
  if (!equal(actual, expected) || host.pending.isNotEmpty) {
    throw StateError('JsonCodable with the analyzer model differs from the CFE phase 2: '
        '${jsonEncode(actual)}');
  }
  print('JsonCodable declarations phase equal to the CFE with the analyzer model (3 queries)');
}
