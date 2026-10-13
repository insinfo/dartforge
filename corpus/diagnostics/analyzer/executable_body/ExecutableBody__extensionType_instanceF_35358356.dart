// %before-language-feature: augmentations
extension type E(int i) {
  abstract int foo;
//             ^^^
// [diag.extensionTypeDeclaresInstanceField] Extension types can't declare instance fields.
}
