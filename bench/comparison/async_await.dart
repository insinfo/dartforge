// Async micro-benchmark (RELATORIO-PRONTIDAO-PRODUCAO.md): chains of awaited async
// calls, five rounds of one million outer awaits, each awaiting two completed futures.
Future<int> leaf(int i) async => i & 7;
Future<int> mid(int i) async {
  final a = await leaf(i);
  final b = await leaf(i + 1);
  return a + b;
}
Future<int> run(int n) async {
  var s = 0;
  for (var i = 0; i < n; i++) {
    s += await mid(i);
  }
  return s;
}
Future<void> main() async {
  final t = <int>[];
  var r = 0;
  for (var k = 0; k < 5; k++) {
    final sw = Stopwatch()..start();
    r = await run(1000000);
    t.add(sw.elapsedMilliseconds);
  }
  print('async: ${t.join(' ')} ms | $r');
}
