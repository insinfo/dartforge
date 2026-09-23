base class A {}
mixin B on A {}
//    ^
// [diag.mixinSubtypeOfBaseIsNotBase] The mixin 'B' must be 'base' because the supertype 'A' is 'base'.
