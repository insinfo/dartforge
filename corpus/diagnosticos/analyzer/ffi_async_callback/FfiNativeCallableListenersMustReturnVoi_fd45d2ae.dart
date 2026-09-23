import 'dart:ffi';
void f(int i) => i * 2;
void g() {
  NativeCallable<void Function(int)>.listener(f);
//^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.mustBeANativeFunctionType] The type 'void Function(int)' given to 'NativeCallable' must be a valid 'dart:ffi' native function type.
}
