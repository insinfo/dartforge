// return dentro de laços, switch e try; early return; funções void; throw como expressão em ?? e ternário.
int primeiroPar(List<int> xs) {
  for (final x in xs) {
    if (x.isEven) return x;
  }
  return -1;
}

String buscaMatriz(List<List<int>> m, int alvo) {
  for (var i = 0; i < m.length; i++) {
    for (var j = 0; j < m[i].length; j++) {
      if (m[i][j] == alvo) return 'achou em $i,$j';
    }
  }
  return 'não achou';
}

String categoria(int n) {
  switch (n) {
    case 0:
      return 'zero';
    case 1:
    case 2:
      return 'pouco';
  }
  return 'muito';
}

int comTry(int n) {
  try {
    if (n < 0) return -100;
    return n * 2;
  } finally {
    print('finally de comTry($n)');
  }
}

int whileRetorna() {
  var i = 0;
  while (true) {
    i++;
    if (i * i > 50) return i;
  }
}

int doWhileRetorna() {
  var i = 10;
  do {
    if (i == 7) return i;
    i--;
  } while (i > 0);
  return -1;
}

void voidComReturn(int n) {
  if (n == 0) {
    print('zero, saindo cedo');
    return;
  }
  print('processando $n');
}

void voidArrow() => print('void arrow');

int semRetornoEmLaco(List<int> xs) {
  var soma = 0;
  for (final x in xs) {
    if (x > 100) {
      return soma;
    }
    soma += x;
  }
  return soma;
}

int lanca(String m) => throw StateError(m);

void main() {
  print(primeiroPar([1, 3, 4, 6]));
  print(primeiroPar([1, 3]));
  print(buscaMatriz([
    [1, 2],
    [3, 4]
  ], 4));
  print(buscaMatriz([
    [1, 2]
  ], 9));
  for (final n in [0, 1, 2, 3]) {
    print('$n ${categoria(n)}');
  }
  print(comTry(5));
  print(comTry(-5));
  print(whileRetorna());
  print(doWhileRetorna());
  voidComReturn(0);
  voidComReturn(3);
  voidArrow();
  print(semRetornoEmLaco([1, 2, 500, 3]));
  print(semRetornoEmLaco([1, 2, 3]));

  // throw como expressão dentro de ??
  int? nulo;
  try {
    final r = nulo ?? (throw ArgumentError('era nulo'));
    print(r);
  } on ArgumentError catch (e) {
    print('capturou: ${e.message}');
  }
  int? cheio = 4;
  print(cheio ?? (throw ArgumentError('não lança')));

  // throw dentro de ternário
  try {
    final r = cheio > 3 ? throw Exception('muito grande') : cheio;
    print(r);
  } catch (e) {
    print(e);
  }
  print(cheio > 10 ? throw Exception('nunca') : cheio);

  // throw em arrow function chamada dentro de laço com return
  int atéFalhar() {
    for (var i = 0; i < 5; i++) {
      try {
        if (i == 2) lanca('parou em $i');
        print('ok $i');
      } on StateError catch (e) {
        print(e.message);
        return i;
      }
    }
    return -1;
  }

  print(atéFalhar());

  // return dentro de forEach closure não sai da função externa
  int comForEach() {
    var vistos = 0;
    [1, 2, 3].forEach((x) {
      if (x == 2) return;
      vistos++;
    });
    return vistos;
  }

  print(comForEach());
  print('fim');
}
