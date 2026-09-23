class A {}
class C = A with String;
//               ^^^^^^
// [diag.mixinOfDisallowedClass] Classes can't mixin 'String'.
