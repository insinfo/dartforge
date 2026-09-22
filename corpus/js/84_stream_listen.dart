// Streams: fromIterable, listen (onData/onDone/onError), StreamController, cancelOnError, pause/resume/cancel, periodic+take, map/where/take/skip/expand/asyncMap, toList/first/length/fold/join.
import 'dart:async';

Future<void> fromIterable() async {
  final c = Completer<void>();
  Stream.fromIterable([1, 2, 3]).listen(
    (v) => print('dado $v'),
    onDone: () {
      print('done');
      c.complete();
    },
  );
  print('listen registrado (dados chegam depois)');
  await c.future;
}

Future<void> controller() async {
  final ctrl = StreamController<String>();
  final done = Completer<void>();
  ctrl.stream.listen(
    (v) => print('recebido $v'),
    onError: (e) => print('erro $e'),
    onDone: () {
      print('controller done');
      done.complete();
    },
  );
  ctrl.add('a');
  ctrl.addError(StateError('falha'));
  ctrl.add('b');
  print('isClosed antes: ${ctrl.isClosed}');
  await ctrl.close();
  print('isClosed depois: ${ctrl.isClosed}');
  await done.future;
}

Future<void> cancelOnError() async {
  final ctrl = StreamController<int>();
  final done = Completer<void>();
  ctrl.stream.listen(
    (v) => print('coe dado $v'),
    onError: (e) => print('coe erro $e'),
    onDone: () => print('coe done (NUNCA com cancelOnError)'),
    cancelOnError: true,
  );
  ctrl.onCancel = () {
    print('coe cancelado pelo erro');
    done.complete();
  };
  ctrl.add(1);
  ctrl.addError('boom');
  ctrl.add(2);
  await done.future;
  await ctrl.close();
}

Future<void> pausaResume() async {
  final ctrl = StreamController<int>();
  final done = Completer<void>();
  late StreamSubscription<int> sub;
  sub = ctrl.stream.listen((v) {
    print('pr dado $v');
    if (v == 2) {
      sub.pause();
      print('pausado: ${sub.isPaused}');
      Future.delayed(Duration(milliseconds: 20), () {
        print('retomando');
        sub.resume();
      });
    }
    if (v == 4) {
      sub.cancel();
      print('cancelado');
      done.complete();
    }
  }, onDone: () => print('pr done (NUNCA, cancelado antes)'));
  for (var i = 1; i <= 6; i++) {
    ctrl.add(i);
  }
  print('todos adicionados; isPaused=${sub.isPaused}');
  await done.future;
  await ctrl.close();
}

Future<void> periodico() async {
  final xs = await Stream.periodic(Duration(milliseconds: 10), (i) => i * i)
      .take(4)
      .toList();
  print('periodic take: $xs');
}

Future<void> transformacoes() async {
  Stream<int> s() => Stream.fromIterable([1, 2, 3, 4, 5, 6]);
  print('map: ${await s().map((x) => x * 10).toList()}');
  print('where: ${await s().where((x) => x.isEven).toList()}');
  print('take: ${await s().take(2).toList()}');
  print('skip: ${await s().skip(4).toList()}');
  print('expand: ${await s().take(3).expand((x) => [x, -x]).toList()}');
  print('asyncMap: ${await s().take(3).asyncMap((x) async {
    await Future.delayed(Duration(milliseconds: 5));
    return 'v$x';
  }).toList()}');
  print('first: ${await s().first}');
  print('last: ${await s().last}');
  print('length: ${await s().length}');
  print('fold: ${await s().fold<int>(0, (a, b) => a + b)}');
  print('join: ${await s().join('-')}');
  print('any: ${await s().any((x) => x > 5)}');
  print('every: ${await s().every((x) => x > 5)}');
  print('contains: ${await s().contains(3)}');
  print('takeWhile: ${await s().takeWhile((x) => x < 4).toList()}');
  print('skipWhile: ${await s().skipWhile((x) => x < 4).toList()}');
  print('isEmpty: ${await Stream<int>.empty().isEmpty}');
  print('elementAt: ${await s().elementAt(2)}');
  print('reduce: ${await s().reduce((a, b) => a * b)}');
  print('firstWhere: ${await s().firstWhere((x) => x > 3)}');
  print('distinct: ${await Stream.fromIterable([1, 1, 2, 2, 2, 3, 1]).distinct().toList()}');
  final m = await s().toSet();
  print('toSet length: ${m.length}');
  final err = Stream<int>.error(Exception('stream erro'));
  try {
    await err.first;
  } catch (e) {
    print('first com erro: $e');
  }
  print('handleError: ${await Stream.fromIterable([1, 2]).handleError((e) {}).toList()}');
}

Future<void> main() async {
  await fromIterable();
  await controller();
  await cancelOnError();
  await pausaResume();
  await periodico();
  await transformacoes();
  print('fim');
}
