import 'dart:ffi';
typedef F = int Function(int);
class C<T extends Function> {
  void f(DynamicLibrary lib, NativeFunction x) {
    lib.lookupFunction<T, F>('g');
//                     ^
// [diag.mustBeANativeFunctionType] The type 'T' given to 'lookupFunction' must be a valid 'dart:ffi' native function type.
  }
}
