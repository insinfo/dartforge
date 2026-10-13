final class A {}
//          ^
// [context 1] The type 'C' is a subtype of 'A', and 'A' is defined here.
sealed class B extends A {}
sealed class C extends B {}
class D extends C {}
//    ^
// [diag.subtypeOfFinalIsNotBaseFinalOrSealed][context 1] The type 'D' must be 'base', 'final' or 'sealed' because the supertype 'A' is 'final'.
