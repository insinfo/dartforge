// sync*: yield, yield*, avaliação preguiçosa, recursivo (árvore), infinito com take, try/finally, for-in, toList, map/where.
Iterable<int> simples() sync* {
  print('gerador: início');
  yield 1;
  print('gerador: entre 1 e 2');
  yield 2;
  print('gerador: fim');
}

Iterable<int> naturais() sync* {
  var i = 0;
  while (true) {
    yield i++;
  }
}

class No {
  final int valor;
  final List<No> filhos;
  No(this.valor, [this.filhos = const []]);
}

Iterable<int> percorre(No n) sync* {
  yield n.valor;
  for (final f in n.filhos) {
    yield* percorre(f);
  }
}

Iterable<String> comFinally() sync* {
  try {
    yield 'a';
    yield 'b';
    yield 'c';
  } finally {
    print('finally');
  }
}

Iterable<int> composto() sync* {
  yield 0;
  yield* [1, 2];
  yield* simples();
  yield* Iterable<int>.empty();
  yield 9;
}

Iterable<int> fib() sync* {
  var a = 0, b = 1;
  while (true) {
    yield a;
    final t = a + b;
    a = b;
    b = t;
  }
}

void main() {
  final it = simples();
  print('iterable criado, nada rodou');
  for (final v in it) {
    print('for-in $v');
  }
  print('--');
  print('toList: ${it.toList()}');
  print('--');
  print('first: ${it.first}');
  print('--');
  print('length: ${it.length}');
  print('--');
  print('naturais take 5: ${naturais().take(5).toList()}');
  print('naturais skip 3 take 2: ${naturais().skip(3).take(2).toList()}');
  print('fib take 10: ${fib().take(10).toList()}');
  print('fib firstWhere > 50: ${fib().firstWhere((x) => x > 50)}');
  print('where pares take 4: ${naturais().where((x) => x.isEven).take(4).toList()}');
  print('map: ${naturais().map((x) => x * x).take(4).toList()}');
  print('--');
  final arvore = No(1, [
    No(2, [No(4), No(5)]),
    No(3, [No(6, [No(7)])]),
  ]);
  print('árvore: ${percorre(arvore).toList()}');
  print('árvore soma: ${percorre(arvore).fold<int>(0, (a, b) => a + b)}');
  print('--');
  print('composto: ${composto().toList()}');
  print('--');
  for (final v in comFinally()) {
    print('completo $v');
  }
  print('--');
  for (final v in comFinally()) {
    print('parcial $v');
    if (v == 'a') break;
  }
  print('depois do break');
  print('--');
  print('take(2): ${comFinally().take(2).toList()}');
  print('--');
  final iterador = simples().iterator;
  print('iterator criado');
  print('moveNext: ${iterador.moveNext()} current: ${iterador.current}');
  print('moveNext: ${iterador.moveNext()} current: ${iterador.current}');
  print('moveNext: ${iterador.moveNext()}');
  print('moveNext de novo: ${iterador.moveNext()}');
  print('contains 1: ${simples().contains(1)}');
  print('fim');
}
