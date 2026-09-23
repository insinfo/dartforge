mixin class A {}
mixin class B {}
mixin class C = Object with A, B;
//                     ^^^^^^^^^
// [diag.mixinModifierMixinApplicationClassWithMultipleMixins] The mixin application class 'C' can only have a single mixin.
