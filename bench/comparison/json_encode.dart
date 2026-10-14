// Reusable inputs and untimed content validation isolate encoding work.
import 'dart:convert';

int checksum(String value) {
  var hash = 0;
  for (var i = 0; i < value.length; i++) {
    hash = (hash * 131 + value.codeUnitAt(i)) & 0x7fffffff;
  }
  return hash;
}

List<Map<String, Object?>> inputGraph(String text) => [
      for (var i = 0; i < 24; i++)
        <String, Object?>{
          'id': i,
          'name': '$text:$i',
          'active': i % 3 == 0,
          'metadata': <String, Object?>{
            'title': text,
            'values': <int>[i, i + 1, i * 7],
            'tags': <String>[text, 'quote" slash\\ newline\n'],
            'optional': null,
          },
        },
    ];

String buildBuffer(List<String> chunks) {
  final buffer = StringBuffer();
  for (final chunk in chunks) {
    buffer.write(chunk);
    buffer.write('|');
  }
  return buffer.toString();
}

Map<String, Object?> measure(String kernel, String Function() operation,
    int durationMillis, bool validateOnly) {
  final expected = operation();
  if (validateOnly) {
    return <String, Object?>{
      'kernel': kernel,
      'iterations': 1,
      'elapsed_micros': 0,
      'encoded_units': expected.length,
      'checksum': checksum(expected),
      'consumed': expected.length + expected.codeUnitAt(0),
      'warmup_iterations': 0,
      'validation_outside_timing': true,
    };
  }
  for (var warmup = 0; warmup < 8; warmup++) {
    if (operation() != expected) throw StateError('Unstable warmup output');
  }
  var iterations = 0;
  var consumed = 0;
  var last = '';
  final stopwatch = Stopwatch()..start();
  do {
    for (var batch = 0; batch < 16 && iterations < 100000; batch++) {
      last = operation();
      // Consume length and one changing position, not all encoded code units.
      consumed =
          (consumed + last.length + last.codeUnitAt(iterations % last.length)) &
              0x7fffffff;
      iterations++;
    }
  } while (
      stopwatch.elapsedMilliseconds < durationMillis && iterations < 100000);
  stopwatch.stop();
  final elapsed = stopwatch.elapsedMicroseconds;
  var expectedConsumed = 0;
  for (var i = 0; i < iterations; i++) {
    expectedConsumed = (expectedConsumed +
            expected.length +
            expected.codeUnitAt(i % expected.length)) &
        0x7fffffff;
  }
  if (last != expected || consumed != expectedConsumed) {
    throw StateError('Encoded content changed during measurement');
  }
  return <String, Object?>{
    'kernel': kernel,
    'iterations': iterations,
    'elapsed_micros': elapsed,
    'encoded_units': last.length,
    'checksum': checksum(last),
    'consumed': consumed,
    'warmup_iterations': 8,
    'validation_outside_timing': true,
  };
}

void main(List<String> arguments) {
  var durationMillis = 500;
  var validateOnly = false;
  for (final argument in arguments) {
    if (argument == '--validate') {
      validateOnly = true;
      continue;
    }
    if (!argument.startsWith('--duration-ms=')) {
      throw ArgumentError('Unknown argument: $argument');
    }
    durationMillis = int.parse(argument.substring(14));
  }
  if (durationMillis < 50 || durationMillis > 1000) {
    throw ArgumentError('Expected 50..1000 milliseconds per kernel');
  }
  final scenarios = <String, String>{
    'ascii': 'abcdefghijklmnopqrstuvwxyz012345',
    'latin1': 'áéíóúàèìòùâêîôûäëïöüñçåøæß123456',
    'mixed_utf16': 'ASCII é Ω 漢字 🙂 mixed UTF-16 tail',
  };
  final results = <Map<String, Object?>>[];
  for (final scenario in scenarios.entries) {
    final graph = inputGraph(scenario.value);
    final chunks = <String>[
      for (var i = 0; i < 48; i++) '${scenario.value}:$i',
    ];
    final json = measure(
        'json_encode', () => jsonEncode(graph), durationMillis, validateOnly);
    final buffer = measure('string_buffer', () => buildBuffer(chunks),
        durationMillis, validateOnly);
    results.add(<String, Object?>{
      'scenario': scenario.key,
      'graph_rows': graph.length,
      'buffer_chunks': chunks.length,
      'json': json,
      'string_buffer': buffer,
    });
  }
  print(jsonEncode(<String, Object?>{
    'schema': 1,
    'validation_only': validateOnly,
    'duration_millis_per_kernel': durationMillis,
    'iteration_cap_per_kernel': 100000,
    'units': 'utf16_code_units',
    'reusable_input_graph': true,
    'speedup_asserted': false,
    'scenarios': results,
  }));
}
