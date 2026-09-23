import 'dart:ffi';
int f(int i) => i * 2;
void g() {
  NativeCallable<Int32 Function(Int32)>.listener(f);
//                                               ^
// [diag.mustReturnVoid] The return type of the function passed to 'NativeCallable.listener' must be 'void' rather than 'Int32'.
}
