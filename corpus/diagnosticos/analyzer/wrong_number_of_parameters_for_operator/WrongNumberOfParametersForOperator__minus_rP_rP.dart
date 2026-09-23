class A {
  operator -(a, b) {}
//         ^
// [diag.wrongNumberOfParametersForOperatorMinus] Operator '-' should declare 0 or 1 parameter, but 2 found.
}
