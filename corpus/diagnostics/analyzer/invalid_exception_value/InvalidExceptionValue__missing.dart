import 'dart:ffi';
typedef T = Void Function(Int8);
void f(int i) {}
void g() {
  Pointer.fromFunction<T>(f, 42);
//                           ^^
// [diag.invalidExceptionValue] The method fromFunction can't have an exceptional return value (the second argument) when the return type of the function is either 'void', 'Handle' or 'Pointer'.
}
