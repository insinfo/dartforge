class A<T> {
  void f(T x) {
    if (x case var a?) {}
//                 ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
  }
}
