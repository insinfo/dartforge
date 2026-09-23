void f(double d) {
  double.nan != d;
//^^^^^^^^^^^^^
// [diag.unnecessaryNanComparisonTrue] A double can't equal 'double.nan', so the condition is always 'true'.
}
