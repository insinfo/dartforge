// Map: ordem de inserção, []/[]=, remove e reinserção, putIfAbsent, update/updateAll, keys/values/entries, construtores.
void main() {
  final m = {'b': 2, 'a': 1, 'c': 3};
  print(m);
  print(m['a']);
  print(m['z']);
  print(m.length);
  m['d'] = 4;
  print(m);
  m['a'] = 10;
  print(m);

  // remover e reinserir muda a posição
  print(m.remove('a'));
  print(m.remove('zzz'));
  print(m);
  m['a'] = 1;
  print(m);
  print(m.keys.toList());
  print(m.values.toList());

  // putIfAbsent
  print(m.putIfAbsent('b', () => 99));
  print(m.putIfAbsent('e', () => 5));
  print(m);

  // update / updateAll
  print(m.update('b', (v) => v * 100));
  print(m.update('novo', (v) => v + 1, ifAbsent: () => 0));
  try {
    m.update('inexistente', (v) => v);
  } catch (e) {
    print('update sem ifAbsent: ${e is ArgumentError}');
  }
  m.updateAll((k, v) => v + 1);
  print(m);

  // entries e forEach
  for (final e in m.entries) {
    print('${e.key}:${e.value}');
  }
  m.forEach((k, v) => print('$k -> $v'));

  print(m.containsKey('c'));
  print(m.containsKey('x'));
  print(m.containsValue(1));
  print(m.containsValue(-1));

  // map() produz novo Map
  print(m.map((k, v) => MapEntry(k.toUpperCase(), v * 2)));
  print(m.map((k, v) => MapEntry(v, k)));

  // construtores
  print(Map.fromIterables(['x', 'y'], [1, 2]));
  print(Map.fromIterable([1, 2, 3], key: (x) => 'k$x', value: (x) => x * x));
  print(Map.fromEntries([MapEntry('p', 1), MapEntry('q', 2)]));
  final copia = Map.of(m);
  copia['extra'] = -1;
  print(copia.length);
  print(m.length);
  final copiaFrom = Map<String, num>.from(m);
  copiaFrom['dec'] = 0.5;
  print(copiaFrom);

  // addAll e addEntries
  m.addAll({'f': 6, 'b': 0});
  print(m);
  m.addEntries([MapEntry('g', 7)]);
  print(m);

  // removeWhere
  m.removeWhere((k, v) => v > 5);
  print(m);

  // {} vazio é Map
  final vazio = {};
  print(vazio is Map);
  print(vazio is Set);
  print(vazio.isEmpty);
  print(vazio);
  final vazioTipado = <String, int>{};
  print(vazioTipado.isEmpty);

  // chaves de tipos diversos
  final porInt = {3: 'três', 1: 'um', 2: 'dois'};
  print(porInt);
  print(porInt[1]);
  final porBool = {true: 'sim', false: 'não'};
  print(porBool[false]);

  // [] com chave ausente e ?? e putIfAbsent com contador
  final contagem = <String, int>{};
  for (final p in 'a b a c b a'.split(' ')) {
    contagem[p] = (contagem[p] ?? 0) + 1;
  }
  print(contagem);

  // clear
  contagem.clear();
  print(contagem);

  // valores nulos: containsKey vs []
  final comNulo = <String, int?>{'n': null};
  print(comNulo['n']);
  print(comNulo.containsKey('n'));
  print(comNulo.containsKey('m'));

  // igualdade é de identidade
  final m1 = {'a': 1};
  final m2 = {'a': 1};
  print(m1 == m2);
  print(m1 == m1);

  // toString com aninhamento
  print({
    'lista': [1, 2],
    'mapa': {'x': null},
    'texto': 'oi'
  });

  // keys e values são vistas vivas
  final vivo = {'a': 1};
  final chaves = vivo.keys;
  vivo['b'] = 2;
  print(chaves.toList());
  print(vivo.entries.map((e) => e.value).toList());
}
