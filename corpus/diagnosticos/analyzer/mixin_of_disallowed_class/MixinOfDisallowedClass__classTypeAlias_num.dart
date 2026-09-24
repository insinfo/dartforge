class A {}
class C = A with num;
//               ^^^
// [diag.mixinOfDisallowedClass] Classes can't mixin 'num'.
