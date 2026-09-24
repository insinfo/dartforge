class A {
  operator []=(a, b, [c]) {}
//         ^^^
// [diag.wrongNumberOfParametersForOperator] Operator '[]=' should declare exactly 2 parameters, but 3 found.
}
