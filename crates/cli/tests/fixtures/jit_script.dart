// J06: the JIT's `Platform.script` is the `.dart` (as in `dart run`),
// `Isolate.spawnUri` resolves against it and is rejected (one program per
// process), and `Isolate.spawn` of the program itself works.
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
