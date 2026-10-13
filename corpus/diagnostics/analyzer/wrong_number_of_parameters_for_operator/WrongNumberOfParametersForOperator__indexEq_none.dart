class A {
  operator []=() {}
//         ^^^
// [diag.wrongNumberOfParametersForOperator] Operator '[]=' should declare exactly 2 parameters, but 0 found.
}
