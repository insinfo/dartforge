// geração recursiva: permutações, subconjuntos, torre de hanói, combinações, com contagem de chamadas.
int chamadasPerm = 0;

List<List<int>> permutacoes(List<int> xs) {
  chamadasPerm++;
  if (xs.length <= 1) return [List.of(xs)];
  final resultado = <List<int>>[];
  for (var i = 0; i < xs.length; i++) {
    final resto = [...xs.sublist(0, i), ...xs.sublist(i + 1)];
    for (final p in permutacoes(resto)) {
      resultado.add([xs[i], ...p]);
    }
  }
  return resultado;
}

List<List<T>> subconjuntos<T>(List<T> xs) {
  if (xs.isEmpty) return [[]];
  final resto = subconjuntos(xs.sublist(1));
  return [
    ...resto,
    for (final r in resto) [xs[0], ...r],
  ];
}

int movimentos = 0;
void hanoi(int n, String de, String para, String via) {
  if (n == 0) return;
  hanoi(n - 1, de, via, para);
  movimentos++;
  print('move disco $n de $de para $para');
  hanoi(n - 1, via, para, de);
}

List<List<int>> combinacoes(List<int> xs, int k) {
  if (k == 0) return [[]];
  if (xs.length < k) return [];
  final semPrimeiro = combinacoes(xs.sublist(1), k);
  final comPrimeiro = [
    for (final c in combinacoes(xs.sublist(1), k - 1)) [xs[0], ...c]
  ];
  return [...comPrimeiro, ...semPrimeiro];
}

int chamadasAck = 0;
int ackermann(int m, int n) {
  chamadasAck++;
  if (m == 0) return n + 1;
  if (n == 0) return ackermann(m - 1, 1);
  return ackermann(m - 1, ackermann(m, n - 1));
}

void flood(List<List<int>> grade, int r, int c, int cor) {
  if (r < 0 || c < 0 || r >= grade.length || c >= grade[0].length) return;
  if (grade[r][c] != 0) return;
  grade[r][c] = cor;
  flood(grade, r + 1, c, cor);
  flood(grade, r - 1, c, cor);
  flood(grade, r, c + 1, cor);
  flood(grade, r, c - 1, cor);
}

String binario(int n) => n < 2 ? '$n' : binario(n ~/ 2) + '${n % 2}';

List<String> parenteses(int n) {
  final out = <String>[];
  void gera(String atual, int abertos, int fechados) {
    if (atual.length == 2 * n) {
      out.add(atual);
      return;
    }
    if (abertos < n) gera('$atual(', abertos + 1, fechados);
    if (fechados < abertos) gera('$atual)', abertos, fechados + 1);
  }

  gera('', 0, 0);
  return out;
}

void main() {
  final ps = permutacoes([1, 2, 3]);
  for (final p in ps) {
    print(p.join());
  }
  print('${ps.length} permutações em $chamadasPerm chamadas');
  print(permutacoes([]).length);
  print(permutacoes([1, 2, 3, 4]).length);

  final ss = subconjuntos(['a', 'b', 'c']);
  print(ss.map((s) => s.join()).toList());
  print(ss.length);

  hanoi(3, 'A', 'C', 'B');
  print('movimentos $movimentos');

  for (final c in combinacoes([1, 2, 3, 4], 2)) {
    print(c);
  }
  print(combinacoes([1, 2, 3, 4, 5], 3).length);
  print(combinacoes([1, 2], 3));

  print(ackermann(2, 3));
  print('ackermann chamadas $chamadasAck');

  final grade = [
    [0, 0, 1, 0],
    [0, 1, 1, 0],
    [1, 1, 0, 0],
    [0, 0, 0, 0],
  ];
  flood(grade, 0, 0, 7);
  for (final linha in grade) {
    print(linha.join(' '));
  }
  flood(grade, 3, 3, 9);
  print(grade.map((l) => l.join()).join('/'));

  print([0, 1, 2, 5, 10, 255].map(binario).toList());
  print(parenteses(3));
  print(parenteses(4).length);
}
