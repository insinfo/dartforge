import 'dart:async';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';
import 'dart:typed_data';

import 'package:dartforge/dartforge.dart' show EventLoop;
import 'package:dartforge/gc.dart' show GarbageCollector;
import 'package:dartforge/isolate.dart' show DartForgeIsolate;

void check(bool value, String message) {
  if (!value) throw StateError(message);
}

Future<void> worker(List<Object> message) async {
  final reply = message[0] as SendPort;
  final id = Isolate.current.threadId;
  await Future<void>.delayed(const Duration(milliseconds: 2));
  check(id != null && id == Isolate.current.threadId,
      'Worker identity changed across await');
  reply.send(id);
}

void verifyMacIdentity(int id) {
  if (!Platform.isMacOS) return;
  final process = DynamicLibrary.process();
  final allocate = process.lookupFunction<Pointer<Uint64> Function(Size, Size),
      Pointer<Uint64> Function(int, int)>('calloc');
  final release = process.lookupFunction<Void Function(Pointer<Void>),
      void Function(Pointer<Void>)>('free');
  final current = process.lookupFunction<
      Int32 Function(Pointer<Void>, Pointer<Uint64>),
      int Function(Pointer<Void>, Pointer<Uint64>)>('pthread_threadid_np');
  final value = allocate(1, 8);
  check(value != nullptr, 'Native identity allocation failed');
  try {
    check(current(nullptr, value) == 0 && value.value == id,
        'Reported thread identity differs from the OS');
  } finally {
    release(value.cast<Void>());
  }
}

Future<void> main() async {
  var invalid = false;
  try {
    GarbageCollector.deferFor(Duration.zero);
  } on ArgumentError {
    invalid = true;
  }
  check(invalid, 'Zero deferral accepted');
  if (!GarbageCollector.isSupported) {
    check(
        GarbageCollector.collectionCount == null, 'Fallback reported GC stats');
    check(
        Isolate.current.threadId == null, 'Fallback reported native identity');
    check(EventLoop.nextDeadline == null, 'Fallback reported native deadline');
    check(!GarbageCollector.deferFor(const Duration(milliseconds: 1)),
        'Fallback accepted GC deferral');
    GarbageCollector.cancelDeferral();
    GarbageCollector.collectWhenIdle();
    print('runtime policy fallback validated');
    return;
  }

  final owner = Isolate.current.threadId;
  check(owner != null && owner > 0, 'Missing owner OS identity');
  verifyMacIdentity(owner!);
  final timer = Timer(const Duration(seconds: 1), () {});
  final deadline = EventLoop.nextDeadline;
  check(
      deadline != null &&
      deadline.inMicroseconds >= 0 &&
      deadline.inMicroseconds <= 1001000,
      'Timer snapshot was neither ready nor within its relative deadline: $deadline');
  timer.cancel();
  final microtask = Completer<void>();
  scheduleMicrotask(microtask.complete);
  check(EventLoop.nextDeadline == Duration.zero,
      'Queued microtask was not ready');
  await microtask.future;
  check(
      owner == Isolate.current.threadId, 'Owner identity changed across await');

  final inbox = ReceivePort();
  final spawned = await Isolate.spawn(worker, <Object>[inbox.sendPort]);
  check(spawned.threadId == null,
      'Foreign isolate exposed caller thread identity');
  final workerId = await inbox.first;
  inbox.close();
  check(workerId is int && workerId != owner,
      'Worker did not use a distinct OS thread');

  final stress = Platform.environment.containsKey('DARTFORGE_GC_STRESS');
  final payloads = <Uint8List>[];
  check(
      GarbageCollector.deferFor(const Duration(seconds: 2),
          maxExtraBytes: 16 * 1024 * 1024),
      'Initial bounded deferral rejected');
  final initial = GarbageCollector.collectionCount!;
  for (var i = 0; i < 48; i++) {
    final bytes = Uint8List(64 * 1024);
    bytes[0] = i;
    payloads.add(bytes);
  }
  final after = GarbageCollector.collectionCount!;
  check(stress ? after > initial : after == initial,
      'Deferral ignored normal triggers or suppressed stress collection');
  GarbageCollector.cancelDeferral();
  // A safe allocation services any pending debt before the next window.
  final marker = Uint8List(4096);
  marker[0] = 77;

  check(GarbageCollector.deferFor(const Duration(seconds: 2), maxExtraBytes: 1),
      'Fresh byte-budget deferral rejected');
  final beforeBudget = GarbageCollector.collectionCount!;
  final oversized = Uint8List(4096);
  oversized[0] = 88;
  check(GarbageCollector.collectionCount! > beforeBudget,
      'Allocation exceeded the allowance without servicing collection debt');

  final idle = Completer<void>();
  var countAtTimer = 0;
  Timer(const Duration(milliseconds: 150), () {
    countAtTimer = GarbageCollector.collectionCount!;
    idle.complete();
  });
  check(
      GarbageCollector.deferUntil(
      GarbageCollector.monotonicTime + const Duration(milliseconds: 50),
          maxExtraBytes: 1024 * 1024),
      'Idle deferral rejected');
  final beforeIdle = GarbageCollector.collectionCount!;
  GarbageCollector.collectWhenIdle();
  await idle.future;
  check(countAtTimer > beforeIdle,
      'Deferred idle request did not wake and collect');
  check(
      payloads.first[0] == 0 &&
          payloads.last[0] == 47 &&
          marker[0] == 77 &&
          oversized[0] == 88,
      'Collection lost rooted buffers');
  check(owner == Isolate.current.threadId, 'Final owner identity changed');
  GarbageCollector.cancelDeferral();
  print('runtime deadline, thread identity and bounded GC policy validated');
}
