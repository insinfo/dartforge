// await for: sobre Stream, break (cancela), continue, aninhado, stream de async* com yield*, ordem entre produtor e consumidor.
import 'dart:async';

Stream<int> contagem(int n, String nome) async* {
  for (var i = 1; i <= n; i++) {
    print('$nome produz $i');
    yield i;
    print('$nome continuou depois de $i');
  }
  print('$nome terminou');
}

Stream<int> composto() async* {
  yield 0;
  yield* contagem(2, 'interno');
  yield 100;
}

Future<void> basico() async {
  var soma = 0;
  await for (final v in contagem(3, 'basico')) {
    print('consome $v');
    soma += v;
  }
  print('soma $soma');
}

Future<void> comBreak() async {
  await for (final v in contagem(5, 'break')) {
    print('consome $v');
    if (v == 2) {
      print('break');
      break;
    }
  }
  print('depois do break');
}

Future<void> comContinue() async {
  final pares = <int>[];
  await for (final v in Stream.fromIterable([1, 2, 3, 4, 5, 6])) {
    if (v.isOdd) continue;
    pares.add(v);
  }
  print('pares $pares');
}

Future<void> aninhado() async {
  await for (final a in Stream.fromIterable([1, 2])) {
    await for (final b in Stream.fromIterable(['x', 'y'])) {
      print('$a$b');
    }
  }
}

Future<void> viaYieldStar() async {
  final vistos = <int>[];
  await for (final v in composto()) {
    vistos.add(v);
  }
  print('composto $vistos');
}

Future<void> comController() async {
  final ctrl = StreamController<String>();
  Future<void>(() async {
    for (final s in ['a', 'b', 'c']) {
      print('add $s');
      ctrl.add(s);
      await Future.delayed(Duration(milliseconds: 5));
    }
    print('fechando');
    await ctrl.close();
  });
  await for (final s in ctrl.stream) {
    print('recebido $s');
  }
  print('await for terminou');
}

Future<void> comErro() async {
  Stream<int> falha() async* {
    yield 1;
    throw StateError('no meio');
  }

  try {
    await for (final v in falha()) {
      print('antes do erro $v');
    }
    print('NUNCA');
  } catch (e) {
    print('capturado: $e');
  }
}

Future<int> retornoDentro() async {
  await for (final v in contagem(4, 'ret')) {
    if (v == 3) return v * 10;
  }
  return -1;
}

Future<void> main() async {
  await basico();
  print('--');
  await comBreak();
  print('--');
  await comContinue();
  await aninhado();
  await viaYieldStar();
  print('--');
  await comController();
  print('--');
  await comErro();
  print('--');
  print('retorno ${await retornoDentro()}');
  print('fim');
}
