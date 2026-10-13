// %before-language-feature: augmentations
mixin M {
  static set foo(int _);
//                     ^
// [diag.missingFunctionBody] A function body must be provided.
}
