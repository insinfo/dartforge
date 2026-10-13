void f(double d) {
  d != double.nan;
//  ^^^^^^^^^^^^^
// [diag.unnecessaryNanComparisonTrue] A double can't equal 'double.nan', so the condition is always 'true'.
}
