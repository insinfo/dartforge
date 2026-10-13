mixin M {}
f(M m) {
  m[0];
// ^^^
// [diag.undefinedOperator] The operator '[]' isn't defined for the type 'M'.
}
