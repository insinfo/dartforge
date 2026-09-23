class A {}
class C = A with bool;
//               ^^^^
// [diag.mixinOfDisallowedClass] Classes can't mixin 'bool'.
