final class A {}
class B implements A {}
//    ^
// [diag.subtypeOfFinalIsNotBaseFinalOrSealed] The type 'B' must be 'base', 'final' or 'sealed' because the supertype 'A' is 'final'.
