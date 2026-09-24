class A {
  static var x;
}
f() { A?.x = 1; }
//     ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?.' is unnecessary.
