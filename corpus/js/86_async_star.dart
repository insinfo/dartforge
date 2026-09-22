// async*: yield, yield*, await dentro, try/finally no gerador (finally roda ao cancelar), recursivo, consumido por await for e por listen.
import 'dart:async';

Stream<int> simples() async* {
  print('gerador: início');
  yield 1;
  print('gerador: entre');
  yield 2;
  print('gerador: fim');
}

Stream<int> comAwait() async* {
  for (var i = 1; i <= 3; i++) {
    await Future.delayed(Duration(milliseconds: 5));
    yield i * i;
  }
}

Stream<String> comFinally(String nome) async* {
  try {
    yield '$nome-a';
    yield '$nome-b';
    yield '$nome-c';
  } finally {
    print('finally de $nome');
  }
}

Stream<int> recursivo(int n) async* {
  if (n <= 0) return;
  yield n;
  yield* recursivo(n - 1);
}

Stream<int> comYieldStar() async* {
  yield* Stream.fromIterable([10, 20]);
  yield* comAwait();
  yield 99;
}

Stream<int> vazio() async* {}

Stream<int> lancaDepois() async* {
  yield 1;
  yield 2;
  throw FormatException('gerador falhou');
}

Future<void> main() async {
  final s = simples();
  print('stream criado (nada rodou)');
  await for (final v in s) {
    print('await for $v');
  }
  print('--');
  print('comAwait: ${await comAwait().toList()}');
  print('recursivo: ${await recursivo(4).toList()}');
  print('yield*: ${await comYieldStar().toList()}');
  print('vazio: ${await vazio().toList()} isEmpty=${await vazio().isEmpty}');
  print('--');
  await for (final v in comFinally('completo')) {
    print('consome $v');
  }
  print('--');
  await for (final v in comFinally('cancelado')) {
    print('consome $v');
    if (v.endsWith('a')) break;
  }
  print('depois do break');
  print('--');
  print('take(1): ${await comFinally('take').take(1).toList()}');
  print('--');
  final done = Completer<void>();
  final itens = <int>[];
  simples().listen(itens.add, onDone: () {
    print('listen done $itens');
    done.complete();
  });
  print('listen registrado');
  await done.future;
  print('--');
  try {
    await for (final v in lancaDepois()) {
      print('antes do erro $v');
    }
  } on FormatException catch (e) {
    print('capturado: ${e.message}');
  }
  final erroDone = Completer<void>();
  lancaDepois().listen(
    (v) => print('listen dado $v'),
    onError: (e) => print('listen erro: ${e is FormatException}'),
    onDone: () {
      print('listen done depois do erro');
      erroDone.complete();
    },
  );
  await erroDone.future;
  print('fim');
}
