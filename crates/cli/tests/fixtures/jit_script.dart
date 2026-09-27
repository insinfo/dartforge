// J06: o `Platform.script` do JIT é o `.dart` (como no `dart run`), o
// `Isolate.spawnUri` resolve contra ele e é recusado (um programa por
// processo), e o `Isolate.spawn` do próprio programa funciona.
import 'dart:io';
import 'dart:isolate';

void filho(SendPort p) => p.send('do filho');

Future<void> main() async {
  print(Platform.script.pathSegments.last);
  try {
    await Isolate.spawnUri(Uri.file('outro.dart'), [], null);
    print('iniciado');
  } on IsolateSpawnException catch (e) {
    final m = e.message;
    print(m.contains('${Platform.script.resolve('outro.dart')}') && m.contains('DartForge JIT'));
  }
  final r = ReceivePort();
  await Isolate.spawn(filho, r.sendPort);
  print(await r.first);
}
