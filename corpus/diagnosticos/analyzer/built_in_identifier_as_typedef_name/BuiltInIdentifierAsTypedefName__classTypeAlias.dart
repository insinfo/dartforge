class A {}
mixin B {}
class as = A with B;
//    ^^
// [diag.builtInIdentifierAsTypedefName] The built-in identifier 'as' can't be used as a typedef name.
