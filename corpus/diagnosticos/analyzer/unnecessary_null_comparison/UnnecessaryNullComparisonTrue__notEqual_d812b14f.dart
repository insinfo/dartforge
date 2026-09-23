f(int a) {
  a != null;
//  ^^^^^^^
// [diag.unnecessaryNullComparisonNeverNullTrue] The operand can't be 'null', so the condition is always 'true'.
  null != a;
//^^^^^^^
// [diag.unnecessaryNullComparisonNeverNullTrue] The operand can't be 'null', so the condition is always 'true'.
}
