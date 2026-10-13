import 'dart:ffi';
typedef TPrime = int Function(int);
typedef F = String Function(String);
class C {
  void f(Pointer<NativeFunction<TPrime>> p) {
    p.asFunction<F>();
//               ^
// [diag.nonNativeFunctionTypeArgumentToPointer] Can't invoke 'asFunction' because the function signature 'NativeFunction<TPrime>' for the pointer isn't a valid C function signature.
  }
}
