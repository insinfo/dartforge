// Future.delayed e Timer: ordem por duração, duração igual (ordem de registro), Duration.zero vs microtask, valor, aninhado, Timer.periodic com cancel.
import 'dart:async';

Future<void> duracoesDistintas() async {
  print('distintas: início');
  final f3 = Future.delayed(Duration(milliseconds: 60), () => print('60 ms'));
  final f1 = Future.delayed(Duration(milliseconds: 20), () => print('20 ms'));
  final f2 = Future.delayed(Duration(milliseconds: 40), () => print('40 ms'));
  await Future.wait([f1, f2, f3]);
  print('distintas: fim');
}

Future<void> duracoesIguais() async {
  print('iguais: início');
  final fs = <Future<void>>[];
  for (var i = 1; i <= 4; i++) {
    fs.add(Future.delayed(Duration(milliseconds: 10), () => print('igual $i')));
  }
  await Future.wait(fs);
  print('iguais: fim');
}

Future<void> zeroVsMicro() async {
  print('zero: síncrono');
  Future.delayed(Duration.zero, () => print('zero: delayed zero'));
  scheduleMicrotask(() => print('zero: microtask'));
  Future(() => print('zero: Future()'));
  Future.delayed(Duration(milliseconds: 20), () => print('zero: 20 ms'));
  await Future.delayed(Duration(milliseconds: 40));
  print('zero: fim');
}

Future<void> comValor() async {
  final v = await Future.delayed(Duration(milliseconds: 10), () => 42);
  print('valor $v');
  final s = await Future<String>.delayed(Duration(milliseconds: 10), () => 'texto');
  print('valor $s');
  final n = await Future.delayed(Duration(milliseconds: 10));
  print('sem callback: $n');
  final aninhado = await Future.delayed(
      Duration(milliseconds: 10),
      () => Future.delayed(Duration(milliseconds: 10), () => 'interno'));
  print('aninhado $aninhado');
}

Future<void> timers() async {
  print('timers: início');
  final c = Completer<void>();
  var ticks = 0;
  late Timer periodico;
  Timer(Duration(milliseconds: 15), () => print('timer único 15 ms'));
  periodico = Timer.periodic(Duration(milliseconds: 30), (t) {
    ticks++;
    print('tick $ticks ativo=${t.isActive} identico=${identical(t, periodico)}');
    if (ticks == 3) {
      t.cancel();
      print('cancelado ativo=${t.isActive}');
      c.complete();
    }
  });
  print('periodico ativo antes: ${periodico.isActive}');
  final cancelado = Timer(Duration(milliseconds: 20), () => print('NUNCA'));
  cancelado.cancel();
  print('cancelado.isActive=${cancelado.isActive}');
  await c.future;
  await Future.delayed(Duration(milliseconds: 40));
  print('timers: fim ticks=$ticks');
}

Future<void> main() async {
  await duracoesDistintas();
  await duracoesIguais();
  await zeroVsMicro();
  await comValor();
  await timers();
  final t = Timer(Duration(milliseconds: 10), () {});
  print('Timer é Timer: ${t is Timer}');
  await Future.delayed(Duration(milliseconds: 30));
  print('depois ativo=${t.isActive}');
  print('fim');
}
