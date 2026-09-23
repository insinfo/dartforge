void nonPrefix() {}
f() {
  new nonPrefix.Class<int>.named();
//    ^^^^^^^^^
// [diag.prefixShadowedByLocalDeclaration] The prefix 'nonPrefix' can't be used here because it's shadowed by a local declaration.
}
