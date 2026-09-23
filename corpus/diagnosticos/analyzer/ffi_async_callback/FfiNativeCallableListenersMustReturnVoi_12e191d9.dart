import 'dart:ffi';
int f(int i) => i * 2;
void g() {
  NativeCallable<Void Function(Int32)>.isolateLocal(f);
}
