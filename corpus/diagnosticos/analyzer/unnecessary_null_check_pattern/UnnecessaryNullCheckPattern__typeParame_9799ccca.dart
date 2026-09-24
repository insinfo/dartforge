class A<T extends num> {
  void f(T x) {
    if (x case var a?) {}
//                 ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
//                  ^
// [diag.unnecessaryNullCheckPattern] The null-check pattern will have no effect because the matched type isn't nullable.
  }
}
