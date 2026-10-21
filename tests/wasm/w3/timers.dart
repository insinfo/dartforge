// W3: the event loop of a module returns to the host while timers wait
// (`dartforge_resume`, docs/WASM-HOST-ABI-V1.md), in the browser too. The
// deadlines are far apart: the order does not depend on the host's timer slack.
import 'dart:async';

Future<void> main() async {
  final order = <String>[];
  Timer(const Duration(milliseconds: 200), () => order.add('timer 200'));
  scheduleMicrotask(() => order.add('microtask'));
  await Future.delayed(const Duration(milliseconds: 10), () => order.add('delayed 10'));
  var ticks = 0;
  await for (final _ in Stream.periodic(const Duration(milliseconds: 5)).take(3)) {
    ticks++;
  }
  order.add('periodic $ticks');
  await Future.delayed(const Duration(milliseconds: 250));
  final watch = Stopwatch()..start();
  await Future.delayed(const Duration(milliseconds: 20));
  print(order.join(', '));
  print('waited at least 20 ms: ${watch.elapsedMilliseconds >= 20}');
}
