// Iterable.generate, take de sequência grande, whereType, cast, zip manual, iterator manual (moveNext/current).
void main() {
  print(Iterable.generate(5).toList());
  print(Iterable.generate(4, (i) => i * i).toList());
  print(Iterable.generate(0).toList());

  // sequência grande consumida preguiçosamente por take
  var gerados = 0;
  final grande = Iterable.generate(1000, (i) {
    gerados++;
    return i * 3;
  });
  print(grande.take(4).toList());
  print('gerados $gerados');
  print(grande.skip(997).toList());
  print(grande.where((x) => x % 7 == 0).take(3).toList());
  print(grande.firstWhere((x) => x > 100));
  print(grande.length);

  // expand aninhado e com iterables preguiçosos
  print([1, 2, 3].expand((x) => Iterable.generate(x, (i) => x)).toList());
  print([
    [1, 2],
    [],
    [3]
  ].expand((l) => l).toList());
  print(['ab', 'cd'].expand((s) => s.split('')).join());

  // reduce com tipos
  print(['a', 'b', 'c'].reduce((a, b) => '$a-$b'));
  print([3, 1, 2].reduce((a, b) => a < b ? a : b));
  print([1].reduce((a, b) => a + b));
  print([1.5, 2.25].reduce((a, b) => a + b));

  // whereType e cast
  final mista = <Object?>[1, 'a', 2.5, null, 'b', 3, [4]];
  print(mista.whereType<int>().toList());
  print(mista.whereType<String>().toList());
  print(mista.whereType<num>().toList());
  print(mista.whereType<List>().toList());
  print(mista.whereType<Null>().length);
  final nums = <Object>[1, 2, 3];
  final comoNum = nums.cast<num>();
  print(comoNum.map((n) => n + 1).toList());
  try {
    final errado = nums.cast<String>();
    print(errado.first);
  } catch (e) {
    print('cast preguiçoso falha ao ler: ${e is TypeError}');
  }
  print(nums.cast<String>().length);

  // join com iterables preguiçosos
  print(Iterable.generate(3, (i) => 'i$i').join(','));
  print([1, 2, 3].map((x) => x * x).join(' + '));

  // zip manual com iterators
  final a = [1, 2, 3, 4];
  final b = ['um', 'dois', 'três'];
  final ia = a.iterator;
  final ib = b.iterator;
  final zip = <String>[];
  while (ia.moveNext() && ib.moveNext()) {
    zip.add('${ia.current}=${ib.current}');
  }
  print(zip);

  // iterator manual passo a passo
  final it = [10, 20].iterator;
  print(it.moveNext());
  print(it.current);
  print(it.moveNext());
  print(it.current);
  print(it.moveNext());
  print(it.moveNext());

  // iterator sobre Iterable preguiçoso reexecuta a função
  var chamadas = 0;
  final lazy = [1, 2].map((x) {
    chamadas++;
    return x;
  });
  final it1 = lazy.iterator;
  final it2 = lazy.iterator;
  it1.moveNext();
  it2.moveNext();
  it1.moveNext();
  print('chamadas $chamadas');

  // iterator de Set e de Map.keys
  final si = {'x', 'y'}.iterator;
  final ss = <String>[];
  while (si.moveNext()) {
    ss.add(si.current);
  }
  print(ss);
  final mi = {'k1': 1, 'k2': 2}.entries.iterator;
  mi.moveNext();
  print('${mi.current.key}=${mi.current.value}');

  // Iterable.generate infinito não existe; simular com generate grande e takeWhile
  final ate = Iterable.generate(100000, (i) => i * i).takeWhile((x) => x < 50);
  print(ate.toList());

  // iterable como argumento genérico: soma sem materializar
  var soma = 0;
  for (final x in Iterable.generate(10, (i) => i + 1)) {
    soma += x;
  }
  print(soma);

  // encadeamento longo
  print(Iterable.generate(20)
      .where((x) => x.isEven)
      .map((x) => x ~/ 2)
      .skip(1)
      .take(5)
      .expand((x) => [x, x])
      .toList());

  // toSet de generate e toList de toSet
  print(Iterable.generate(6, (i) => i % 3).toSet().toList());
}
