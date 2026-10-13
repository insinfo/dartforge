import 'package:macros/src/executor/assembly.dart';

Map<String, Object?> code(List<Object?> parts) => {'k': 'raw', 'p': parts};

final class Table implements AssemblyResolver {
  @override
  ResolvedIdentifier identifier(int id) => switch (id) {
        1 => const ResolvedIdentifier('Comparable', IdentifierKind.topLevel, uri: 'dart:core'),
        2 => const ResolvedIdentifier('Point', IdentifierKind.topLevel, uri: 'package:a/a.dart'),
        _ => throw StateError('identifier $id'),
      };

  @override
  AugmentedType augmentedType(int id) => switch (id) {
        2 => AugmentedType('class', const ['abstract', 'base'], 'Point', [
            code(['T'])
          ]),
        _ => throw StateError('type $id'),
      };

  @override
  Map<String, Object?>? inferredType(int key) => null;
}

void check(String actual, String expected) {
  if (actual != expected) {
    throw StateError('different augmentation:\n--- actual ---\n$actual--- expected ---\n$expected');
  }
}

void main() {
  // The same order, header and imports contract as `assembly.rs`, whose
  // output was checked against CFE 3.6.2.
  final result = [
    {
      'library': [
        code(['class PointTwin {}'])
      ],
      'interfaces': [
        [
          2,
          [
            code([
              {'i': 1},
              '<',
              {'i': 2},
              '>'
            ])
          ],
        ],
      ],
    },
    {
      'types': [
        [
          2,
          [
            code(['  int get x => 1;'])
          ],
        ],
      ],
    },
  ];
  check(
    assembleAugmentation(result, Table(), header: 'part of', uri: 'a.dart'),
    "part of 'a.dart';\n\n"
    "import 'dart:core' as prefix0;\n"
    "import 'package:a/a.dart' as prefix1;\n\n"
    'class PointTwin {}\n'
    'augment abstract base class Point<T> implements prefix0.Comparable<prefix1.Point> {\n'
    '  int get x => 1;\n'
    '}\n',
  );
  check(
    assembleAugmentation([
      {
        'library': [
          code([
            'prefix prefix0 ',
            {'i': 1}
          ])
        ],
      },
    ], Table(), header: 'augment library', uri: 'a.dart'),
    "augment library 'a.dart';\n\nimport 'dart:core' as prefix1_0;\n\nprefix prefix0 prefix1_0.Comparable\n",
  );
  print('Dart assembly: 2 cases equal to the Rust/CFE contract');
}
