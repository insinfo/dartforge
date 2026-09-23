import 'dart:ffi';
int f(int i) => i * 2;
void g() {
  int e = 123;
  NativeCallable<Int32 Function(Int32)>.isolateLocal(exceptionalReturn: e, f);
//                                                                      ^
// [diag.argumentMustBeAConstant] Argument 'exceptionalReturn' must be a constant.
}
