/// Differential contracts for the current dart_ui native allocator, arena,
/// mutex and Objective-C ABI structs. No closed trampoline is ever invoked.
/// Native views remain inside the lifetime of their backing allocation.
import 'dart:async';
import 'dart:ffi';
import 'dart:typed_data';

import 'package:dart_ui/src/concurrency/native_mutex.dart';
import 'package:dart_ui/src/ffi/native_memory.dart';
import 'package:dart_ui/src/ffi/objc_runtime.dart'
    show ObjCDouble2, ObjCDouble4;

typedef LocalNative = Int32 Function(Int32);
typedef LocalDart = int Function(int);
typedef ListenerNative = Void Function(Int32);
typedef ListenerDart = void Function(int);

void require(bool condition, String message) {
  if (!condition) throw StateError(message);
}

String bytes(Uint8List values) {
  const digits = '0123456789abcdef';
  final output = StringBuffer();
  for (final value in values) {
    output.write(digits[value >> 4]);
    output.write(digits[value & 15]);
  }
  return output.toString();
}

void exerciseViews(Uint8List values) {
  for (var i = 0; i < values.length; i++) values[i] = i;
  final middle = Uint8List.sublistView(values, 3, 27);
  middle.setRange(4, 20, middle, 0);
  middle.setRange(0, 12, middle, 8);
  final words = ByteData.view(values.buffer, values.offsetInBytes + 4, 16);
  words.setUint32(4, 0x12345678, Endian.little);
  require(words.getUint32(4, Endian.little) == 0x12345678, 'ByteData offset');
}

void memoryContracts() {
  final allocator = NativeAllocator.instance;
  allocator.trackBlockSizes(true);
  final before = allocator.stats;
  final pointer = allocator.allocate<Uint8>(64, alignment: 16);
  try {
    require(pointer.address % 16 == 0, 'allocator alignment');
    final external = pointer.asTypedList(64);
    for (final value in external) require(value == 0, 'allocator zeroing');
    final internal = Uint8List(64);
    exerciseViews(internal);
    exerciseViews(external);
    require(
        bytes(internal) == bytes(external), 'external/internal view parity');
    print('views=${bytes(external)}');

    // Distinct view objects alias the same native bytes. Copies must preserve
    // the source range in both overlap directions, just as an internal view.
    final first = pointer.asTypedList(32);
    final second = (pointer + 4).asTypedList(28);
    final expected = Uint8List.fromList(first);
    expected.setRange(8, 24, expected, 4);
    first.setRange(8, 24, second, 0);
    require(bytes(first) == bytes(expected), 'external alias forward overlap');
    final snapshot = Uint8List.fromList(first);
    first.setRange(0, 16, second, 4);
    expected.setRange(0, 16, snapshot, 8);
    require(bytes(first) == bytes(expected), 'external alias reverse overlap');
    print('overlap=${bytes(first)}');
  } finally {
    allocator.free(pointer);
  }

  final pair =
      allocator.allocate<ObjCDouble2>(sizeOf<ObjCDouble2>(), alignment: 16);
  try {
    pair.ref.a0 = 1.25;
    pair.ref.a1 = -2.5;
    final view = pair.cast<Double>().asTypedList(2);
    require(view[0] == 1.25 && view[1] == -2.5, 'real double-pair layout');
    print('pair size=${sizeOf<ObjCDouble2>()} values=${view[0]},${view[1]}');
  } finally {
    allocator.free(pair);
  }
  final quad =
      allocator.allocate<ObjCDouble4>(sizeOf<ObjCDouble4>(), alignment: 16);
  try {
    quad.ref.a0 = 1.25;
    quad.ref.a1 = -2.5;
    quad.ref.a2 = 3.75;
    quad.ref.a3 = -4.125;
    final view = quad.cast<Double>().asTypedList(4);
    require(
        view[0] == 1.25 &&
            view[1] == -2.5 &&
            view[2] == 3.75 &&
            view[3] == -4.125,
        'real double-quad layout');
    print(
        'quad size=${sizeOf<ObjCDouble4>()} values=${view[0]},${view[1]},${view[2]},${view[3]}');
  } finally {
    allocator.free(quad);
  }

  var alignmentRejected = false;
  try {
    final unsupported = allocator.allocate<Uint8>(16, alignment: 32);
    allocator.free(unsupported);
  } on ArgumentError {
    alignmentRejected = true;
  }
  require(alignmentRejected, 'stricter alignment must reject');

  final arena = NativeArena();
  try {
    const text = 'UI caf\u00e9 \u{1f680}';
    final utf8 = arena.allocateUtf8(text);
    final utf16 = arena.allocateUtf16(text);
    require(readNativeUtf8(utf8, limit: 64) == text, 'arena UTF-8');
    require(readNativeUtf16(utf16, limit: 64) == text, 'arena UTF-16');
    require(arena.length == 2, 'arena allocation count');
    arena.releaseAll();
    require(arena.length == 0, 'arena releaseAll');
    final ascii = arena.allocateAscii('reuse');
    require(readNativeAscii(ascii, limit: 16) == 'reuse', 'arena reuse');
    print('arena utf8/utf16/reuse verified');
  } finally {
    arena.dispose();
  }

  final mutex = NativeMutex.allocate();
  try {
    require(mutex.tryAcquire(), 'uncontended mutex');
    mutex.release();
    var threw = false;
    try {
      mutex.runLocked<void>(() {
        throw StateError('critical-section probe');
      });
    } on StateError {
      threw = true;
    }
    require(threw && mutex.tryAcquire(), 'finally must release mutex');
    mutex.release();
    mutex.checkGuard();
    print('mutex acquire/exception/reacquire/guard verified');
  } finally {
    mutex.destroy();
  }
  mutex.destroy();
  final after = allocator.stats;
  require(
      after.liveBlocks == before.liveBlocks &&
          after.liveBytes == before.liveBytes,
      'native allocations leaked');
  require(
      after.totalAllocations - before.totalAllocations ==
          after.totalReleases - before.totalReleases,
      'native allocation/release accounting');
  final allocations = after.totalAllocations - before.totalAllocations;
  final releases = after.totalReleases - before.totalReleases;
  require(allocations == 7, 'expected seven tracked native allocations');
  print('native memory balanced allocations=$allocations releases=$releases');
}

Future<void> callbackContracts() async {
  final trace = <String>[];
  final local = NativeCallable<LocalNative>.isolateLocal((int value) {
    trace.add('local:$value');
    if (value < 0) throw StateError('callback exceptional return probe');
    return value * 3;
  }, exceptionalReturn: 999);
  try {
    final invoke = local.nativeFunction.asFunction<LocalDart>();
    trace.add('before-local');
    final normal = invoke(7);
    final exceptional = invoke(-1);
    require(normal == 21 && exceptional == 999, 'local callback returns');
    trace.add('after-local:$normal,$exceptional');
  } finally {
    local.close();
  }

  final done = Completer<void>();
  var count = 0;
  final listener = NativeCallable<ListenerNative>.listener((int value) {
    trace.add('listener:$value');
    if (value < 0) {
      try {
        throw StateError('listener handler probe');
      } on StateError {
        trace.add('listener-caught');
      }
    }
    count++;
    if (count == 3) done.complete();
  });
  try {
    final invoke = listener.nativeFunction.asFunction<ListenerDart>();
    invoke(11);
    invoke(-22);
    invoke(33);
    require(count == 0, 'listener must enqueue rather than run synchronously');
    trace.add('posted');
    await done.future;
    require(count == 3, 'listener delivery count');
  } finally {
    // All posted calls have completed before close; no late native calls occur.
    listener.close();
  }
  print('callbacks=${trace.join('|')}');
}

Future<void> main() async {
  memoryContracts();
  await callbackContracts();
  print('ffi fixtures verified');
}
