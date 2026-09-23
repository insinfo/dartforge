base mixin class A {}
class B with A {}
//    ^
// [diag.subtypeOfBaseIsNotBaseFinalOrSealed] The type 'B' must be 'base', 'final' or 'sealed' because the supertype 'A' is 'base'.
