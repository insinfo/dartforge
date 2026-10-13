void f(x, int a) {
  if (x case {a: 0}) {}
//            ^
// [diag.nonConstantMapPatternKey] Key expressions in map patterns must be constants.
}
