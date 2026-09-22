// literais de coleção com spread (..., ...?), if/else e for, aninhados, em List/Set/Map, tipos inferidos.
void main() {
  final base = [1, 2];
  List<int>? nulo;
  final talvez = [9];

  // spread e spread nulo
  print([0, ...base, 3]);
  print([...base, ...base]);
  print([...?nulo]);
  print([...?nulo, ...?talvez]);
  print([...base, ...?nulo, 5]);
  print([...{1, 2, 1}]);
  print([...'abc'.split('')]);
  print([...base.map((x) => x * 10)]);

  // if em coleção
  final incluir = true;
  print([1, if (incluir) 2, 3]);
  print([1, if (!incluir) 2, 3]);
  print([if (incluir) 'sim' else 'não']);
  print([if (!incluir) 'sim' else 'não']);
  print([if (incluir) ...base]);
  print([if (incluir) ...base else ...[7, 8]]);
  print([if (!incluir) ...base else ...[7, 8]]);

  // for em coleção
  print([for (var i = 0; i < 3; i++) i * i]);
  print([for (final x in base) x + 100]);
  print([for (final x in base) for (final y in [10, 20]) x + y]);
  print([for (final x in [1, 2, 3, 4]) if (x.isEven) x]);
  print([for (final x in [1, 2, 3]) if (x.isOdd) x else -x]);
  print([for (final x in base) ...[x, x]]);
  print([for (final (a, b) in [(1, 'a'), (2, 'b')]) '$a$b']);
  print([for (final e in {'k': 1}.entries) '${e.key}=${e.value}']);

  // aninhados profundamente
  print([
    for (var i = 1; i <= 3; i++)
      if (i != 2) ...[
        for (var j = 0; j < i; j++) '$i.$j'
      ]
  ]);

  // Set
  print({...base, 2, 3});
  print({if (incluir) 'a', 'b', if (!incluir) 'c'});
  print({for (final x in [1, 2, 3, 1]) x % 2});
  print({...?nulo, ...base});

  // Map
  final extra = {'z': 26};
  Map<String, int>? mapaNulo;
  print({'a': 1, ...extra});
  print({...extra, 'z': 0});
  print({'z': 0, ...extra});
  print({...?mapaNulo, 'q': 1});
  print({if (incluir) 'sim': 1 else 'não': 0});
  print({for (final x in base) 'k$x': x * x});
  print({for (final x in base) if (x > 1) x: 'grande'});
  print({
    'a': 1,
    for (var i = 0; i < 2; i++) 'i$i': i,
    if (incluir) ...{'fim': -1},
  });
  print({...{'x': 1}, ...{'x': 2}});

  // tipos inferidos
  final l1 = [...base, 2.5];
  print(l1 is List<num>);
  print(l1 is List<int>);
  final l2 = [if (incluir) 1 else 'um'];
  print(l2 is List<Object>);
  print(l2 is List<int>);
  final s1 = {for (final x in base) x};
  print(s1 is Set<int>);
  final m1 = {for (final x in base) x: '$x'};
  print(m1 is Map<int, String>);
  final vazio = [...?nulo];
  print(vazio is List<int>);
  print(vazio.isEmpty);

  // spread de Iterable preguiçoso e de Set em List, spread com length
  print([...Iterable.generate(3)].length);
  print([...base.where((x) => x > 1)]);
  print({...[1, 2, 3].map((x) => x % 2)});

  // spread com efeitos: ordem
  List<int> f(String s) {
    print('spread $s');
    return [s.length];
  }

  print([...f('a'), ...f('bb'), if (incluir) ...f('ccc')]);

  // for com variável de iteração modificada e continue-like via if
  print([for (var i = 0; i < 6; i++) if (i % 2 == 0) if (i != 2) i]);

  // const com spread e if
  const c1 = [1, 2];
  const c2 = [...c1, 3];
  print(c2);
  const c3 = {if (true) 'x': 1, ...{'y': 2}};
  print(c3);
  print(identical(c2, const [1, 2, 3]));
}
