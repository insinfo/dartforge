import 'dart:ffi';
typedef T = Void Function(Int8);
int f(int i) => i * 2;
void g() {
  Pointer.fromFunction<T>(f);
}
