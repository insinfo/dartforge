void f(List<double> list) {
  switch (list) {
    case [double.nan]:
//        ^^^^^^^^^^
// [diag.unnecessaryNanComparisonFalse] A double can't equal 'double.nan', so the condition is always 'false'.
  }
}
