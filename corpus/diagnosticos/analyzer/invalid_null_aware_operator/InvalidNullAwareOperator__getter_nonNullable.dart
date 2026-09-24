f(int x) {
  x?.isEven;
// ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?.' is unnecessary.
  x?..isEven;
// ^^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?..' is unnecessary.
}
