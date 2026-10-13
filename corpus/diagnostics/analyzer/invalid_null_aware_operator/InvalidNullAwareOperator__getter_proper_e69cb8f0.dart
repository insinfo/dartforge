class C {
  int x = 0;
}

void f() {
  new C()?.x;
//       ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?.' is unnecessary.
}
