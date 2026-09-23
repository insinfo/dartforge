class A {}
mixin M on A {}
augment mixin M on A {}
//              ^^
// [diag.mixinAugmentationHasOnClause] Mixin augmentations can't have 'on' clauses.
//                 ^
// [diag.onRepeated] The type 'A' can be included in the superclass constraints only once.
