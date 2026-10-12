// dart:isolate: Isolate.spawn/run em threads, constantes e enums idênticos entre isolados, tipos genéricos e Type
// nas mensagens, onError/onExit, Isolate.exit, kill, ping e erros do Isolate.run.
import 'dart:async';
import 'dart:isolate';

enum Cor { verde, azul }

class Pt {
  final int x;
  const Pt(this.x);
}

const pt = Pt(3);

int fib(int n) => n < 2 ? n : fib(n - 1) + fib(n - 2);

class Resultado {
  final String nome;
  final List<int> valores;
  Resultado(this.nome, this.valores);
  @override
  String toString() => 'Resultado($nome, $valores)';
}

void filho(SendPort p) {
  p.send([Cor.azul, pt, 'oi', 42, 1.5, [1, 2], {'a': 1}]);
}

void quebra(int _) {
  throw StateError('falhou no filho');
}

void eterno(SendPort p) {
  p.send('comecei');
  Timer.periodic(const Duration(milliseconds: 5), (_) {});
}

Future<void> main() async {
  final r = ReceivePort();
  await Isolate.spawn(filho, r.sendPort);
  final m = await r.first as List;
  print(m);
  print(identical(m[0], Cor.azul));
  print(identical(m[1], pt));
  switch (m[0] as Cor) {
    case Cor.azul:
      print('switch azul');
    case Cor.verde:
      print('verde');
  }

  final erros = ReceivePort();
  final saida = ReceivePort();
  await Isolate.spawn(quebra, 0, onError: erros.sendPort, onExit: saida.sendPort);
  print((await erros.first as List)[0]);
  print(await saida.first);

  final r2 = ReceivePort();
  await Isolate.spawn((SendPort p) => Isolate.exit(p, 'saiu'), r2.sendPort);
  print(await r2.first);

  final base = 20;
  print('run: ${await Isolate.run(() => fib(base) + 1)}');
  final fs = [for (var i = 20; i < 25; i++) Isolate.run(() => fib(i))];
  print('paralelo: ${await Future.wait(fs)}');
  print(await Isolate.run(() => Resultado('x', [for (var i = 0; i < 5; i++) i * i])));
  try {
    await Isolate.run(() => throw ArgumentError('ruim'));
  } catch (e) {
    print('erro: ${e.runtimeType} $e');
  }

  final pronto = ReceivePort();
  final saiu = ReceivePort();
  final iso = await Isolate.spawn(eterno, pronto.sendPort, onExit: saiu.sendPort);
  print(await pronto.first);
  iso.kill(priority: Isolate.immediate);
  print('saiu: ${await saiu.first}');

  final pingado = ReceivePort();
  final pronto2 = ReceivePort();
  final iso2 = await Isolate.spawn(eterno, pronto2.sendPort);
  await pronto2.first;
  iso2.ping(pingado.sendPort, response: 'pong');
  print(await pingado.first);
  iso2.kill();

  print('debugName: ${Isolate.current.debugName}');
  final tipado = await Isolate.run(() => <String, List<int>>{'a': [1]});
  print(tipado is Map<String, List<int>>);
  print(tipado.runtimeType);
  print(identical(await Isolate.run(() => int), int));
}
