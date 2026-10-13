mixin M {
  static void foo() {}
}

f() {
  M?.foo();
// ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?.' is unnecessary.
}
