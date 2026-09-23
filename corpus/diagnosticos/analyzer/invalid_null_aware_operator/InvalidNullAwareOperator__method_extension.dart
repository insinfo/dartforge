extension E on int {
  static void foo() {}
}

f() {
  E?.foo();
// ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?.' is unnecessary.
}
