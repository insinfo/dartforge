// Records: posicionais, nomeados, mistos, $1/$2, toString, igualdade estrutural, retorno de função, aninhados.
(int, String) parIntStr() => (7, 'sete');

({int a, String b}) nomeado() => (a: 1, b: 'um');

(int, {String nome, bool ativo}) misto(int id) => (id, nome: 'n$id', ativo: id.isOdd);

(int min, int max) extremos(List<int> xs) {
  var mn = xs.first;
  var mx = xs.first;
  for (final x in xs) {
    if (x < mn) mn = x;
    if (x > mx) mx = x;
  }
  return (mn, mx);
}

((int, int), (int, int)) segmento() => ((0, 0), (3, 4));

(int, int) troca((int, int) p) => (p.$2, p.$1);

void main() {
  final p = (1, 'a');
  print(p);
  print(p.$1);
  print(p.$2);
  print(p.$1 + 1);
  print(p.$2.toUpperCase());

  final n = (x: 10, y: 20);
  print(n);
  print(n.x);
  print(n.y);
  print(n.x + n.y);

  final m = (5, 'cinco', nome: 'numero', par: false);
  print(m);
  print(m.$1);
  print(m.$2);
  print(m.nome);
  print(m.par);

  print(parIntStr());
  print(parIntStr().$2.length);
  print(nomeado());
  print(nomeado().b);
  print(misto(3));
  print(misto(4).ativo);
  print(extremos([4, 9, 1, 7]));
  print(extremos([4, 9, 1, 7]).$2 - extremos([4, 9, 1, 7]).$1);
  print(segmento());
  print(segmento().$2.$1);
  print(troca((1, 2)));
  print(troca(troca((1, 2))));

  print((1, 2) == (1, 2));
  print((1, 2) == (2, 1));
  print((a: 1, b: 2) == (b: 2, a: 1));
  print((1, 'x') == (1, 'y'));
  print((1, nome: 'a') == (1, nome: 'a'));
  const c1 = (1, 2);
  const c2 = (1, 2);
  print(identical(c1, c2));

  final lista = [(1, 'um'), (2, 'dois'), (3, 'tres')];
  print(lista);
  print(lista[1].$2);
  print(lista.map((r) => r.$1 * 10).toList());
  print(lista.where((r) => r.$2.length == 4).map((r) => r.$1).toList());
  final mapa = {'p': (0, 0), 'q': (1, 1)};
  print(mapa['q']);
  print(mapa['q']!.$1);

  final aninhado = (1, (2, (3, 'fundo')));
  print(aninhado);
  print(aninhado.$2.$2.$2);
  final comLista = ([1, 2], {'k': 'v'});
  print(comLista);
  print(comLista.$1.length);
  final comRecordNomeado = (pos: (1, 2), tam: (w: 3, h: 4));
  print(comRecordNomeado);
  print(comRecordNomeado.tam.h);

  (int, String) tipado = (9, 'nove');
  print(tipado);
  ({int a, String b}) tipadoNomeado = (b: 'b', a: 8);
  print(tipadoNomeado);
  final (int, int)? nulo = null;
  print(nulo);
  print(nulo?.$1);
  final Object obj = (1, 2);
  print(obj is (int, int));
  print(obj is (String, int));
  print(obj is (int, int, int));
  print(obj is Record);
  print((n: 1) is ({int n}));
  print((n: 1) is ({int m}));
  print(('s',).$1);
  print(('s',));
  final unico = (42,);
  print(unico.$1);
}
