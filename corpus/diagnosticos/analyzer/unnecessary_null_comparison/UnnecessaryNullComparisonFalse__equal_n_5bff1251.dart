f(int a) {
  a == null;
//  ^^^^^^^
// [diag.unnecessaryNullComparisonNeverNullFalse] The operand can't be 'null', so the condition is always 'false'.
  null == a;
//^^^^^^^
// [diag.unnecessaryNullComparisonNeverNullFalse] The operand can't be 'null', so the condition is always 'false'.
}
