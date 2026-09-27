// J01: controle de um isolado preso num laço síncrono sem eventos (o ponto
// seguro de cada volta de laço, como a verificação de pilha da VM): o `ping`
// imediato responde, o `kill` imediato termina o isolado (rodando os
// `finally` do caminho, que um `catch` não intercepta) e os outros isolados
// seguem; um laço `for-in`, um `do-while` e um laço num gerador também param.
import 'dart:async';
import 'dart:isolate';

void giroWhile(SendPort p) {
  p.send('while');
  var x = 0;
  try {
    try {
      while (true) {
        x = (x * 31 + 7) & 0xffff;
      }
    } catch (e) {
      p.send('catch não deveria ver o kill');
    }
  } finally {
    p.send('finally do while');
  }
}

void giroDo(SendPort p) {
  p.send('do');
  var x = 1;
  do {
    x = (x * 17) % 1000003;
  } while (x != 0);
}

Iterable<int> infinito() sync* {
  var i = 0;
  while (true) {
    yield i++;
  }
}

void giroForIn(SendPort p) {
  p.send('for-in');
  var s = 0;
  for (final i in infinito()) {
    s = (s + i) & 0xff;
  }
}

Future<void> controlar(void Function(SendPort) entrada) async {
  final mensagens = ReceivePort();
  final saiu = ReceivePort();
  final iso = await Isolate.spawn(entrada, mensagens.sendPort, onExit: saiu.sendPort);
  final fila = StreamIterator(mensagens);
  await fila.moveNext();
  print('começou: ${fila.current}');
  // O ping imediato chega com o isolado ocupado.
  final pong = ReceivePort();
  iso.ping(pong.sendPort, response: 'pong', priority: Isolate.immediate);
  print('ping: ${await pong.first}');
  iso.kill(priority: Isolate.immediate);
  final r = await saiu.first.timeout(const Duration(seconds: 10), onTimeout: () => 'não saiu');
  print('saiu: ${r == null}');
  // O que o isolado mandou antes de morrer (o `finally`).
  while (await fila.moveNext().timeout(const Duration(milliseconds: 200), onTimeout: () => false)) {
    print('  ${fila.current}');
  }
  mensagens.close();
}

Future<void> main() async {
  await controlar(giroWhile);
  await controlar(giroDo);
  await controlar(giroForIn);
  // O principal seguiu rodando timers o tempo todo.
  final t = Stopwatch()..start();
  await Future.delayed(const Duration(milliseconds: 10));
  print('principal vivo: ${t.elapsedMilliseconds >= 10}');
}
