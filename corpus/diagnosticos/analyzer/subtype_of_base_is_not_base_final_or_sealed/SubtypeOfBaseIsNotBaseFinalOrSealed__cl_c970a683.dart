base class A {}
//         ^
// [context 1] The type 'AA' is a subtype of 'A', and 'A' is defined here.
sealed class AA extends A {}
mixin B {}
class C = Object with B implements AA;
//    ^
// [diag.subtypeOfBaseIsNotBaseFinalOrSealed][context 1] The type 'C' must be 'base', 'final' or 'sealed' because the supertype 'A' is 'base'.
