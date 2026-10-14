import 'dart:async';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';
import 'package:dartforge/dartforge.dart';

typedef ContextCallback = Void Function(Pointer<Void>);
typedef TimerCallback = Void Function(Pointer<Void>, Pointer<Void>);

final cf = DynamicLibrary.open(
    '/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation');
final process = DynamicLibrary.process();
final mainThread =
    process.lookupFunction<Int32 Function(), int Function()>('pthread_main_np');
final mainLoop =
    cf.lookupFunction<Pointer<Void> Function(), Pointer<Void> Function()>(
        'CFRunLoopGetMain');
final release = cf.lookupFunction<Void Function(Pointer<Void>),
    void Function(Pointer<Void>)>('CFRelease');
final clock = cf.lookupFunction<Double Function(), double Function()>(
    'CFAbsoluteTimeGetCurrent');
final addTimer = cf.lookupFunction<
    Void Function(Pointer<Void>, Pointer<Void>, Pointer<Void>),
    void Function(
        Pointer<Void>, Pointer<Void>, Pointer<Void>)>('CFRunLoopAddTimer');
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
final allocate = process.lookupFunction<Pointer<Void> Function(Size, Size),
    Pointer<Void> Function(int, int)>('calloc');
final free = process.lookupFunction<Void Function(Pointer<Void>),
    void Function(Pointer<Void>)>('free');

final done = Completer<void>();
bool microtask = false;
bool timer = false;
bool dispatch = false;
bool message = false;

void complete() {
  if (microtask && timer && dispatch && message && !done.isCompleted)
    done.complete();
}

void onDispatch(Pointer<Void> _) {
  if (mainThread() != 1)
    throw StateError('Dispatch callback left the main thread');
  dispatch = true;
  complete();
}

void onTimer(Pointer<Void> _, Pointer<Void> __) {
  if (mainThread() != 1)
    throw StateError('CF timer callback left the main thread');
  // No Dart deadline existed when the OS wait started. Scheduling this timer
  // must signal the persistent source and return to the Dart scheduler.
  scheduleMicrotask(() {
    microtask = true;
    complete();
  });
  Timer(const Duration(milliseconds: 10), () {
    timer = true;
    complete();
  });
}

Future<void> worker(SendPort reply) async {
  for (final operation in [
    EventLoop.enableNativePump,
    EventLoop.disableNativePump
  ]) {
    var rejected = false;
    try {
      operation();
    } on StateError {
      rejected = true;
    }
    if (!rejected) throw StateError('Worker accepted an owner-only operation');
  }
  await Future<void>.delayed(const Duration(milliseconds: 40));
  reply.send(EventLoop.supportsNativePump);
}

Future<void> main() async {
  if (!EventLoop.supportsNativePump) {
    if (EventLoop.supportsNestedTurns ||
        EventLoop.supportsSyncCallbacks != SyncNativeCallable.isSupported)
      throw StateError('Unexpected capability');
    try {
      EventLoop.enableNativePump();
      throw StateError('Fallback accepted native pump');
    } on UnsupportedError {
      print('native pump fallback validated');
    }
    return;
  }
  if (!Platform.isMacOS || mainThread() != 1)
    throw StateError('Unexpected native capability');
  final inbox = ReceivePort();
  inbox.listen((value) {
    if (value != false)
      throw StateError('Worker advertised main-thread capability');
    message = true;
    complete();
  });
  EventLoop.enableNativePump();
  EventLoop.enableNativePump();
  final context = allocate(1, 40);
  final cfTimer = createTimer(nullptr, clock() + 0.02, 0, 0, 0,
      Pointer.fromFunction<TimerCallback>(onTimer), context);
  free(context);
  if (cfTimer == nullptr) throw StateError('CF timer creation failed');
  try {
    addTimer(mainLoop(), cfTimer,
        cf.lookup<Void>('kCFRunLoopDefaultMode').cast<Pointer<Void>>().value);
    final enqueue = process.lookupFunction<
        Void Function(Pointer<Void>, Pointer<Void>,
            Pointer<NativeFunction<ContextCallback>>),
        void Function(Pointer<Void>, Pointer<Void>,
            Pointer<NativeFunction<ContextCallback>>)>('dispatch_async_f');
    enqueue(process.lookup<Void>('_dispatch_main_q'), nullptr,
        Pointer.fromFunction<ContextCallback>(onDispatch));
    await Isolate.spawn(worker, inbox.sendPort);
    await done.future;
    print('native pump main-queue microtask timer message validated');
  } finally {
    inbox.close();
    release(cfTimer);
    EventLoop.disableNativePump();
    EventLoop.disableNativePump();
  }
}
