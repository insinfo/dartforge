// Padrões em declarações (listas, mapas, objetos) e em for-in; switch sobre (int, int) como tabela verdade.
class Ponto {
  final int x;
  final int y;
  Ponto(this.x, this.y);
}

String tabelaE(int a, int b) => switch ((a, b)) {
      (0, 0) => '0',
      (0, 1) => '0',
      (1, 0) => '0',
      (1, 1) => '1',
      _ => '?',
    };

String tabelaOuX(int a, int b) => switch ((a, b)) {
      (0, 0) || (1, 1) => '0',
      (0, 1) || (1, 0) => '1',
      _ => '?',
    };

void main() {
  final [a, b] = [1, 2];
  print(a + b);
  final [primeiro, ...resto] = [10, 20, 30];
  print(primeiro);
  print(resto);
  final [..., ultimo] = ['x', 'y', 'z'];
  print(ultimo);
  final [int p, int q, int r] = [3, 4, 5];
  print(p * q * r);
  final [[a1, a2], [b1, b2]] = [[1, 2], [3, 4]];
  print(a1 + a2 + b1 + b2);

  final {'x': x, 'y': y} = {'x': 7, 'y': 8};
  print(x * y);
  final Map<String, dynamic> registro = {
    'nome': 'ana',
    'tags': ['a', 'b'],
  };
  final {'nome': String nome, 'tags': [String t1, ...]} = registro;
  print('$nome $t1');
  final {1: um, 2: dois} = {1: 'um', 2: 'dois', 3: 'tres'};
  print('$um $dois');

  {
    final Ponto(:x, :y) = Ponto(5, 6);
    print(x + y);
  }
  final Ponto(x: px, y: py) = Ponto(1, 9);
  print(px - py);
  final String(length: tamanho, isEmpty: vazia) = 'quatro';
  print('$tamanho $vazia');

  final mapa = {'a': 1, 'b': 2, 'c': 3};
  for (final (k, v) in mapa.entries.map((e) => (e.key, e.value))) {
    print('$k -> $v');
  }
  for (final MapEntry(:key, :value) in mapa.entries) {
    print('$key=$value');
  }
  for (final MapEntry(key: k, value: v) in mapa.entries.where((e) => e.value.isOdd)) {
    print('impar $k $v');
  }
  var soma = 0;
  for (final (_, v) in mapa.entries.map((e) => (e.key, e.value))) {
    soma += v;
  }
  print(soma);

  final pontos = [Ponto(1, 2), Ponto(3, 4)];
  for (final Ponto(:x, :y) in pontos) {
    print('${x * y}');
  }
  final linhas = [[1, 2, 3], [4, 5, 6]];
  for (final [inicio, ..., fim] in linhas) {
    print('$inicio..$fim');
  }
  final pares = [(1, 'a'), (2, 'b')];
  for (final (n, s) in pares) {
    print('$s$n');
  }
  for (final (i, (n, s)) in pares.indexed) {
    print('$i:$n$s');
  }
  for (final (:index, :value) in ['p', 'q'].indexed.map((e) => (index: e.$1, value: e.$2))) {
    print('$index $value');
  }

  print('E:');
  for (final (i, j) in [(0, 0), (0, 1), (1, 0), (1, 1)]) {
    print('$i $j -> ${tabelaE(i, j)}');
  }
  print('XOR:');
  for (final (i, j) in [(0, 0), (0, 1), (1, 0), (1, 1)]) {
    print('$i $j -> ${tabelaOuX(i, j)}');
  }
  print(tabelaE(2, 2));

  final valores = <Object>[[1, 2], (3, 4), {'k': 5}, Ponto(6, 7)];
  for (final v in valores) {
    final total = switch (v) {
      [int m, int n] => m + n,
      (int m, int n) => m + n,
      {'k': int m} => m,
      Ponto(:final x, :final y) => x + y,
      _ => -1,
    };
    print(total);
  }

  var (contagem, acumulado) = (0, '');
  for (final letra in ['a', 'b', 'c']) {
    (contagem, acumulado) = (contagem + 1, acumulado + letra);
  }
  print('$contagem $acumulado');
}
