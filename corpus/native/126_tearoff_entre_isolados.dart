// Tear-off de função de topo e de método estático mandado a outro isolado
// numa mensagem: no destino ele tem o mesmo tipo de função (o `as` do
// `StreamIsolate` do new_sali/backend) e é chamável pelas duas convenções.
// O destino recriava o tear-off canônico sem o tipo (`runtimeType`
// `Function`) nem o corpo tipado (docs/NATIVO-PROJETOS-REAIS.md, C11).

import 'dart:async';
import 'dart:isolate';

class R {
  static Stream entrada(Stream de, dynamic arg) => Stream.value('ok $arg');
}

Stream topo(Stream de, dynamic arg) => Stream.value('topo $arg');

void filho(List args) {
  final f = args[0] as Stream Function(Stream, dynamic);
  final g = args[1] as Stream<Object?> Function(Stream<Object?>, dynamic);
  final s = args[2] as SendPort;
  print('${f.runtimeType} ${g.runtimeType}');
  f(const Stream.empty(), 1).first.then((v) => g(const Stream.empty(), 2).first.then((w) => s.send('$v $w')));
}

void main() async {
  print('${R.entrada.runtimeType}');
  final p = ReceivePort();
  await Isolate.spawn(filho, [R.entrada, topo, p.sendPort]);
  print(await p.first);
}
