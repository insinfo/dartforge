import 'dart:ffi';
typedef T = Int8 Function(Int8);
int f(int i) => i * 2;
void g() {
  Pointer.fromFunction<T>(f);
//        ^^^^^^^^^^^^
// [diag.missingExceptionValue] The method fromFunction must have an exceptional return value (the second argument) when the return type of the function is neither 'void', 'Handle', nor 'Pointer'.
}
