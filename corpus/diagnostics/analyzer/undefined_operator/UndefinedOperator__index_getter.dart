class A {}

f(A a) {
  a[0];
// ^^^
// [diag.undefinedOperator] The operator '[]' isn't defined for the type 'A'.
}
