class A {
  int foo() => 0;
//    ^^^
// [context 1] The member being overridden.
}

mixin M {
  String foo() => '';
//       ^^^
// [diag.invalidOverride][context 1] 'M.foo' ('String Function()') isn't a valid override of 'A.foo' ('int Function()').
}

augment mixin M on A {}
//              ^^
// [diag.mixinAugmentationHasOnClause] Mixin augmentations can't have 'on' clauses.
