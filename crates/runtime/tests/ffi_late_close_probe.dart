// Native-only extension probe: never call a closed callback on the official VM.
import 'dart:async';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

@Native<Int64 Function()>(symbol: 'dartforge_ffi_callback_late_calls')
external int lateCalls();

typedef IntegerCallback = Int32 Function(Int32);
typedef DoubleCallback = Double Function(Double);
typedef FloatCallback = Float Function(Float);
typedef BooleanCallback = Bool Function(Bool);
typedef PointerCallback = Pointer<Void> Function(Pointer<Void>);
typedef VoidCallback = Void Function(Pointer<Void>);

void check(bool condition, String message) {
  if (!condition) throw StateError(message);
}

void foreignThread(
    DynamicLibrary helper, Pointer<NativeFunction<VoidCallback>> callback) {
  final invoke = helper.lookupFunction<
          Int32 Function(Pointer<NativeFunction<VoidCallback>>),
          int Function(Pointer<NativeFunction<VoidCallback>>)>(
      'dartforge_test_invoke_callback_on_thread');
  check(invoke(callback) == 0, 'Foreign callback thread failed');
}

int foreignInteger(
    DynamicLibrary helper, Pointer<NativeFunction<IntegerCallback>> callback) {
  final lib = DynamicLibrary.process();
  final malloc = lib.lookupFunction<Pointer<Void> Function(IntPtr),
      Pointer<Void> Function(int)>('malloc');
  final free = lib.lookupFunction<Void Function(Pointer<Void>),
      void Function(Pointer<Void>)>('free');
  final invoke = helper.lookupFunction<
      Int32 Function(
          Pointer<NativeFunction<IntegerCallback>>, Int32, Pointer<Int32>),
      int Function(Pointer<NativeFunction<IntegerCallback>>, int,
          Pointer<Int32>)>('dartforge_test_invoke_integer_on_thread');
  final result = malloc(sizeOf<Int32>()).cast<Int32>();
  try {
    check(invoke(callback, 5, result) == 0,
        'Foreign integer callback thread failed');
    return result.value;
  } finally {
    free(result.cast<Void>());
  }
}

void worker(SendPort sender) {
  final captured = List.generate(128, (i) => i + 200);
  final callback = NativeCallable<IntegerCallback>.isolateLocal(
      (int x) => captured[x],
      exceptionalReturn: 222);
  callback.keepIsolateAlive = false;
  sender.send(callback.nativeFunction.address);
  // Natural worker shutdown must retire this unclosed callback before onExit.
}

Future<void> main(List<String> args) async {
  if (!const bool.fromEnvironment('dartforge.compiler')) {
    print('Late-call extension requires DartForge AOT');
    return;
  }
  final helper = Platform.isWindows ? null : DynamicLibrary.open(args.first);
  final before = lateCalls();
  var calls = 0;
  final captured = List.generate(128, (i) => i);
  final integer = NativeCallable<IntegerCallback>.isolateLocal((int x) {
    calls++;
    return captured[x];
  }, exceptionalReturn: 37);
  final saved = integer.nativeFunction;
  final invoke = saved.asFunction<int Function(int)>();
  check(invoke(5) == 5 && calls == 1, 'Live callback changed');
  integer.close();
  integer.close();
  check(invoke(5) == 37 && calls == 1, 'Closed callback entered Dart');
  if (!Platform.isWindows) {
    check(foreignInteger(helper!, saved) == 37,
        'Foreign late integer result changed');
  }
  final addresses = <int>{saved.address};
  for (var i = 0; i < 256; i++) {
    final next = NativeCallable<IntegerCallback>.isolateLocal((int x) => x + i,
        exceptionalReturn: 111);
    check(addresses.add(next.nativeFunction.address),
        'Published trampoline address reused');
    next.close();
  }
  check(invoke(5) == 37, 'Old pointer rebound to new callback');
  final doubleCallback = NativeCallable<DoubleCallback>.isolateLocal(
      (double x) => x + 1,
      exceptionalReturn: 17.25);
  final doubleInvoke =
      doubleCallback.nativeFunction.asFunction<double Function(double)>();
  doubleCallback.close();
  check(doubleInvoke(1) == 17.25, 'Double exceptional bits changed');
  final floatCallback = NativeCallable<FloatCallback>.isolateLocal(
      (double x) => x + 1,
      exceptionalReturn: 2.5);
  final floatInvoke =
      floatCallback.nativeFunction.asFunction<double Function(double)>();
  floatCallback.close();
  check(floatInvoke(1) == 2.5, 'Float exceptional bits changed');
  final booleanCallback = NativeCallable<BooleanCallback>.isolateLocal(
      (bool x) => !x,
      exceptionalReturn: true);
  final booleanInvoke =
      booleanCallback.nativeFunction.asFunction<bool Function(bool)>();
  booleanCallback.close();
  check(booleanInvoke(false), 'Boolean exceptional bits changed');
  final pointerCallback =
      NativeCallable<PointerCallback>.isolateLocal((Pointer<Void> x) => x);
  final pointerInvoke = pointerCallback.nativeFunction
      .asFunction<Pointer<Void> Function(Pointer<Void>)>();
  pointerCallback.close();
  check(
      pointerInvoke(nullptr).address == 0, 'Pointer exceptional bits changed');
  late NativeCallable<IntegerCallback> self;
  self = NativeCallable<IntegerCallback>.isolateLocal((int x) {
    self.close();
    return x + 7;
  }, exceptionalReturn: 12);
  final selfInvoke = self.nativeFunction.asFunction<int Function(int)>();
  check(selfInvoke(3) == 10 && selfInvoke(3) == 12,
      'Self-close invalidated active callback');
  final voidCallback =
      NativeCallable<VoidCallback>.isolateLocal((Pointer<Void> _) {
    calls++;
  });
  final voidPointer = voidCallback.nativeFunction;
  voidCallback.close();
  if (!Platform.isWindows) foreignThread(helper!, voidPointer);
  final listener = NativeCallable<VoidCallback>.listener((Pointer<Void> _) {
    calls++;
  });
  final listenerPointer = listener.nativeFunction;
  listener.close();
  listenerPointer.asFunction<void Function(Pointer<Void>)>()(nullptr);
  if (!Platform.isWindows) foreignThread(helper!, listenerPointer);
  await Future<void>.delayed(Duration.zero);
  check(calls == 1, 'Retired void/listener callback ran Dart');
  final pointerPort = ReceivePort();
  final exitPort = ReceivePort();
  await Isolate.spawn(worker, pointerPort.sendPort, onExit: exitPort.sendPort);
  final address = await pointerPort.first as int;
  await exitPort.first;
  pointerPort.close();
  exitPort.close();
  final workerPointer =
      Pointer<NativeFunction<IntegerCallback>>.fromAddress(address);
  check(workerPointer.asFunction<int Function(int)>()(5) == 222,
      'Worker callback was not retired before onExit');
  if (!Platform.isWindows) {
    check(foreignInteger(helper!, workerPointer) == 222,
        'Foreign worker tombstone lost exceptional return');
  }
  final expected = Platform.isWindows ? 9 : 13;
  check(lateCalls() - before == expected, 'Late-call diagnostics incorrect');
  print(
      'late callback exceptional returns, stable pointers and foreign drops verified');
}
