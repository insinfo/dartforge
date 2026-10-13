class A {}
mixin B {}
class inout = A with B;
//    ^^^^^
// [diag.builtInIdentifierAsTypedefName] The built-in identifier 'inout' can't be used as a typedef name.
