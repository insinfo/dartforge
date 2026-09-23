class A {}
class C = A with String, num;
//               ^^^^^^
// [diag.mixinOfDisallowedClass] Classes can't mixin 'String'.
//                       ^^^
// [diag.mixinOfDisallowedClass] Classes can't mixin 'num'.
