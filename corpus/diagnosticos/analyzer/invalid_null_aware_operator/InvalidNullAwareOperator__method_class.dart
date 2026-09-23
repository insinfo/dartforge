class C {
  static void foo() {}
}

f() {
  C?.foo();
// ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?.' is unnecessary.
}
