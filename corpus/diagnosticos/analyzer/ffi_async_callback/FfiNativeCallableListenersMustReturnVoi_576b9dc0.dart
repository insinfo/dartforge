import 'dart:ffi';
int f(int i) => i * 2;
void g() {
  NativeCallable.isolateLocal(f, exceptionalReturn: 4);
//^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.mustBeANativeFunctionType] The type 'Function' given to 'NativeCallable' must be a valid 'dart:ffi' native function type.
}
