m() {
  Null x;
  x == null;
//^^^^
// [diag.unnecessaryNullComparisonAlwaysNullTrue] The operand must be 'null', so the condition is always 'true'.
}
