// Capture ContextLocator._uniqueSortedPaths on the Dart 3.6.2 VM.
// Usage: dart context_root_order.dart <output.json>
// The same logical path strings are used on every host; no files are loaded.
import 'dart:collection';
import 'dart:convert';
import 'dart:io';

void main(List<String> args) {
  final cases = <Object>[];
  for (final pattern in ['same_length', 'mixed', 'duplicates']) {
    for (final count in [0, 2, 7, 33, 34, 40, 64, 97, 128]) {
      final paths = List.generate(count, (i) {
        if (pattern == 'duplicates') i %= 17;
        final nested =
            pattern == 'same_length' ? '' : 'nested/' * ((i * 7) % 11);
        final unicode = pattern != 'same_length' && i % 3 == 0 ? '_😀' : '';
        return '/roots/${nested}project_${i.toString().padLeft(3, '0')}$unicode';
      });
      final sorted = HashSet<String>.from(paths).toList()
        ..sort((a, b) => a.length - b.length);
      cases.add({
        'pattern': pattern,
        'count': count,
        'expected': sorted.map(paths.indexOf).toList(),
      });
    }
  }
  final sdkRoot = File(Platform.resolvedExecutable).parent.parent;
  final version = File('${sdkRoot.path}/version').readAsStringSync().trim();
  if (version != '3.6.2') throw StateError('expected Dart 3.6.2, got $version');
  final rows = cases.map((row) => '  ${jsonEncode(row)}').join(',\n');
  File(args.single).writeAsStringSync(
    '{"sdk": ${jsonEncode(version)}, "cases": [\n$rows\n]}\n',
  );
}
