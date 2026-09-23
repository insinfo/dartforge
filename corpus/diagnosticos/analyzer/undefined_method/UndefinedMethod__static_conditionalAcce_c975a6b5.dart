class A {
  static void m() {}
}
f() { A?.m(); }
//     ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?.' is unnecessary.
