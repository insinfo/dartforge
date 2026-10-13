mixin M {}
f(M m) {
  -m;
//^
// [diag.undefinedOperator] The operator 'unary-' isn't defined for the type 'M'.
}
