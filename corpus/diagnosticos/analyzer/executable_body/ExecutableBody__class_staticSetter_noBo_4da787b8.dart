// %before-language-feature: augmentations
class A {
  static set foo(int _);
//                     ^
// [diag.missingFunctionBody] A function body must be provided.
}
