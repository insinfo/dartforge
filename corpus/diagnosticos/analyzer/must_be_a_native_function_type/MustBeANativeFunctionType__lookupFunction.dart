import 'dart:ffi';
typedef S = int Function(int);
typedef F = String Function(String);
void f(DynamicLibrary lib) {
  lib.lookupFunction<S, F>('g');
//                   ^
// [diag.mustBeANativeFunctionType] The type 'S' given to 'lookupFunction' must be a valid 'dart:ffi' native function type.
}
