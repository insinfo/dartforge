// Sonda: o tempo real de timers curtos (Future.delayed) no laço de eventos
// nativo, contra a VM. Rodar fora do Windows pelo workflow depurar.yml.
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
