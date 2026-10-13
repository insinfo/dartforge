import 'dart:convert';
import 'dart:io';

import 'package:json/json.dart' as json;
import 'package:macros/macros.dart';
import 'package:macros/src/executor/execute.dart';
import 'package:macros/src/executor/model.dart';
import 'package:macros/src/executor/result.dart';

final class _Query {
  final String kind;
  final Map<String, Object?> args;
  final Object? value;
  final Map<String, Object?>? error;
  _Query(this.kind, this.args, this.value, this.error);
}

bool _equal(Object? a, Object? b) {
  if (a is Map && b is Map) {
    return a.length == b.length && a.keys.every((key) => b.containsKey(key) && _equal(a[key], b[key]));
  }
  if (a is List && b is List) {
    return a.length == b.length && List.generate(a.length, (i) => i).every((i) => _equal(a[i], b[i]));
  }
  return a == b;
}

final class _RecordedHost implements Host {
  final List<_Query> pending;
  _RecordedHost(this.pending);

  @override
  Future<Object?> query(String kind, Map<String, Object?> args) async {
    final index = pending.indexWhere((p) => p.kind == kind && _equal(p.args, args));
    if (index < 0) throw StateError('query missing from the oracle: $kind $args');
    final answer = pending.removeAt(index);
    final error = answer.error;
    if (error != null) {
      throw HostError(error['kind'] as String, error['message'] as String);
    }
    return answer.value;
  }
}

Macro _macro(String name) => switch (name) {
      'Endereco' || 'Usuario' => const json.JsonCodable(),
      'SoSaida' => const json.JsonEncodable(),
      'SoEntrada' => const json.JsonDecodable(),
      _ => throw StateError('unknown macro in the corpus: $name'),
    };

Future<void> main() async {
  final lines = File('../../corpus/macros/410_json_codable/esperado/session.dfexec').readAsLinesSync();
  final executions = <int, Map<String, Object?>>{};
  final results = <int, Map<String, Object?>>{};
  final queries = <int, Map<String, Object?>>{};
  final byExecution = <int, List<_Query>>{};
  for (final line in lines) {
    final m = Map<String, Object?>.from(jsonDecode(line.substring(2)) as Map);
    final id = m['id'];
    switch (m['t']) {
      case 'macro.execute':
        executions[id as int] = m;
      case 'macro.result':
        results[id as int] = Map<String, Object?>.from(m['result'] as Map);
      case 'macro.query':
        queries[id as int] = m;
      case 'macro.response':
        final request = queries.remove(id);
        if (request == null) throw StateError('response $id without a query');
        (byExecution[request['execution'] as int] ??= []).add(_Query(
          request['kind'] as String,
          Map<String, Object?>.from(request['args'] as Map),
          m['value'],
          m['error'] is Map ? Map<String, Object?>.from(m['error'] as Map) : null,
        ));
    }
  }

  for (final entry in executions.entries) {
    final id = entry.key;
    final execution = entry.value;
    final host = _RecordedHost(byExecution[id] ?? []);
    final model = Model(host)..receive(Map<String, Object?>.from(execution['model'] as Map));
    final targetJson = Map<String, Object?>.from(execution['target'] as Map);
    final target = model.declaration(targetJson);
    final name = (targetJson['ident'] as Map)['name'] as String;
    final phase = Phase.values.byName(execution['phase'] as String);
    final actual = (await executePhase(_macro(name), phase, target, Introspector(model))).toJson();
    final expected = results[id];
    if (!_equal(actual, expected)) {
      throw StateError('result $id ($name, $phase) diverged from the CFE:\n'
          'actual: ${jsonEncode(actual)}\nexpected: ${jsonEncode(expected)}');
    }
    if (host.pending.isNotEmpty) {
      throw StateError('execution $id left ${host.pending.length} unused queries');
    }
  }
  print('executor in the same isolate: ${executions.length} results equal to the CFE session');
}
