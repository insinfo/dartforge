// %before-language-feature: augmentations
extension type E(int i) {
  static set foo(int _);
//                     ^
// [diag.missingFunctionBody] A function body must be provided.
}
