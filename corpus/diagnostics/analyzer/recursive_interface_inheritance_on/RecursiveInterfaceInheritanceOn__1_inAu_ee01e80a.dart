mixin A {}
augment mixin A on A {}
//              ^^
// [diag.mixinAugmentationHasOnClause] Mixin augmentations can't have 'on' clauses.
//                 ^
// [diag.recursiveInterfaceInheritanceOn] 'A' can't use itself as a superclass constraint.
