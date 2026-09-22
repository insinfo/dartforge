// Future.wait (ordem), wait com erro, Future.any, then encadeado, catchError com test, whenComplete, Future.error/value, timeout, Future.forEach, Future.doWhile.
import 'dart:async';

Future<int> lento(int v, int ms) =>
    Future.delayed(Duration(milliseconds: ms), () => v);

Future<void> wait() async {
  final rs = await Future.wait([lento(1, 45), lento(2, 10), lento(3, 30)]);
  print('wait mantém ordem: $rs');
  final vazio = await Future.wait(<Future<int>>[]);
  print('wait vazio: $vazio');
  final ordem = <int>[];
  await Future.wait([
    lento(1, 45).then((v) => ordem.add(v)),
    lento(2, 10).then((v) => ordem.add(v)),
    lento(3, 30).then((v) => ordem.add(v)),
  ]);
  print('ordem de conclusão: $ordem');
  try {
    await Future.wait([
      lento(1, 10),
      Future<int>.delayed(Duration(milliseconds: 25), () => throw StateError('segundo')),
      lento(3, 45),
    ]);
  } catch (e) {
    print('wait com erro: $e');
  }
  final limpo = <int>[];
  try {
    await Future.wait([
      lento(1, 10),
      Future<int>.error(ArgumentError('cedo')),
    ], cleanUp: (v) => limpo.add(v));
  } catch (e) {
    print('wait cleanUp: ${e is ArgumentError}');
  }
  await Future.delayed(Duration(milliseconds: 30));
  print('limpos: $limpo');
  final eager = await Future.wait([lento(7, 10), lento(8, 20)], eagerError: true);
  print('eagerError sem erro: $eager');
}

Future<void> any() async {
  final r = await Future.any([lento(1, 45), lento(2, 10), lento(3, 30)]);
  print('any: $r');
  try {
    await Future.any([
      lento(1, 45),
      Future<int>.delayed(Duration(milliseconds: 10), () => throw Exception('primeiro falhou')),
    ]);
  } catch (e) {
    print('any com erro: $e');
  }
}

Future<void> encadeado() async {
  final r = await Future.value(2)
      .then((v) => v * 3)
      .then((v) => 'texto $v')
      .then((s) => s.length);
  print('then encadeado: $r');
  final r2 = await Future.value(1).then((v) => lento(v + 10, 10));
  print('then retornando Future: $r2');
  final r3 = await Future<int>.error(StateError('a'))
      .catchError((e) => -1)
      .then((v) => v * 2);
  print('catchError recupera: $r3');
  final r4 = await Future<int>.error(FormatException('b'))
      .catchError((e) => 100, test: (e) => e is FormatException);
  print('catchError com test verdadeiro: $r4');
  try {
    await Future<int>.error(FormatException('c'))
        .catchError((e) => 100, test: (e) => e is StateError);
  } catch (e) {
    print('catchError com test falso repassa: ${e is FormatException}');
  }
  final r5 = await Future.value(5)
      .then<String>((v) => throw StateError('em then'))
      .catchError((e) => 'recuperado $e');
  print('erro lançado em then: $r5');
  final log = <String>[];
  await Future.value(1)
      .whenComplete(() => log.add('wc1'))
      .then((v) => log.add('then $v'))
      .whenComplete(() => log.add('wc2'));
  print('whenComplete: $log');
  try {
    await Future<int>.error(Exception('x')).whenComplete(() => log.add('wc erro'));
  } catch (e) {
    log.add('capturado');
  }
  print('whenComplete com erro: $log');
  final r6 = await Future.value(1).then((v) => v, onError: (e) => -1);
  print('then onError sem erro: $r6');
  final r7 = await Future<int>.error(Exception('y')).then((v) => v, onError: (e) => -1);
  print('then onError com erro: $r7');
  print('Future.sync: ${await Future.sync(() => 3)}');
  print('Future.value null: ${await Future<int?>.value()}');
}

Future<void> timeout() async {
  try {
    await lento(1, 60).timeout(Duration(milliseconds: 15));
  } on TimeoutException {
    print('timeout lançou TimeoutException');
  }
  final r = await lento(1, 60).timeout(Duration(milliseconds: 15), onTimeout: () => -1);
  print('timeout onTimeout: $r');
  final r2 = await lento(2, 10).timeout(Duration(milliseconds: 60));
  print('sem timeout: $r2');
}

Future<void> lacos() async {
  final vistos = <String>[];
  await Future.forEach<int>([1, 2, 3], (v) async {
    await Future.delayed(Duration(milliseconds: 5));
    vistos.add('f$v');
  });
  print('forEach: $vistos');
  var n = 0;
  await Future.doWhile(() async {
    n++;
    await Future.delayed(Duration(milliseconds: 5));
    return n < 4;
  });
  print('doWhile: $n');
  await Future.doWhile(() => false);
  print('doWhile imediato ok');
}

Future<void> main() async {
  await wait();
  await any();
  await encadeado();
  await timeout();
  await lacos();
  print('fim');
}
