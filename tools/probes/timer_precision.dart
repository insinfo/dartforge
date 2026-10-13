// Probe: the real time of short timers (Future.delayed) in the native event
// loop, against the VM. Run outside Windows through the debug.yml workflow.
Future<void> main() async {
  final sw = Stopwatch()..start();
  for (var i = 0; i < 200; i++) {
    await Future.delayed(const Duration(milliseconds: 5));
  }
  print("200x5ms: ${sw.elapsedMilliseconds} ms");
  sw.reset();
  for (var i = 0; i < 50; i++) {
    await Future.delayed(const Duration(milliseconds: 1));
  }
  print("50x1ms: ${sw.elapsedMilliseconds} ms");
}
