// Ordem de microtarefas: código síncrono, scheduleMicrotask, Future.microtask, Future() (timer), then, await null, microtarefas aninhadas.
import 'dart:async';

Future<void> parte1() async {
  print('p1: 1 síncrono');
  scheduleMicrotask(() => print('p1: 4 scheduleMicrotask'));
  Future.microtask(() => print('p1: 5 Future.microtask'));
  Future(() => print('p1: 9 Future() timer'));
  Future.value(6).then((v) => print('p1: $v Future.value.then'));
  print('p1: 2 síncrono');
  scheduleMicrotask(() {
    print('p1: 7 microtarefa externa');
    scheduleMicrotask(() => print('p1: 8 aninhada antes do timer'));
  });
  print('p1: 3 síncrono');
  await null;
  print('p1: depois de await null');
  await Future(() => null);
  print('p1: depois de await timer');
}

Future<void> parte2() async {
  print('p2: a');
  final f = Future(() {
    print('p2: f timer');
    return 1;
  });
  scheduleMicrotask(() => print('p2: b micro'));
  final r = await f;
  print('p2: c await timer $r');
  scheduleMicrotask(() => print('p2: d micro agendada antes do await null'));
  await null;
  print('p2: e depois de d');
}

Future<void> parte3() async {
  print('p3: início');
  scheduleMicrotask(() {
    print('p3: m1');
    scheduleMicrotask(() {
      print('p3: m1.1');
      scheduleMicrotask(() => print('p3: m1.1.1'));
    });
    scheduleMicrotask(() => print('p3: m1.2'));
  });
  scheduleMicrotask(() => print('p3: m2'));
  Future(() => print('p3: t1'));
  Future(() {
    print('p3: t2');
    scheduleMicrotask(() => print('p3: t2.micro'));
  });
  Future(() => print('p3: t3'));
  await null;
  print('p3: depois de await null');
  await Future(() => null);
  print('p3: depois de await Future()');
}

Future<void> main() async {
  print('main início');
  await parte1();
  print('--');
  await parte2();
  print('--');
  await parte3();
  print('--');
  var log = <String>[];
  for (var i = 0; i < 3; i++) {
    scheduleMicrotask(() => log.add('m$i'));
    Future(() => log.add('t$i'));
  }
  log.add('s');
  await Future(() => null);
  await Future(() => null);
  print(log.join(','));
  print('main fim');
}
