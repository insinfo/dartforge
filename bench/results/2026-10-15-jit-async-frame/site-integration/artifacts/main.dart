
import 'dart:async';
import 'dart:io';
Future<void> worker() async {
  final value = 37;
  final extra = DateTime.now().millisecondsSinceEpoch;
  File('/Volumes/ssd_dock/MyRustProjects/dartforge/target/jit-async-frame-reload-17855/suspended').writeAsStringSync('suspended');
  while (!File('/Volumes/ssd_dock/MyRustProjects/dartforge/target/jit-async-frame-reload-17855/release').existsSync()) {
    await Future<void>.delayed(const Duration(milliseconds: 10));
  }
  File('/Volumes/ssd_dock/MyRustProjects/dartforge/target/jit-async-frame-reload-17855/result').writeAsStringSync('new:$value:${extra > 0}\n', mode: FileMode.append);
}
Future<void> main() async {
  await worker();
  await worker();
}
