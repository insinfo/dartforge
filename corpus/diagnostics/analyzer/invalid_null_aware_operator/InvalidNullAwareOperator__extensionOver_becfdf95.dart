extension E on int {
  bool operator[](int index) => true;
}

void f(int? a, int b) {
  E(a)?[0];
  E(b)?[0];
//    ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?[' is unnecessary.
}
