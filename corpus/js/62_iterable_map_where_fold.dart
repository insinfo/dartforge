// Iterable: map/where/fold/reduce/forEach/expand/toList/toSet/elementAt/followedBy e laziness de map/where.
void main() {
  final xs = [1, 2, 3, 4, 5];
  print(xs.map((x) => x * 10).toList());
  print(xs.where((x) => x.isOdd).toList());
  print(xs.fold<int>(0, (a, b) => a + b));
  print(xs.fold('', (a, b) => '$a$b'));
  print(xs.reduce((a, b) => a * b));
  print(xs.reduce((a, b) => a > b ? a : b));
  try {
    <int>[].reduce((a, b) => a + b);
  } catch (e) {
    print('reduce vazio: ${e is StateError}');
  }
  print(<int>[].fold<int>(100, (a, b) => a + b));

  xs.forEach((x) => print('forEach $x'));
  print(xs.expand((x) => [x, -x]).toList());
  print(xs.expand((x) => x.isEven ? [x] : <int>[]).toList());
  print(xs.map((x) => x % 3).toSet());
  print(xs.contains(3));
  print(xs.map((x) => x * 2).contains(3));
  print(xs.where((x) => x > 2).length);
  print(xs.map((x) => x + 1).elementAt(0));
  print(xs.where((x) => x.isEven).elementAt(1));
  try {
    xs.where((x) => x > 100).elementAt(0);
  } catch (e) {
    print('elementAt fora: ${e is RangeError || e is StateError}');
  }
  print(xs.followedBy([6, 7]).toList());
  print(xs.take(2).followedBy(xs.skip(4)).toList());
  print(xs.map((x) => x.toString()).join('|'));
  print(xs.where((x) => x > 1).map((x) => x * x).where((x) => x < 20).toList());
  print(xs.map((x) => x.isEven).toList());
  print(xs.map((x) => x.isEven).toSet());

  // laziness: map só executa ao iterar
  print('--- lazy ---');
  final mapeado = xs.map((x) {
    print('mapeando $x');
    return x * 2;
  });
  print('criado');
  print(mapeado.first);
  print('primeiro lido');
  print(mapeado.toList());
  print('iterado de novo:');
  print(mapeado.length);
  print(mapeado.take(2).toList());

  // where também é preguiçoso e reexecuta
  final filtrado = xs.where((x) {
    print('testando $x');
    return x > 3;
  });
  print('where criado');
  print(filtrado.isEmpty);
  print(filtrado.toList());

  // toList materializa: não reexecuta
  final materializado = xs.map((x) {
    print('materializando $x');
    return x;
  }).toList();
  print(materializado.length);
  print(materializado.first);

  // encadeamento preguiçoso com take: para cedo
  final cadeia = xs.map((x) {
    print('cadeia $x');
    return x;
  }).take(2);
  print(cadeia.toList());

  // any/every param cedo
  print(xs.any((x) {
    print('any $x');
    return x == 2;
  }));
  print(xs.every((x) {
    print('every $x');
    return x < 3;
  }));

  // map com índice via asMap
  print(xs.asMap().entries.map((e) => '${e.key}:${e.value}').join(' '));

  // fold com acumulador de tipo diferente
  final agrupado = xs.fold(<String, List<int>>{}, (m, x) {
    m.putIfAbsent(x.isEven ? 'par' : 'ímpar', () => []).add(x);
    return m;
  });
  print(agrupado);

  // firstWhere/lastWhere/singleWhere em Iterable preguiçoso
  final pares = xs.where((x) => x.isEven);
  print(pares.first);
  print(pares.last);
  print(xs.where((x) => x == 4).single);

  // Iterable de String
  print('abc'.split('').map((c) => c.toUpperCase()).toList());
  print('a,b,,c'.split(',').where((s) => s.isNotEmpty).toList());

  // toList growable false
  final fixo = xs.map((x) => x).toList(growable: false);
  try {
    fixo.add(1);
  } catch (e) {
    print('toList fixo: ${e is UnsupportedError}');
  }

  // isEmpty/isNotEmpty/length em Iterables derivados
  print(xs.where((x) => x > 10).isEmpty);
  print(xs.map((x) => x).isNotEmpty);
  print(xs.skip(2).length);
}
