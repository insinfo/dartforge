class A {
  operator ~(a) {}
//         ^
// [diag.wrongNumberOfParametersForOperator] Operator '~' should declare exactly 0 parameters, but 1 found.
}
