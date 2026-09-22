// Convertido de tests/conformance/modules/fluxo26 (módulo antigo do corpus de conformidade).
// Controle de fluxo do incremento 26: try/on/catch/finally, throw, rethrow,
// assert, for-in, rótulos e o operador condicional.
//
// A saída de referência em main.stdout foi produzida por
// `dart run --enable-asserts main.dart` com o Dart SDK 3.6.2 e conferida
// também com o Dart SDK 3.13.4.

class Falha {
  final String motivo;
  Falha(this.motivo);
}

int retornoComFinally() {
  var x = 1;
  try {
    x = 2;
    return x;
  } finally {
    x = 3;
    print('finally $x');
  }
}

void breakComFinally() {
  for (var i = 0; i < 3; i++) {
    try {
      if (i == 1) {
        break;
      }
      print('corpo $i');
    } finally {
      print('finally $i');
    }
  }
}

void continueComFinally() {
  var i = 0;
  while (i < 3) {
    i = i + 1;
    try {
      if (i == 2) {
        continue;
      }
      print('volta $i');
    } finally {
      print('sai $i');
    }
  }
}

void capturaPorTipo() {
  try {
    throw 'texto';
  } on String catch (e) {
    print('capturado $e');
  }
}

void naoCapturaEPropaga() {
  try {
    try {
      throw Falha('a');
    } on String catch (e) {
      print('nunca $e');
    }
  } on Falha catch (e) {
    print('externo ${e.motivo}');
  }
}

void comRethrow() {
  try {
    try {
      throw 'raiz';
    } on String catch (e) {
      print('interno $e');
      rethrow;
    }
  } on String catch (e) {
    print('externo $e');
  }
}

void ordemComExcecao() {
  try {
    try {
      print('antes');
      throw 'x';
    } finally {
      print('finally roda');
    }
  } on String catch (e) {
    print('depois $e');
  }
}

void onSemBinding() {
  try {
    throw Falha('b');
  } on Falha {
    print('sem binding');
  }
}

void catchSemOnComPromocao() {
  try {
    throw 7;
  } catch (e) {
    if (e is int) {
      print('qualquer ${e + 1}');
    }
  }
}

List<int> fonte() {
  print('avaliou');
  return <int>[1, 2, 3];
}

int? nulo() => null;

void main() {
  print(retornoComFinally());
  breakComFinally();
  continueComFinally();
  capturaPorTipo();
  naoCapturaEPropaga();
  comRethrow();
  ordemComExcecao();
  onSemBinding();
  catchSemOnComPromocao();
  for (final x in fonte()) {
    print(x);
  }
  for (var s in <String>['a', 'b']) {
    print(s);
  }
  externo:
  for (var i = 0; i < 3; i++) {
    for (var j = 0; j < 3; j++) {
      if (j == 1) {
        continue externo;
      }
      if (i == 2) {
        break externo;
      }
      print('$i-$j');
    }
  }
  var k = 0;
  laco:
  while (k < 4) {
    k = k + 1;
    if (k == 2) {
      continue laco;
    }
    if (k == 3) {
      break laco;
    }
    print('while $k');
  }
  var m = 0;
  repete:
  do {
    m = m + 1;
    if (m == 2) {
      continue repete;
    }
    if (m == 3) {
      break repete;
    }
    print('do $m');
  } while (m < 5);
  print(1 > 2 ? 'sim' : 'nao');
  final int? v = nulo();
  print(v == null ? 'vazio' : 'valor ${v + 1}');
  print(nulo() ?? (1 > 0 ? 10 : 20));
  print(true || false ? 'x' : 'y');
  print(true ? 1 : true ? 2 : 3);
  Object o = 1;
  print(o is int ? 'inteiro' : 'outro');
  var t = 5;
  assert(t == 5);
  assert(t == 5, 'deveria ser cinco');
  print('asserções passaram');
  try {
    final int? w = nulo();
    print(w ?? (throw 'faltou'));
  } on String catch (e) {
    print('lancou $e');
  }
  try {
    assert(t == 6, 'falhou de propósito');
    print('assert não disparou');
  } catch (e) {
    print('assert disparou');
  }
}
