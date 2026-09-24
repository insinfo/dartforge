f() {
  int? i;
  i == null;
//^^^^
// [diag.unnecessaryNullComparisonAlwaysNullTrue] The operand must be 'null', so the condition is always 'true'.
  null == i;
//     ^^^^
// [diag.unnecessaryNullComparisonAlwaysNullTrue] The operand must be 'null', so the condition is always 'true'.
}
