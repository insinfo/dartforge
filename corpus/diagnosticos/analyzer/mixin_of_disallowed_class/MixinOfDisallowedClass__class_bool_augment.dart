class A {}

augment class A with bool {}
//                   ^^^^
// [diag.mixinOfDisallowedClass] Classes can't mixin 'bool'.
