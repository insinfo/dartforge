import 'dart:ffi';
typedef T = Int8 Function(Int8);
int f(int i) => i * 2;
void g() {
  Pointer.fromFunction<T>(f, 42);
}
