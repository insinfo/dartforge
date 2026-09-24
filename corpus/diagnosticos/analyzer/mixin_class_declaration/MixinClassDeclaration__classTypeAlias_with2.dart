mixin M1 {}
mixin M2 {}
mixin class A = Object with M1, M2;
//                     ^^^^^^^^^^^
// [diag.mixinModifierMixinApplicationClassWithMultipleMixins] The mixin application class 'A' can only have a single mixin.
