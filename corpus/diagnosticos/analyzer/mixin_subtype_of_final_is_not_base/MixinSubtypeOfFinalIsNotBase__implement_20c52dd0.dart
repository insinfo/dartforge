final class A {}
//          ^
// [context 1] The type 'B' is a subtype of 'A', and 'A' is defined here.
sealed class B implements A {}
mixin C implements B {}
//    ^
// [diag.mixinSubtypeOfFinalIsNotBase][context 1] The mixin 'C' must be 'base' because the supertype 'A' is 'final'.
