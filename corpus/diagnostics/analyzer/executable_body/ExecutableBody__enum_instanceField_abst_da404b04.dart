// %before-language-feature: augmentations
enum E {
  v;
  abstract int foo;
//             ^^^
// [diag.nonFinalFieldInEnum] Enums can only declare final fields.
}
