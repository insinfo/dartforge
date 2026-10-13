class A {}
class C = A with int;
//               ^^^
// [diag.mixinOfDisallowedClass] Classes can't mixin 'int'.
