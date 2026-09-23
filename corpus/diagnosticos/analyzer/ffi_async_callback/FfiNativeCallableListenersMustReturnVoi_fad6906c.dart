import 'dart:ffi';
int f(int i) => i * 2;
void g() {
  NativeCallable<Int32 Function(Int32)>.isolateLocal(f);
//^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.missingExceptionValue] The method isolateLocal must have an exceptional return value (the second argument) when the return type of the function is neither 'void', 'Handle', nor 'Pointer'.
}
