import 'dart:ffi';
typedef T = Int8 Function(Int8);
class C<F extends int Function(int)> {
  void f(DynamicLibrary lib, NativeFunction x) {
    lib.lookupFunction<T, F>('g');
//                        ^
// [diag.mustBeASubtype] The type 'T' must be a subtype of 'F' for 'lookupFunction'.
  }
}
