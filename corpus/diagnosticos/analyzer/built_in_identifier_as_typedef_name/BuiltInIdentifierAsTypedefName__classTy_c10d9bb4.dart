class A {}
mixin B {}
class out = A with B;
//    ^^^
// [diag.builtInIdentifierAsTypedefName] The built-in identifier 'out' can't be used as a typedef name.
