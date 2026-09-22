// Desestruturação de records: final (a, b) = f(), (:x, :y), aninhado, renomeação, for-in, swap, atribuição, _.
(int, String) f() => (1, 'um');

({int x, int y}) ponto() => (x: 3, y: 4);

(int, (String, bool)) aninhado() => (9, ('nove', true));

(int soma, int produto) somaProduto(int a, int b) => (a + b, a * b);

({int min, int max}) minMax(List<int> xs) {
  var mn = xs.first;
  var mx = xs.first;
  for (final x in xs) {
    if (x < mn) mn = x;
    if (x > mx) mx = x;
  }
  return (min: mn, max: mx);
}

void main() {
  final (a, b) = f();
  print(a);
  print(b);
  final (int c, String d) = f();
  print('$c-$d');

  var (:x, :y) = ponto();
  print(x);
  print(y);
  x += 10;
  print(x + y);

  final (x: px, y: py) = ponto();
  print(px * py);

  final (n, (s, flag)) = aninhado();
  print(n);
  print(s);
  print(flag);
  final (_, (String texto, _)) = aninhado();
  print(texto);

  final (soma, produto) = somaProduto(3, 4);
  print(soma);
  print(produto);
  final (int sm, int pr) = somaProduto(2, 5);
  print('$sm $pr');
  final (:min, :max) = minMax([4, 2, 8]);
  print('$min $max');
  final (min: mn, max: mx) = minMax([1]);
  print('$mn $mx');

  final pares = [(1, 'a'), (2, 'b'), (3, 'c')];
  for (final (num, letra) in pares) {
    print('$num=$letra');
  }
  for (final (int num, String letra) in pares) {
    print('$letra$num');
  }
  final nomeados = [(id: 1, nome: 'x'), (id: 2, nome: 'y')];
  for (final (:id, :nome) in nomeados) {
    print('$id:$nome');
  }
  for (final (id: i, nome: _) in nomeados) {
    print(i * 100);
  }

  var p = 1;
  var q = 2;
  (p, q) = (q, p);
  print('$p $q');
  (p, q) = (p + q, p - q);
  print('$p $q');
  var r = 0;
  (p, (q, r)) = (7, (8, 9));
  print('$p $q $r');
  var nomeA = '';
  var nomeB = '';
  (x: nomeA, y: nomeB) = (x: 'primeiro', y: 'segundo');
  print('$nomeA $nomeB');
  var contador = 0;
  String? rotulo;
  (contador, rotulo) = (contador + 1, 'definido');
  print('$contador $rotulo');
  (contador, rotulo) = (contador + 1, null);
  print('$contador $rotulo');

  final (primeiro, _, terceiro) = (10, 20, 30);
  print(primeiro + terceiro);
  final (_, _, ultimo) = (1, 2, 3);
  print(ultimo);
  final (unico,) = (99,);
  print(unico);

  int fibN(int k) {
    var (ant, atual) = (0, 1);
    for (var i = 0; i < k; i++) {
      (ant, atual) = (atual, ant + atual);
    }
    return ant;
  }

  print(List.generate(10, fibN));

  final (List<int> xs, Map<String, int> ys) = ([1, 2], {'k': 3});
  print(xs.length + ys['k']!);
  final (Object obj, num numero) = ('o', 2);
  print('$obj $numero');
  final ((a1, a2), (b1, b2)) = ((1, 2), (3, 4));
  print(a1 * b1 + a2 * b2);
  final (int? talvez, String? talvezS) = (null, 'x');
  print(talvez);
  print(talvezS);
}
