// A large literal whose elements are nested literals of constant scalars
// (the Unicode tables of `unorm_dart`) is built from a data table
// (`lower/literals.rs`, `fill_from_table`; docs/NATIVO-PRODUCAO-GRANDE.md §7).
// Every nested literal is still a fresh, mutable collection with the static
// type of its literal, filled in order; a literal with a type parameter or a
// non-constant element keeps the ordinary path.

final Map<int, Map<int, List<Object?>>> table = {
  0: {60: [null, null, {824: 8814}], 61: [null, 230, {}]},
  256: {256: [[65, 772], null, {}], 257: [[97, 772], 1, {768: 7700}]},
  512: {512: [[65, 783], null, {}]},
  768: {768: [null, 230, {}], 769: [null, 230, {}]},
  1024: {1024: [[1045, 768], null, {}]},
  1280: {1415: [[1381, 1410], 256, {}]},
  1536: {1552: [null, 230, {}]},
  1792: {1809: [null, 36, {}], 1840: [null, 230, {1: 2, 3: 4}]},
  2048: {},
};

final names = {
  'a': ['x', 'y', true, false, null, 1.5, -2, -0.5],
  'b': <String>{'p', 'q'},
  'c': <num>[1, 2.5],
  'd': <String, List<int>>{'k': [1, 2, 3]},
  'e': [<double>[1, 2]],
  'f': [],
  'g': {},
  'h': <Object?>{},
};

List<List<T>> generic<T>(T a, T b) => [
      [a],
      [b],
      <T>[],
      [a, b],
      [b, a],
      [a, a],
      [b, b],
      [a, b, a],
    ];

int counter = 0;
int next() => ++counter;

void main() {
  print(table);
  print(table.runtimeType);
  print(table[0].runtimeType);
  print(table[0]![60].runtimeType);
  print(table[0]![60]![2].runtimeType);
  print(table[256]![257]![0].runtimeType);
  print(table[2048].runtimeType);
  print(names);
  for (final v in names.values) {
    print('${v.runtimeType} $v');
  }
  // Fresh and mutable: changing one does not change the other evaluation.
  final inner = table[0]![61]!;
  inner.add('added');
  (inner[2] as Map)[5] = 6;
  print(table[0]);
  Map<int, Map<int, List<Object?>>> build() => {
        1: {1: [1], 2: [2]},
        2: {3: [3]},
        3: {},
        4: {},
        5: {},
        6: {},
        7: {},
        8: {8: [null]},
      };
  final x = build(), y = build();
  print(identical(x, y));
  print(identical(x[1], y[1]));
  print(identical(x[1]![1], y[1]![1]));
  x[1]![1]!.add(99);
  print('$x $y');
  try {
    (table[0]![60]! as List<Map<int, int>?>).add(null);
    print('cast ok');
  } catch (e) {
    print('cast: ${e.runtimeType}');
  }
  try {
    (names['e'] as List)[0].add('s');
  } catch (e) {
    print('typed: ${e.runtimeType}');
  }
  print(generic<int>(1, 2));
  print(generic<String>('a', 'b').runtimeType);
  // A non-constant element keeps the order of evaluation.
  final m = {
    1: [next()],
    2: [next()],
    3: [next()],
    4: [next()],
    5: [next()],
    6: [next()],
    7: [next()],
    8: [next()],
  };
  print('$m $counter');
}
