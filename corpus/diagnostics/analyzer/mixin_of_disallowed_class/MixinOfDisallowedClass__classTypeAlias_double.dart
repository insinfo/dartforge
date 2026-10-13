class A {}
class C = A with double;
//               ^^^^^^
// [diag.mixinOfDisallowedClass] Classes can't mixin 'double'.
