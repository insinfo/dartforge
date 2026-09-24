// %before-language-feature: augmentations
extension type E(int i) {
  static void foo();
//                 ^
// [diag.missingFunctionBody] A function body must be provided.
}
