// A blocked native callback skips missed periods without losing Timer.tick.
import 'dart:async';
import 'dart:ffi';
import 'dart:io';

typedef CompareNative = Int32 Function(Pointer<Void>, Pointer<Void>);
int comparisons = 0;
int callbacks = 0;
int compare(Pointer<Void> a, Pointer<Void> b) {
  if (comparisons++ == 0) {
    if (callbacks != 0) throw StateError('Timer ran under native frame');
    sleep(const Duration(milliseconds: 200));
    if (callbacks != 0) throw StateError('Timer ran under native frame');
  }
  return 0;
}

Future<void> main() async {
  final libc = Platform.isWindows
      ? DynamicLibrary.open('ucrtbase.dll')
      : DynamicLibrary.process();
  final malloc = libc.lookupFunction<Pointer<Void> Function(Size),
      Pointer<Void> Function(int)>('malloc');
  final free = libc.lookupFunction<Void Function(Pointer<Void>),
      void Function(Pointer<Void>)>('free');
  final qsort = libc.lookupFunction<
      Void Function(
          Pointer<Void>, Size, Size, Pointer<NativeFunction<CompareNative>>),
      void Function(Pointer<Void>, int, int,
          Pointer<NativeFunction<CompareNative>>)>('qsort');
  final done = Completer<void>();
  Timer.periodic(const Duration(milliseconds: 10), (timer) {
    callbacks++;
    if (!timer.isActive || timer.tick <= callbacks)
      throw StateError('Missed ticks were not counted');
    timer.cancel();
    if (timer.isActive) throw StateError('Timer remains active');
    done.complete();
  });
  final items = malloc(8).cast<Int32>();
  items[0] = 2;
  items[1] = 1;
  qsort(items.cast(), 2, 4, Pointer.fromFunction<CompareNative>(compare, 0));
  free(items.cast());
  if (comparisons == 0 || callbacks != 0)
    throw StateError('Native frame did not block');
  await done.future;
  print('native-frame: callbacks=$callbacks skipped=true active=false');
}
