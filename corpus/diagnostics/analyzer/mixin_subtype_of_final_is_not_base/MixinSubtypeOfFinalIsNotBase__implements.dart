final class A {}
mixin B implements A {}
//    ^
// [diag.mixinSubtypeOfFinalIsNotBase] The mixin 'B' must be 'base' because the supertype 'A' is 'final'.
