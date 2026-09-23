// %before-language-feature: augmentations
class A {
  static int get foo;
//                  ^
// [diag.missingFunctionBody] A function body must be provided.
}
