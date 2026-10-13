class A {}
mixin M {}
class C = A with M;
//    ^
// [context 1] The declaration being augmented.
augment class C {}
// [diag.augmentationOfMixinApplicationClass][column 1][length 7][context 1] Mixin application classes can't be augmented.
