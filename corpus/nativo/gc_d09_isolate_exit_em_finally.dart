// D9 (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3): `Isolate.exit` dentro
// de `finally`, com a exceção que saiu de uma função Dart chamada pelo
// runtime (`Function.apply`, pela porta `dart_r3`) ainda em curso. O
// `Isolate.exit` não é capturável: manda a mensagem e encerra o isolado, e a
// exceção pendente se perde. Com as exceções por tabelas, a saída do isolado
// sobe de pouso em pouso por `@df.lancar` até a porta de `rodar_isolado`.
//
// Saída: 870

import 'dart:isolate';

class Erro implements Exception {
  final int n;
  Erro(this.n);
}

int lanca(int x) {
  // Aloca antes de lançar.
  final s = 'v$x';
  throw Erro(x + s.length);
}

void filho(List<Object> args) {
  final porta = args[0] as SendPort;
  final k = args[1] as int;
  try {
    Function.apply(lanca, [k]);
  } finally {
    // Aloca com a exceção em curso, depois sai.
    final quadrados = List<int>.generate(k + 1, (i) => i * i);
    Isolate.exit(porta, quadrados.fold<int>(0, (a, b) => a + b) + k);
  }
}

Future<void> main() async {
  var total = 0;
  for (var k = 0; k < 10; k++) {
    final r = ReceivePort();
    await Isolate.spawn(filho, <Object>[r.sendPort, k]);
    total += await r.first as int;
  }
  print(total);
}
