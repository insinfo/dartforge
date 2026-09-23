extension E on int {
  bool get foo => true;
}

void f(int? a, int b) {
  E(a)?.foo;
  E(b)?.foo;
//    ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?.' is unnecessary.
}
