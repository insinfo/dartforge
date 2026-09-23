mixin M {}
f(M m) {
  m..abs();
//   ^^^
// [diag.undefinedMethod] The method 'abs' isn't defined for the type 'M'.
}
