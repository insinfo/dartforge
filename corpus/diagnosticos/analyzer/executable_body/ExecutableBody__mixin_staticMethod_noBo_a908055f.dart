// %before-language-feature: augmentations
mixin M {
  static void foo();
//                 ^
// [diag.missingFunctionBody] A function body must be provided.
}
