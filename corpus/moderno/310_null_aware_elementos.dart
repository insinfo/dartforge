// requer-dart: 3.13
// Elementos null-aware (3.8): `?e` em lista e conjunto, `?k: v`, `k: ?v` e
// `?k: ?v` em mapa. Ordem de avaliação: a chave antes do valor; chave
// null-aware nula descarta a entrada **sem avaliar o valor**.
String? ev(String? s) {
  print('ev($s)');
  return s;
}

int? talvez(int? x) => x;

void main() {
  String? n;
  String? s = 'x';
  var l = [?ev(n), ?ev(s), 'fim'];
  print(l);
  print(l is List<String>);
  var st = {?ev(n), ?ev(s)};
  print(st);
  print(st is Set<String>);
  var m = {?ev(n): 1, 'k': ?ev(n), ?ev(s): ?ev(s), 'z': ?ev('v')};
  print(m);
  print(m is Map<String, Object>);

  // Contexto anulável para o operando, elemento não anulável.
  List<int> li = [?talvez(1), ?talvez(null), 3];
  print(li);
  Map<String, int> mi = {'a': ?talvez(null), 'b': ?talvez(2)};
  print(mi);

  // Dentro de if/for de coleção.
  var ns = <int?>[1, null, 3];
  print([for (var x in ns) ?x]);
  print([if (s != null) ?talvez(null) else 0, if (s != null) ?talvez(9)]);
  print({for (var x in ns) ?x});

  // Constante.
  const int? c1 = null;
  const int? c2 = 4;
  const cl = [?c1, ?c2];
  print(cl);
  const cm = {'um': ?c1, 'dois': ?c2};
  print(cm);
}
