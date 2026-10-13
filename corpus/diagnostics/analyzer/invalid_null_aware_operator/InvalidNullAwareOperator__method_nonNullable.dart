f(int x) {
  x?.round();
// ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?.' is unnecessary.
  x?..round();
// ^^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?..' is unnecessary.
}
