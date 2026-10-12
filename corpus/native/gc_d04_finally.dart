// D4 (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3): `finally` e `catch`
// que alocam e relançam, e `break` atravessando `finally`.

class No {
  final int v;
  No(this.v);
}

int conta = 0;

int f(int n) {
  try {
    if (n % 3 == 0) throw StateError('x$n');
    return n;
  } finally {
    final lixo = <No>[];
    for (var i = 0; i < 50; i++) {
      lixo.add(No(i));
    }
    conta += lixo.length;
  }
}

int g(int n) {
  try {
    return f(n);
  } on StateError catch (e) {
    final t = '${e.message}!';
    try {
      throw StateError(t);
    } finally {
      conta += t.length;
    }
  }
}

void main() {
  var soma = 0;
  var erros = 0;
  for (var i = 0; i < 30; i++) {
    try {
      soma += g(i);
    } on StateError catch (e) {
      erros++;
      soma += e.message.length;
    }
  }
  for (var i = 0; i < 10; i++) {
    var k = 0;
    while (true) {
      try {
        k++;
        if (k == 3) break;
      } finally {
        conta++;
      }
    }
  }
  print('$soma $erros $conta');
}
