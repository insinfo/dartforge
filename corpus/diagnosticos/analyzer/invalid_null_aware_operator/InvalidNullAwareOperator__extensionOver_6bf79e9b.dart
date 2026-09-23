extension E on int {
  operator[]=(int index, bool _) {}
}

void f(int? a, int b) {
  E(a)?[0] = true;
  E(b)?[0] = true;
//    ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?[' is unnecessary.
}
