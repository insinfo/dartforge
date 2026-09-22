// Completer: complete/completeError, isCompleted, then/catchError/whenComplete, completar após await, completar duas vezes (StateError), Completer.sync.
import 'dart:async';

Future<void> basico() async {
  final c = Completer<int>();
  print('isCompleted antes: ${c.isCompleted}');
  c.future.then((v) => print('then recebeu $v'));
  c.complete(10);
  print('isCompleted depois: ${c.isCompleted}');
  print('await: ${await c.future}');
  print('await de novo: ${await c.future}');
}

Future<void> erro() async {
  final c = Completer<String>();
  c.future
      .then((v) => print('NUNCA'))
      .catchError((e) => print('catchError: $e'))
      .whenComplete(() => print('whenComplete'));
  c.completeError(StateError('falhou'));
  print('isCompleted com erro: ${c.isCompleted}');
  try {
    await c.future;
  } catch (e) {
    print('await capturou: $e');
  }
  final c2 = Completer<int>();
  c2.completeError(ArgumentError('x'), StackTrace.empty);
  try {
    await c2.future;
  } on ArgumentError catch (e) {
    print('on ArgumentError: ${e.message}');
  }
}

Future<void> completarDepois() async {
  final c = Completer<String>();
  Future<void> produtor() async {
    print('produtor: antes do await');
    await Future.delayed(Duration(milliseconds: 20));
    print('produtor: completando');
    c.complete('pronto');
  }

  produtor();
  print('consumidor: esperando');
  final r = await c.future;
  print('consumidor: $r');
}

void duasVezes() {
  final c = Completer<int>();
  c.complete(1);
  try {
    c.complete(2);
    print('NUNCA');
  } catch (e) {
    print('completar duas vezes: ${e is StateError}');
  }
  try {
    c.completeError(Exception('x'));
  } catch (e) {
    print('completeError depois: ${e is StateError}');
  }
  print('isCompleted: ${c.isCompleted}');
}

Future<void> sincrono() async {
  final normal = Completer<int>();
  final sync = Completer<int>.sync();
  normal.future.then((v) => print('normal then $v'));
  sync.future.then((v) => print('sync then $v'));
  print('antes de completar');
  normal.complete(1);
  print('normal completado (then ainda não rodou)');
  sync.complete(2);
  print('sync completado (then já rodou)');
  await null;
  print('depois de await null');
}

Future<void> completarComFuture() async {
  final c = Completer<int>();
  c.complete(Future.delayed(Duration(milliseconds: 10), () => 99));
  print('isCompleted com future: ${c.isCompleted}');
  print('valor: ${await c.future}');
  final c2 = Completer<int>();
  c2.complete(Future<int>.error(FormatException('ruim')));
  try {
    await c2.future;
  } on FormatException catch (e) {
    print('erro via future: ${e.message}');
  }
  final cv = Completer<void>();
  cv.complete();
  await cv.future;
  print('void completado: ${cv.isCompleted}');
}

Future<void> main() async {
  await basico();
  await erro();
  await completarDepois();
  duasVezes();
  await sincrono();
  await completarComFuture();
  print('fim');
}
