// Capture package path normalization and relative resolution on Dart 3.6.2.
// Usage: dart package_uri_paths.dart <output.json>
import 'dart:convert';
import 'dart:io';

void main(List<String> args) {
  final version =
      File('${File(Platform.resolvedExecutable).parent.parent.path}/version')
          .readAsStringSync()
          .trim();
  if (version != '3.6.2') throw StateError('expected Dart 3.6.2, got $version');
  final absolute = [
    'p/%67enerated.dart',
    'p/a%2fb.dart',
    'p/%7e.dart',
    'p/%zz.dart',
    r'p/a\b.dart',
    'p/a//b.dart',
    'p//a.dart',
    'p/../a.dart',
    'p/a%23b.dart',
    'p/café.dart',
    'p/a[b].dart',
    'p/a+b.dart',
    '//host/p/x.dart',
    'p/x.dart?query',
    'p/x/../../other/x.dart',
  ];
  final rows = <Object>[
    for (final path in absolute)
      {
        'kind': 'absolute',
        'reference': path,
        'expected': Uri.parse('package:$path').path
      },
    for (final base in [
      'entry.dart',
      'src/entry.dart',
      'folder space/entry.dart'
    ])
      for (final reference in [
        '../x.dart',
        '../../x.dart',
        '../../../../x.dart',
        '/x.dart',
        'space%20name.dart',
        '%2e%2e/x.dart',
        'dir%2Fpart.dart',
        'x.dart?query',
        '//host/p/x.dart',
        '%67enerated.dart',
        'café.dart',
        r'..\x.dart',
        '',
        './',
        '../a/../../x.dart',
      ])
        {
          'kind': 'relative',
          'base': base,
          'reference': reference,
          'expected': Uri.parse('package:p/$base').resolve(reference).path
        },
  ];
  File(args.single).writeAsStringSync(
    '{"sdk": ${jsonEncode(version)}, "cases": [\n${rows.map((r) => '  ${jsonEncode(r)}').join(',\n')}\n]}\n',
  );
}
