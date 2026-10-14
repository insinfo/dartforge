// Native-only lifecycle extension: retain the closure through error formatting,
// then have an actual C atexit handler call its saved pointer after finalization.
import 'dart:ffi';

typedef IntegerCallback = Int32 Function(Int32);

final class CallbackError implements Exception {
  CallbackError(this.invoke);
  final int Function(int) invoke;
  @override
  String toString() => 'error callback lifetime verified: ${invoke(5)}';
}

void main(List<String> args) {
  if (!const bool.fromEnvironment('dartforge.compiler')) {
    print('Late-call extension requires DartForge AOT');
    return;
  }
  final helper = DynamicLibrary.open(args.first);
  final register = helper.lookupFunction<
      Int32 Function(Pointer<NativeFunction<IntegerCallback>>, Int32),
      int Function(Pointer<NativeFunction<IntegerCallback>>,
          int)>('dartforge_test_register_atexit');
  final captured = [10];
  final callback = NativeCallable<IntegerCallback>.isolateLocal(
      (int x) => x + captured[0],
      exceptionalReturn: 69);
  callback.keepIsolateAlive = false;
  final pointer = callback.nativeFunction;
  if (register(pointer, 69) != 0)
    throw StateError('atexit registration failed');
  if (args.contains('--error')) {
    throw CallbackError(pointer.asFunction<int Function(int)>());
  }
  print('normal main completed before callback retirement');
}
