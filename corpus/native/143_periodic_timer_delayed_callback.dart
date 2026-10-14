// Slow callbacks preserve timer phase, microtask order and zero-period ticks.
import 'dart:async';
import 'dart:io';

Future<void> main() async {
  var callbacks = 0;
  var firstTick = 0;
  var microtaskRan = false;
  final done = Completer<void>();
  Timer.periodic(const Duration(milliseconds: 10), (timer) {
    callbacks++;
    if (callbacks == 1) {
      firstTick = timer.tick;
      sleep(const Duration(milliseconds: 200));
      if (timer.tick != firstTick)
        throw StateError('Tick changed inside callback');
      scheduleMicrotask(() {
        microtaskRan = true;
      });
    } else {
      if (!microtaskRan || timer.tick <= firstTick + 1)
        throw StateError('Missed ticks or microtask ordering failed');
      timer.cancel();
      done.complete();
    }
  });
  await done.future;
  print('delayed-callback: callbacks=$callbacks skipped=true microtask=true');
  final zeroDone = Completer<void>();
  var zeroCalls = 0;
  Timer.periodic(Duration.zero, (timer) {
    zeroCalls++;
    if (timer.tick != zeroCalls) throw StateError('Zero period skipped a tick');
    if (zeroCalls == 2) {
      timer.cancel();
      zeroDone.complete();
    }
  });
  await zeroDone.future;
  print('zero-period: callbacks=$zeroCalls tick=2');
}
