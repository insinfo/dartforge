mixin class A {}
mixin class B {}
mixin class C = Object with A, B;
//                     ^^^^^^^^^
// [diag.mixinModifierMixinApplicationClassWithMultipleMixins] The mixin application class 'C' can only have a single mixin.
class D extends Object with C {}
//                          ^
// [diag.mixinInheritsFromNotObject] The class 'C' can't be used as a mixin because it extends a class other than 'Object'.
