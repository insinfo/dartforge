f(List<int> x) {
  x?[0];
// ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?[' is unnecessary.
  x?..[0];
// ^^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?..' is unnecessary.
}
