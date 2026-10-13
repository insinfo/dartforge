import 'dart:ffi';
int f(int i) => i * 2;
void g() {
  NativeCallable<Int32 Function(Int32)>.isolateLocal(f, 0);
//                                                      ^
// [diag.extraPositionalArgumentsCouldBeNamed] Too many positional arguments: 1 expected, but 2 found.
}
