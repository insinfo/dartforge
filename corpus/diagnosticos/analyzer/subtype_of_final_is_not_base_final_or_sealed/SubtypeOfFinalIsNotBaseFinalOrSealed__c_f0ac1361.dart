mixin A {}
final class B {}
//          ^
// [context 1] The type 'C' is a subtype of 'B', and 'B' is defined here.
sealed class C extends B with A {}
class D extends C {}
//    ^
// [diag.subtypeOfFinalIsNotBaseFinalOrSealed][context 1] The type 'D' must be 'base', 'final' or 'sealed' because the supertype 'B' is 'final'.
