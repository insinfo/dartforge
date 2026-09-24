import 'dart:ffi';
void f(int i) => i * 2;
void g() {
  NativeCallable<Void Function(Int32)>? callback;
  callback = NativeCallable.listener(f);
  callback.close();
}
