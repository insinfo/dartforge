f() {
  int? i;
  i != null;
//^^^^
// [diag.unnecessaryNullComparisonAlwaysNullFalse] The operand must be 'null', so the condition is always 'false'.
  null != i;
//     ^^^^
// [diag.unnecessaryNullComparisonAlwaysNullFalse] The operand must be 'null', so the condition is always 'false'.
}
