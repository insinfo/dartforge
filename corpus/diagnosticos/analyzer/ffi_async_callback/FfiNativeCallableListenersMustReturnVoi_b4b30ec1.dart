import 'dart:ffi';
int f(int i) => i * 2;
void g() {
  NativeCallable<Int32 Function(Int32)>? callback;
  callback = NativeCallable.isolateLocal(f, exceptionalReturn: 4);
  callback.close();
}
