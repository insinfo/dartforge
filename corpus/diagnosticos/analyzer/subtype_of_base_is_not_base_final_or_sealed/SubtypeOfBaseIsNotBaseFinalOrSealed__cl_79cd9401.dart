base class A {}
//         ^
// [context 1] The type 'B' is a subtype of 'A', and 'A' is defined here.
sealed class B extends A {}
class C extends B {}
//    ^
// [diag.subtypeOfBaseIsNotBaseFinalOrSealed][context 1] The type 'C' must be 'base', 'final' or 'sealed' because the supertype 'A' is 'base'.
