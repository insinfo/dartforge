void NonType() {}
f() {
  new NonType.named();
//    ^^^^^^^
// [diag.prefixShadowedByLocalDeclaration] The prefix 'NonType' can't be used here because it's shadowed by a local declaration.
}
