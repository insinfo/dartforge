class C {
  void m(void Function(covariant int) p) {}
//                     ^^^^^^^^^
// [diag.invalidUseOfCovariant] The 'covariant' keyword can only be used for parameters in instance methods or before non-final instance fields.
}
