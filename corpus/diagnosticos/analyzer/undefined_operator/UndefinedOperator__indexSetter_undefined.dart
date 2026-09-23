class A {}
f(A a) {
  a[0] = 1;
// ^^^
// [diag.undefinedOperator] The operator '[]=' isn't defined for the type 'A'.
}
