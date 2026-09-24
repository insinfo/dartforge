class A {}
class C = A with Null;
//               ^^^^
// [diag.mixinOfDisallowedClass] Classes can't mixin 'Null'.
