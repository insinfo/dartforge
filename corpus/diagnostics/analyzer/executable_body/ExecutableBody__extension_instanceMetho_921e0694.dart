// %before-language-feature: augmentations
extension E on int {
  void foo();
//     ^^^
// [diag.extensionDeclaresAbstractMember] Extensions can't declare abstract members.
}
