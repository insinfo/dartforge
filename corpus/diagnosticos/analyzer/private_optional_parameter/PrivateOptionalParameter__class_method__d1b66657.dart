class A {
  void f({int? _123}) {}
//             ^^^^
// [diag.privateNamedNonFieldParameter] Named parameters that don't refer to instance variables can't start with underscore.
}
