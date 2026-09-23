class A {
  operator ~(a, {b}) {}
//         ^
// [diag.wrongNumberOfParametersForOperator] Operator '~' should declare exactly 0 parameters, but 2 found.
}
