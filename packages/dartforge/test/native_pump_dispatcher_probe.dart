import 'dart:async';
import 'dart:ffi';
import 'dart:isolate';
import 'package:dartforge/dartforge.dart';

typedef Callback = Void Function(Pointer<Void>);
bool insideNative = false;
bool microtaskRan = false;
int callbacks = 0;
final done = Completer<void>();

void wake(Pointer<Void> _) {
  insideNative = true;
  EventLoop.setNativeEventDispatcher(() {
    if (insideNative || !microtaskRan)
      throw StateError(
          'Dispatcher ran in a foreign frame or before microtasks');
    callbacks++;
    final scratch = List.generate(100, (index) => <int>[index]);
    if (scratch.last.single != 99)
      throw StateError('Scratch allocation failed');
    EventLoop.clearNativeEventDispatcher();
    EventLoop.setNativeEventDispatcher(() {
      callbacks++;
      EventLoop.disableNativePump();
      final scratch = List.generate(100, (index) => <int>[index]);
      if (scratch.first.single != 0)
        throw StateError('Active callback lost its root');
      done.complete();
    });
  });
  scheduleMicrotask(() {
    microtaskRan = true;
  });
  insideNative = false;
}

Future<void> worker(SendPort reply) async {
  for (final operation in [
    () => EventLoop.setNativeEventDispatcher(() {}),
    EventLoop.clearNativeEventDispatcher
  ]) {
    var rejected = false;
    try {
      operation();
    } on StateError {
      rejected = true;
    }
    if (!rejected) throw StateError('Dispatcher accepted a foreign owner');
  }
  reply.send(true);
}

void fail() => throw StateError('native dispatcher exception sentinel');

Future<void> main(List<String> args) async {
  if (args.contains('--microtask-error')) {
    scheduleMicrotask(fail);
    return;
  }
  if (!EventLoop.supportsNativeEventDispatcher) {
    for (final operation in [
      () => EventLoop.setNativeEventDispatcher(() {}),
      EventLoop.clearNativeEventDispatcher
    ]) {
      var rejected = false;
      try {
        operation();
      } on UnsupportedError {
        rejected = true;
      }
      if (!rejected) throw StateError('Fallback accepted a dispatcher');
    }
    print('native dispatcher fallback validated');
    return;
  }
  if (args.contains('--throw')) {
    EventLoop.enableNativePump();
    EventLoop.setNativeEventDispatcher(fail);
    return;
  }
  var rejected = false;
  try {
    EventLoop.setNativeEventDispatcher(() {});
  } on StateError {
    rejected = true;
  }
  if (!rejected) throw StateError('Dispatcher accepted a disabled pump');
  final inbox = ReceivePort();
  final ownerCheck = Completer<void>();
  inbox.listen((value) {
    if (value != true) throw StateError('Owner check failed');
    inbox.close();
    ownerCheck.complete();
  });
  EventLoop.enableNativePump();
  EventLoop.clearNativeEventDispatcher();
  EventLoop.clearNativeEventDispatcher();
  await Isolate.spawn(worker, inbox.sendPort);
  await ownerCheck.future;
  final process = DynamicLibrary.process();
  final enqueue = process.lookupFunction<
      Void Function(
          Pointer<Void>, Pointer<Void>, Pointer<NativeFunction<Callback>>),
      void Function(
          Pointer<Void>, Pointer<Void>, Pointer<NativeFunction<Callback>>)>(
    'dispatch_async_f',
  );
  enqueue(process.lookup<Void>('_dispatch_main_q'), nullptr,
      Pointer.fromFunction<Callback>(wake));
  await done.future;
  if (callbacks != 2) throw StateError('Unexpected dispatcher callback count');
  print('native dispatcher outer-turn replacement disable roots validated');
}
