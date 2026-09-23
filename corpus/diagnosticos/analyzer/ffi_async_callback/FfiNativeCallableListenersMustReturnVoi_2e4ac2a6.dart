import 'dart:ffi';
int f(int i) => i * 2;
void g() {
  NativeCallable<int Function(int)>.isolateLocal(f, exceptionalReturn: 4);
//^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.mustBeANativeFunctionType] The type 'int Function(int)' given to 'NativeCallable' must be a valid 'dart:ffi' native function type.
}
