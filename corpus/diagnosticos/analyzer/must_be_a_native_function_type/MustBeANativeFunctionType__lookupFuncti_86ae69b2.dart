import 'dart:ffi';
typedef S = Void Function(Pointer<NativeFunction>);
typedef F = void Function(Pointer<NativeFunction>);
void f(DynamicLibrary lib) {
  lib.lookupFunction<S, F>('g');
//                   ^
// [diag.mustBeANativeFunctionType] The type 'S' given to 'lookupFunction' must be a valid 'dart:ffi' native function type.
}
