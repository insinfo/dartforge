base class A {}
//         ^
// [context 1] The type 'C' is a subtype of 'A', and 'A' is defined here.
interface class B {}
sealed class C extends B implements A {}
class D extends C {}
//    ^
// [diag.subtypeOfBaseIsNotBaseFinalOrSealed][context 1] The type 'D' must be 'base', 'final' or 'sealed' because the supertype 'A' is 'base'.
