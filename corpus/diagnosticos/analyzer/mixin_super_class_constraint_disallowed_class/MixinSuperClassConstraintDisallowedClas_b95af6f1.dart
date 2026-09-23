mixin A {}
augment mixin A on int {}
//              ^^
// [diag.mixinAugmentationHasOnClause] Mixin augmentations can't have 'on' clauses.
//                 ^^^
// [diag.mixinSuperClassConstraintDisallowedClass] 'int' can't be used as a superclass constraint.
