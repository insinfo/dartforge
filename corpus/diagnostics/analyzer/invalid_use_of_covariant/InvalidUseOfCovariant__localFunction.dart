void foo() {
  void f(covariant int x) {}
//     ^
// [diag.unusedElement] The declaration 'f' isn't referenced.
//       ^^^^^^^^^
// [diag.invalidUseOfCovariant] The 'covariant' keyword can only be used for parameters in instance methods or before non-final instance fields.
}
