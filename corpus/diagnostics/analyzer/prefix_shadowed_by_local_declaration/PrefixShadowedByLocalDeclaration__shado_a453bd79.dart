import 'dart:core' as core;
class A {
  core.List foo = 0;
//^^^^
// [diag.prefixShadowedByLocalDeclaration] The prefix 'core' can't be used here because it's shadowed by a local declaration.
  get core => 0;
}
