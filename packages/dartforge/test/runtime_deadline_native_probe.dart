import 'dart:async';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:dartforge/dartforge.dart';

typedef TimerCallback = Void Function(Pointer<Void>, Pointer<Void>);

void check(bool condition, String message) {
  if (!condition) throw StateError(message);
}

Future<void> main() async {
  if (!EventLoop.supportsNativePump) {
    check(EventLoop.nextDeadline == null, 'Fallback exposed native deadline');
    check(
        Isolate.current.threadId == null, 'Fallback exposed native thread ID');
    print('runtime deadline native callback fallback validated');
    return;
  }
  check(Platform.isMacOS, 'Unexpected native pump platform');
  final cf = DynamicLibrary.open(
      '/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation');
  final process = DynamicLibrary.process();
  final mainThread = process
      .lookupFunction<Int32 Function(), int Function()>('pthread_main_np');
  final mainLoop =
      cf.lookupFunction<Pointer<Void> Function(), Pointer<Void> Function()>(
          'CFRunLoopGetMain');
  final clock = cf.lookupFunction<Double Function(), double Function()>(
      'CFAbsoluteTimeGetCurrent');
  final release = cf.lookupFunction<Void Function(Pointer<Void>),
      void Function(Pointer<Void>)>('CFRelease');
  final invalidate = cf.lookupFunction<Void Function(Pointer<Void>),
      void Function(Pointer<Void>)>('CFRunLoopTimerInvalidate');
  final createTimer = cf.lookupFunction<
      Pointer<Void> Function(Pointer<Void>, Double, Double, Uint64, IntPtr,
          Pointer<NativeFunction<TimerCallback>>, Pointer<Void>),
      Pointer<Void> Function(
          Pointer<Void>,
          double,
          double,
          int,
          int,
          Pointer<NativeFunction<TimerCallback>>,
          Pointer<Void>)>('CFRunLoopTimerCreate');
  final addTimer = cf.lookupFunction<
      Void Function(Pointer<Void>, Pointer<Void>, Pointer<Void>),
      void Function(
          Pointer<Void>, Pointer<Void>, Pointer<Void>)>('CFRunLoopAddTimer');
  final allocate = process.lookupFunction<Pointer<Void> Function(Size, Size),
      Pointer<Void> Function(int, int)>('calloc');
  final free = process.lookupFunction<Void Function(Pointer<Void>),
      void Function(Pointer<Void>)>('free');
  final owner = Isolate.current.threadId;
  check(mainThread() == 1 && owner != null, 'Main native owner unavailable');
  final done = Completer<void>();
  final dartTimer = Timer(const Duration(seconds: 1), () {
    if (!done.isCompleted) {
      done.completeError(
          StateError('CF callback did not run before Dart timer'));
    }
  });
  final callable = NativeCallable<TimerCallback>.isolateLocal(
      (Pointer<Void> _, Pointer<Void> __) {
    try {
      check(mainThread() == 1 && Isolate.current.threadId == owner,
          'CF callback changed its native owner');
      final deadline = EventLoop.nextDeadline;
      check(
          deadline != null &&
              deadline.inMicroseconds > 0 &&
              deadline < const Duration(seconds: 1),
          'CF callback did not observe the future Dart timer');
      dartTimer.cancel();
      check(EventLoop.nextDeadline == null,
          'Cancelled Dart timer still appears as ready work');
      scheduleMicrotask(() {
        if (!done.isCompleted) done.complete();
      });
      check(EventLoop.nextDeadline == Duration.zero,
          'Scheduled microtask did not expose immediate readiness');
    } catch (error, stack) {
      // NativeCallable swallows uncaught callback exceptions. Report through
      // the awaited future so every failed assertion remains visible.
      if (!done.isCompleted) done.completeError(error, stack);
    }
  });
  EventLoop.enableNativePump();
  Pointer<Void> cfTimer = nullptr;
  try {
    final context = allocate(1, 40);
    check(context != nullptr, 'CF timer context allocation failed');
    try {
      cfTimer = createTimer(
          nullptr, clock() + 0.02, 0, 0, 0, callable.nativeFunction, context);
    } finally {
      free(context);
    }
    check(cfTimer != nullptr, 'CF timer creation failed');
    addTimer(mainLoop(), cfTimer,
        cf.lookup<Void>('kCFRunLoopDefaultMode').cast<Pointer<Void>>().value);
    await done.future;
    print(
        'runtime deadline future cancel microtask and native owner validated');
  } finally {
    dartTimer.cancel();
    if (cfTimer != nullptr) {
      invalidate(cfTimer);
      release(cfTimer);
    }
    callable.close();
    EventLoop.disableNativePump();
  }
}
