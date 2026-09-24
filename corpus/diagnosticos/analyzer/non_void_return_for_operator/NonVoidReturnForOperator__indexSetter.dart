class A {
  int operator []=(a, b) { return a; }
//^^^
// [diag.nonVoidReturnForOperator] The return type of the operator []= must be 'void'.
}
