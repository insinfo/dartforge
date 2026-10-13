void nonPrefix() {}
f() {
  new nonPrefix.Class();
//    ^^^^^^^^^
// [diag.prefixShadowedByLocalDeclaration] The prefix 'nonPrefix' can't be used here because it's shadowed by a local declaration.
}
