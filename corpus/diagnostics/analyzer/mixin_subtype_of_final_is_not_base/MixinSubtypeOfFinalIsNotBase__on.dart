final class A {}
mixin B on A {}
//    ^
// [diag.mixinSubtypeOfFinalIsNotBase] The mixin 'B' must be 'base' because the supertype 'A' is 'final'.
