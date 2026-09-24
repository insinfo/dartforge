base class A {}
mixin B {}
augment mixin B implements A {}
//            ^
// [diag.mixinSubtypeOfBaseIsNotBase] The mixin 'B' must be 'base' because the supertype 'A' is 'base'.
