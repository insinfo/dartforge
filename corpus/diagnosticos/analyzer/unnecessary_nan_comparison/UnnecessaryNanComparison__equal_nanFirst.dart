void f(double d) {
  double.nan == d;
//^^^^^^^^^^^^^
// [diag.unnecessaryNanComparisonFalse] A double can't equal 'double.nan', so the condition is always 'false'.
}
