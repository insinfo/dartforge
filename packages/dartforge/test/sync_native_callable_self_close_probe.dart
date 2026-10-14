import 'dart:async';
import 'dart:ffi';

import 'package:dartforge/dartforge.dart';

typedef BufferCallback = Int32 Function(Pointer<Int32>);

void check(bool condition, String message) {
  if (!condition) throw StateError(message);
}

SyncNativeCallable<BufferCallback> createSelfClosing(
    int Function() onOwner, void Function(int) sleep, void Function() count) {
  // Only the active callback capture keeps these objects reachable after close.
  final capturedList = <int>[11, 13];
  final capturedMap = <String, int>{'offset': 17, 'marker': 29};
  final self = <SyncNativeCallable<BufferCallback>>[];
  final callback = SyncNativeCallable<BufferCallback>((Pointer<Int32> buffer) {
    count();
    check(onOwner() == 1, 'Self-closing callback ran outside owner');
    final before = buffer.value;
    self.single.close();
    check(!self.single.keepIsolateAlive, 'Self-close retained owner keepalive');
    check(capturedList[0] == 11 && capturedMap['marker'] == 29,
        'Self-close invalidated captured objects');
    // The pre-claim timeout is 30 ms. Claimed work keeps native storage alive
    // until completion even after it closes its own port/context root.
    sleep(80);
    check(onOwner() == 1, 'Self-closing callback changed owner thread');
    check(
        buffer.value == before, 'Self-close reclaimed running native storage');
    check(capturedList[1] == 13 && capturedMap['marker'] == 29,
        'Captured list/map did not survive active frame');
    return before + capturedList[0] + capturedList[1] + capturedMap['offset']!;
  }, exceptionalReturn: -81, queueTimeout: const Duration(milliseconds: 30));
  self.add(callback);
  return callback;
}

Future<void> main(List<String> arguments) async {
  if (!SyncNativeCallable.isSupported) {
    var unsupported = false;
    try {
      SyncNativeCallable<BufferCallback>((Pointer<Int32> _) => 0,
          exceptionalReturn: -81);
    } on UnsupportedError {
      unsupported = true;
    }
    check(unsupported, 'Official runtime created synchronous callback');
    print('sync self-close callback fallback validated');
    return;
  }
  check(arguments.length == 1, 'Expected the POSIX producer library path');
  final library = DynamicLibrary.open(arguments.single);
  final initialize = library.lookupFunction<Int32 Function(), int Function()>(
      'sync_probe_initialize');
  final finish = library
      .lookupFunction<Void Function(), void Function()>('sync_probe_finish');
  final onOwner = library
      .lookupFunction<Int32 Function(), int Function()>('sync_probe_on_owner');
  final sleep =
      library.lookupFunction<Void Function(Int32), void Function(int)>(
          'sync_probe_sleep');
  final start = library.lookupFunction<
      Pointer<Void> Function(Pointer<Void>, Int32, Int32),
      Pointer<Void> Function(Pointer<Void>, int, int)>('sync_probe_start');
  final done = library.lookupFunction<Int32 Function(Pointer<Void>),
      int Function(Pointer<Void>)>('sync_probe_done');
  final integer = library.lookupFunction<Int64 Function(Pointer<Void>),
      int Function(Pointer<Void>)>('sync_probe_integer');
  final destroy = library.lookupFunction<Int32 Function(Pointer<Void>),
      int Function(Pointer<Void>)>('sync_probe_destroy');
  check(initialize() == 1, 'Unable to start native watchdog');
  var calls = 0;
  final callback = createSelfClosing(onOwner, sleep, () {
    calls++;
  });
  final saved = callback.nativeFunction;

  Future<int> invoke(int value) async {
    final state = start(saved.cast<Void>(), 6, value);
    check(state != nullptr, 'Unable to start native caller');
    try {
      final limit = DateTime.now().add(const Duration(seconds: 2));
      while (done(state) == 0) {
        check(
            DateTime.now().isBefore(limit), 'Native caller failed to complete');
        await Future<void>.delayed(const Duration(milliseconds: 1));
      }
      return integer(state);
    } finally {
      if (done(state) == 1) {
        check(destroy(state) == 1, 'Completed native state was not reclaimed');
      }
    }
  }

  try {
    check(await invoke(41) == 82 && calls == 1,
        'Self-close timed out or lost active captured state');
    var inaccessible = false;
    try {
      callback.nativeFunction;
    } on StateError {
      inaccessible = true;
    }
    check(inaccessible, 'Closed callback still exposes native pointer');
    check(await invoke(43) == -81 && calls == 1,
        'Late foreign callback reran a self-closed handler');
    check(saved.asFunction<int Function(Pointer<Int32>)>()(nullptr) == -81,
        'Late owner callback dereferenced its null argument');
    print(
        'sync callback self-close preserves active captures and native storage');
  } finally {
    callback.close();
    finish();
  }
}
