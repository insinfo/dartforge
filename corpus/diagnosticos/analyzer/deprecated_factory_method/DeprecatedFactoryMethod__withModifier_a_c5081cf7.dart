// %before-language-feature: primary-constructors
class C {
  augment external factory();
//^^^^^^^
// [diag.augmentationWithoutDeclaration] The declaration being augmented doesn't exist.
//                 ^^^^^^^
// [diag.deprecatedFactoryMethod] Methods named 'factory' will become constructors when the primary_constructors feature is enabled.
}
