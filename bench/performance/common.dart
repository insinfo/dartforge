// Shared measurement of the `bench/performance` programs: runs the kernel `n`
// times (the first one warms up the JIT) and prints the time of each round and
// the result, which must be the same on every executor.
void measure(String name, Object? Function() kernel, {int rounds = 6}) {
  Object? r;
  final times = <int>[];
  for (var i = 0; i < rounds; i++) {
    final sw = Stopwatch()..start();
    r = kernel();
    times.add(sw.elapsedMicroseconds);
  }
  print('$name: ${times.join(' ')} us | $r');
}
