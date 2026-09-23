class A {}
f(a) {
  if (a is A) {
    a++;
//   ^^
// [diag.undefinedOperator] The operator '+' isn't defined for the type 'A'.
  }
}
