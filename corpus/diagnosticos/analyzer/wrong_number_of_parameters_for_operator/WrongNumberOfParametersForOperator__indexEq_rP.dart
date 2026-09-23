class A {
  operator []=(a) {}
//         ^^^
// [diag.wrongNumberOfParametersForOperator] Operator '[]=' should declare exactly 2 parameters, but 1 found.
}
