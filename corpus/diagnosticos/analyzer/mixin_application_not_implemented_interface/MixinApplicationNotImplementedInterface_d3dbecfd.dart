class B with M {}
//           ^
// [diag.mixinApplicationNotImplementedInterface] 'M' can't be mixed onto 'Object' because 'Object' doesn't implement 'A'.
mixin M {}
class A {}
augment mixin M on A {}
//              ^^
// [diag.mixinAugmentationHasOnClause] Mixin augmentations can't have 'on' clauses.
