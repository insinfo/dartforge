// %before-language-feature: augmentations
extension E on int {
  static void foo();
//                 ^
// [diag.missingFunctionBody] A function body must be provided.
}
