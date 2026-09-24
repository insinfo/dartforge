class C extends B {}
//    ^
// [diag.subtypeOfFinalIsNotBaseFinalOrSealed][context 1] The type 'C' must be 'base', 'final' or 'sealed' because the supertype 'A' is 'final'.
sealed class B extends A {}
final class A {}
//          ^
// [context 1] The type 'B' is a subtype of 'A', and 'A' is defined here.
