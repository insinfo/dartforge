mixin M {}
f(M m) {
  m++;
// ^^
// [diag.undefinedOperator] The operator '+' isn't defined for the type 'M'.
}
