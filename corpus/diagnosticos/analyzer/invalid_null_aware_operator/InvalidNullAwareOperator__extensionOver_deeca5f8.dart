extension E on int {
  set foo(bool _) {}
}

void f(int? a, int b) {
  E(a)?.foo = true;
  E(b)?.foo = true;
//    ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?.' is unnecessary.
}
