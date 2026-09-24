class A {
  void f({int? _p}) {}
//             ^^
// [diag.privateNamedNonFieldParameter] Named parameters that don't refer to instance variables can't start with underscore.
}
