class C {
  int x = 0;

  void f() {
    this?.x;
//      ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?.' is unnecessary.
  }
}
