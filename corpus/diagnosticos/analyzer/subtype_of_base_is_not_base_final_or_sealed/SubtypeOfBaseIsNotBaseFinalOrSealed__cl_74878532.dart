base class A {}
mixin B {}
interface class C = Object with B implements A;
//              ^
// [diag.subtypeOfBaseIsNotBaseFinalOrSealed] The type 'C' must be 'base', 'final' or 'sealed' because the supertype 'A' is 'base'.
