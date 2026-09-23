class A {
  operator >() {}
//         ^
// [diag.wrongNumberOfParametersForOperator] Operator '>' should declare exactly 1 parameters, but 0 found.
}
