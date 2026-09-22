// Set: literal, add repetido, remove, contains, union/intersection/difference, lookup, retainAll/removeAll, Set.from.
void main() {
  final s = {3, 1, 2};
  print(s);
  print(s.length);
  print(s.add(4));
  print(s.add(1));
  print(s);
  print(s.remove(3));
  print(s.remove(3));
  print(s);
  print(s.contains(2));
  print(s.contains(9));
  print(s.first);
  print(s.last);

  // literal com repetição descarta duplicados mantendo a primeira posição
  print({1, 2, 1, 3, 2});
  print({'b', 'a', 'b'});

  final a = {1, 2, 3, 4};
  final b = {3, 4, 5, 6};
  print(a.union(b));
  print(b.union(a));
  print(a.intersection(b));
  print(b.intersection(a));
  print(a.difference(b));
  print(b.difference(a));
  print(a.union(b).difference(a.intersection(b)));

  // lookup
  print(a.lookup(3));
  print(a.lookup(30));

  // containsAll
  print(a.containsAll([1, 2]));
  print(a.containsAll([1, 9]));
  print(a.containsAll(<int>[]));

  // toList e iteração mantêm ordem de inserção
  print(a.toList());
  for (final x in b) {
    print('b tem $x');
  }

  // Set.from, Set.of, toSet
  print(Set.from([5, 5, 6]));
  print(Set<int>.of([9, 8, 9]));
  print([1, 1, 2].toSet());
  print('banana'.split('').toSet());
  print('banana'.split('').toSet().join());

  // vazio tipado
  final vazio = <int>{};
  print(vazio.isEmpty);
  print(vazio);
  print(vazio.length);
  vazio.addAll([7, 8, 7]);
  print(vazio);

  // retainAll / removeAll / retainWhere / removeWhere
  final c = {1, 2, 3, 4, 5, 6};
  c.retainAll([2, 4, 6, 8]);
  print(c);
  c.removeAll([4]);
  print(c);
  c.addAll([10, 11, 12]);
  c.retainWhere((x) => x.isEven);
  print(c);
  c.removeWhere((x) => x > 10);
  print(c);

  // Set de strings e de records
  final nomes = {'ana', 'bia', 'ana'};
  print(nomes);
  print(nomes.contains('ana'));
  final pontos = {(1, 2), (3, 4), (1, 2)};
  print(pontos);
  print(pontos.length);
  print(pontos.contains((3, 4)));
  print(pontos.contains((4, 3)));
  final nomeados = {(x: 1, y: 2), (y: 2, x: 1)};
  print(nomeados.length);

  // igualdade é de identidade; comparar via containsAll dos dois lados
  final s1 = {1, 2};
  final s2 = {2, 1};
  print(s1 == s2);
  print(s1.containsAll(s2) && s2.containsAll(s1));
  print(s1.length == s2.length);

  // map/where sobre Set
  print(a.map((x) => x * 10).toSet());
  print(a.where((x) => x.isOdd).toSet());
  print(a.map((x) => x % 2).toSet());

  // remover durante iteração de cópia
  final d = {1, 2, 3};
  for (final x in d.toList()) {
    if (x.isOdd) d.remove(x);
  }
  print(d);

  // clear
  d.clear();
  print(d);
  print(d.isEmpty);

  // Set de listas usa identidade
  final l1 = [1];
  final ls = {l1, [1]};
  print(ls.length);
  print(ls.contains(l1));
  print(ls.contains([1]));

  // is Set / is Iterable
  print(a is Set<int>);
  print(a is Iterable<int>);
  print(a is List);

  // union preserva o receptor
  final u = a.union({0});
  print(u);
  print(a);
}
