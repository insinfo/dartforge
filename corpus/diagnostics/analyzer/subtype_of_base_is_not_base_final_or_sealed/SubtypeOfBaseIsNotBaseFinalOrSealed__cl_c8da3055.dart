base class A {}
base class B extends A {}
class C extends A {}
//    ^
// [diag.subtypeOfBaseIsNotBaseFinalOrSealed] The type 'C' must be 'base', 'final' or 'sealed' because the supertype 'A' is 'base'.
