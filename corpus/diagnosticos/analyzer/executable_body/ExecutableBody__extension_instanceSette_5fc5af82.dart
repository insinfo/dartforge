// %before-language-feature: augmentations
extension E on int {
  set foo(int _);
//    ^^^
// [diag.extensionDeclaresAbstractMember] Extensions can't declare abstract members.
}
