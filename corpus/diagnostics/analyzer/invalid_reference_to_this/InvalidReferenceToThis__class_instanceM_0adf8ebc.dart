class A {
  void foo([Object p = this]) {}
//                     ^^^^
// [diag.invalidReferenceToThis] Invalid reference to 'this' expression.
}
