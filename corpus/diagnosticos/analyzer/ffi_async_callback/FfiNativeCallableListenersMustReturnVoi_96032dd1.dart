import 'dart:ffi';
void f(int i) => i * 2;
void g() {
  NativeCallable<Void Function(Int32)>.isolateLocal(f, 0);
//                                                     ^
// [diag.extraPositionalArgumentsCouldBeNamed] Too many positional arguments: 1 expected, but 2 found.
}
