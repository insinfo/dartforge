import 'dart:async';
import 'dart:ffi';
import 'dart:isolate';

import 'package:dartforge/dartforge.dart';

typedef IntegerCallback = Int32 Function(Int32);
typedef BufferCallback = Int32 Function(Pointer<Int32>);

final class Pair8 extends Struct {
  @Int32()
  external int first;

  @Int32()
  external int second;
}

void check(bool condition, String message) {
  if (!condition) throw StateError(message);
}

void cycleWorker(List<Object?> arguments) {
  final ownerAddress = arguments[0] as int;
  final ready = arguments[1] as SendPort;
  final owner =
      Pointer<NativeFunction<IntegerCallback>>.fromAddress(ownerAddress)
          .asFunction<int Function(int)>();
  final callback = SyncNativeCallable<IntegerCallback>(
      (int value) => owner(value) + 5,
      exceptionalReturn: -80,
      queueTimeout: const Duration(seconds: 10));
  final control = RawReceivePort();
  control.handler = (Object? message) {
    if (message != 'stop')
      throw StateError('Unexpected worker control message');
    callback.close();
    control.close();
  };
  ready.send(<Object?>[callback.nativeFunction.address, control.sendPort]);
}

Future<void> main(List<String> arguments) async {
  var zeroRejected = false;
  try {
    SyncNativeCallable<IntegerCallback>((int v) => v,
        exceptionalReturn: -1, queueTimeout: Duration.zero);
  } on ArgumentError {
    zeroRejected = true;
  }
  check(zeroRejected, 'Zero queue timeout accepted');
  if (!SyncNativeCallable.isSupported) {
    var unsupported = false;
    try {
      SyncNativeCallable<IntegerCallback>((int v) => v, exceptionalReturn: -1);
    } on UnsupportedError {
      unsupported = true;
    }
    check(unsupported, 'Official runtime created a synchronous callback');
    print('sync callback fallback validated');
    return;
  }
  check(arguments.length == 1, 'Expected the POSIX producer library path');
  final library = DynamicLibrary.open(arguments[0]);
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
  final real = library.lookupFunction<Double Function(Pointer<Void>),
      double Function(Pointer<Void>)>('sync_probe_real');
  final destroy = library.lookupFunction<Int32 Function(Pointer<Void>),
      int Function(Pointer<Void>)>('sync_probe_destroy');
  check(initialize() == 1, 'Unable to start the native watchdog');
  final live = <ScopedResource>[];

  Future<void> wait(Pointer<Void> state) async {
    check(state != nullptr, 'Unable to start native caller');
    final limit = DateTime.now().add(const Duration(seconds: 2));
    while (done(state) == 0) {
      check(DateTime.now().isBefore(limit), 'Native caller failed to complete');
      await Future<void>.delayed(const Duration(milliseconds: 1));
    }
  }

  Future<num> invoke(Pointer<Void> function, int kind, int value) async {
    final state = start(function, kind, value);
    try {
      await wait(state);
      return kind == 1 || kind == 2 ? real(state) : integer(state);
    } finally {
      if (state != nullptr && done(state) == 1) {
        check(destroy(state) == 1, 'Completed native state was not reclaimed');
      }
    }
  }

  try {
    void reject(ScopedResource Function() create, String label) {
      var rejected = false;
      ScopedResource? resource;
      try {
        resource = create();
      } on UnsupportedError {
        rejected = true;
      } finally {
        resource?.dispose();
      }
      check(rejected, '$label was not rejected as unsupported');
    }

    reject(
        () => SyncNativeCallable<Int32 Function(Handle)>((Object? value) => 1,
            exceptionalReturn: -1),
        'Handle argument');
    reject(
        () => SyncNativeCallable<Handle Function(Int32)>((int value) => null),
        'Handle return');
    reject(
        () => SyncNativeCallable<Pair8 Function(Int32)>((int value) {
              throw StateError('Unsupported aggregate callback executed');
            }),
        'Aggregate return');

    final ownerName = Isolate.current.debugName;
    final values = <String, int>{'offset': 7, 'calls': 0};
    final seen = <int>[];
    final scalar = SyncNativeCallable<IntegerCallback>((int value) {
      check(onOwner() == 1, 'Callback ran outside its owner thread');
      check(Isolate.current.debugName == ownerName,
          'Callback changed owner isolate');
      values['calls'] = values['calls']! + 1;
      seen.add(value);
      return value + values['offset']!;
    }, exceptionalReturn: -91);
    live.add(scalar);
    check(scalar.keepIsolateAlive, 'New callback does not keep owner alive');
    scalar.keepIsolateAlive = false;
    check(!scalar.keepIsolateAlive, 'Keepalive update failed');
    scalar.keepIsolateAlive = true;
    final direct = scalar.nativeFunction.asFunction<int Function(int)>();
    check(direct(4) == 11, 'Same-owner callback result differs');
    check(await invoke(scalar.nativeFunction.cast<Void>(), 0, 12) == 19,
        'Foreign integer callback result differs');
    check(values['calls'] == 2 && seen[0] == 4 && seen[1] == 12,
        'Captured map/list or callback count differs');

    final doubles = SyncNativeCallable<Double Function(Double)>((double v) {
      check(onOwner() == 1, 'Double callback ran outside owner');
      return v + 0.125;
    }, exceptionalReturn: -9.0);
    live.add(doubles);
    check(await invoke(doubles.nativeFunction.cast<Void>(), 1, 0) == 2.625,
        'Double callback result differs');
    final floats = SyncNativeCallable<Float Function(Float)>(
        (double v) => v * 2,
        exceptionalReturn: -9.0);
    live.add(floats);
    check(await invoke(floats.nativeFunction.cast<Void>(), 2, 0) == 2.5,
        'Float callback result differs');
    final booleans = SyncNativeCallable<Bool Function(Bool)>((bool v) => !v,
        exceptionalReturn: true);
    live.add(booleans);
    check(await invoke(booleans.nativeFunction.cast<Void>(), 3, 0) == 0,
        'Bool callback result differs');
    final pointers = SyncNativeCallable<Pointer<Void> Function(Pointer<Void>)>(
        (Pointer<Void> v) => v,
        exceptionalReturn: nullptr);
    live.add(pointers);
    check(await invoke(pointers.nativeFunction.cast<Void>(), 4, 0) == 1,
        'Pointer callback result differs');
    final pairs = SyncNativeCallable<Int32 Function(Pair8)>((Pair8 value) {
      check(onOwner() == 1, 'Aggregate argument ran outside owner');
      return value.first + value.second;
    }, exceptionalReturn: -9);
    live.add(pairs);
    check(await invoke(pairs.nativeFunction.cast<Void>(), 7, 41) == 46,
        'Copied aggregate callback argument differs');
    var voidValue = 0;
    final voids = SyncNativeCallable<Void Function(Int32)>((int v) {
      voidValue = v;
    });
    live.add(voids);
    check(
        await invoke(voids.nativeFunction.cast<Void>(), 5, 31) == 1 &&
            voidValue == 31,
        'Void callback was not delivered');

    var throwingCalls = 0;
    final throwing = SyncNativeCallable<IntegerCallback>((int value) {
      throwingCalls++;
      if (value == 1) throw StateError('Expected callback failure');
      return value * 3;
    }, exceptionalReturn: -73);
    live.add(throwing);
    check(await invoke(throwing.nativeFunction.cast<Void>(), 0, 1) == -73,
        'Thrown callback did not return exceptional value');
    check(
        await invoke(throwing.nativeFunction.cast<Void>(), 0, 2) == 6 &&
            throwingCalls == 2,
        'Owner stopped after callback exception');

    var expiredCalls = 0;
    final expiring = SyncNativeCallable<IntegerCallback>((int v) {
      expiredCalls++;
      return v;
    }, exceptionalReturn: -52, queueTimeout: const Duration(milliseconds: 20));
    live.add(expiring);
    final expired = start(expiring.nativeFunction.cast<Void>(), 0, 17);
    try {
      // Block only the owner, allowing the producer's queue deadline to expire.
      sleep(60);
      await wait(expired);
      check(
          integer(expired) == -52, 'Queued timeout did not return exceptional');
      await Future<void>.delayed(const Duration(milliseconds: 5));
      check(expiredCalls == 0, 'Expired callback executed after owner resumed');
    } finally {
      if (expired != nullptr && done(expired) == 1) destroy(expired);
    }

    var closedCalls = 0;
    final closing = SyncNativeCallable<IntegerCallback>((int v) {
      closedCalls++;
      return v;
    }, exceptionalReturn: -64);
    live.add(closing);
    final saved = closing.nativeFunction;
    final closed = start(saved.cast<Void>(), 0, 18);
    closing.close();
    try {
      await wait(closed);
      check(integer(closed) == -64 && closedCalls == 0,
          'Close did not cancel an unclaimed callback');
      check(!closing.keepIsolateAlive, 'Closed callback retains keepalive');
      var inaccessible = false;
      try {
        closing.nativeFunction;
      } on StateError {
        inaccessible = true;
      }
      check(inaccessible, 'Closed callback still exposes native pointer');
      check(saved.asFunction<int Function(int)>()(20) == -64,
          'Closed same-owner trampoline did not return exceptional');
    } finally {
      if (closed != nullptr && done(closed) == 1) destroy(closed);
    }

    var runningCalls = 0;
    final running = SyncNativeCallable<BufferCallback>((Pointer<Int32> buffer) {
      runningCalls++;
      check(onOwner() == 1, 'Buffer callback ran outside owner');
      final before = buffer.value;
      // Once claimed, expiration must not reclaim the producer's argument.
      sleep(80);
      check(buffer.value == before, 'Running native argument storage changed');
      return before + 2;
    }, exceptionalReturn: -81, queueTimeout: const Duration(milliseconds: 30));
    live.add(running);
    check(
        await invoke(running.nativeFunction.cast<Void>(), 6, 41) == 43 &&
            runningCalls == 1,
        'A running callback was timed out or delivered twice');

    var cycleOwnerCalls = 0;
    final cycleOwner = SyncNativeCallable<IntegerCallback>((int value) {
      cycleOwnerCalls++;
      return value;
    }, exceptionalReturn: -70, queueTimeout: const Duration(seconds: 10));
    live.add(cycleOwner);
    final ready = ReceivePort();
    final exited = ReceivePort();
    final errors = ReceivePort();
    Object? workerError;
    errors.listen((Object? message) {
      workerError = message;
    });
    SendPort? control;
    try {
      await Isolate.spawn(cycleWorker,
          <Object?>[cycleOwner.nativeFunction.address, ready.sendPort],
          onExit: exited.sendPort,
          onError: errors.sendPort,
          errorsAreFatal: true,
          debugName: 'sync-callback-worker');
      final response = await ready.first as List;
      final workerAddress = response[0] as int;
      control = response[1] as SendPort;
      final worker =
          Pointer<NativeFunction<IntegerCallback>>.fromAddress(workerAddress)
              .asFunction<int Function(int)>();
      // A waits for B; B tries to wait for A. A ten-second queue timeout
      // exceeds the three-second native watchdog, so expiration cannot pass.
      check(worker(7) == -65, 'Cross-isolate wait cycle was not rejected');
      check(cycleOwnerCalls == 0, 'Cycle rejection invoked the blocked owner');
      control.send('stop');
      control = null;
      await exited.first;
      await Future<void>.delayed(const Duration(milliseconds: 1));
      check(
          cycleOwnerCalls == 0, 'Rejected cycle left a runnable owner request');
      check(workerError == null, 'Worker isolate failed: $workerError');
      check(worker(8) == -80, 'Retired worker callback lost its tombstone');
    } finally {
      control?.send('stop');
      ready.close();
      exited.close();
      errors.close();
    }
    print('sync callback owner delivery and cancellation validated');
  } finally {
    for (final resource in live) {
      resource.dispose();
    }
    finish();
  }
}
