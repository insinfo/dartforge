// %before-language-feature: augmentations
enum E {
  v;
  static void foo();
//                 ^
// [diag.missingFunctionBody] A function body must be provided.
}
