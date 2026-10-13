// %before-language-feature: augmentations
extension E on int {
  abstract int foo;
//             ^^^
// [diag.extensionDeclaresInstanceField] Extensions can't declare instance fields.
}
