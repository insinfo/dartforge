import 'dart:ffi';
void f(int i) => i * 2;
void g() {
  NativeCallable<Void Function(Int32)>.isolateLocal(exceptionalReturn: 4, f);
//                                                  ^^^^^^^^^^^^^^^^^^^^
// [diag.invalidExceptionValue] The method isolateLocal can't have an exceptional return value (the second argument) when the return type of the function is either 'void', 'Handle' or 'Pointer'.
}
