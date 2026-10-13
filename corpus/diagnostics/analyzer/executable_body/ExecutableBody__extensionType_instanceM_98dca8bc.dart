// %before-language-feature: augmentations
extension type E(int i) {
  void foo();
//^^^^^^^^^^^
// [diag.extensionTypeWithAbstractMember] 'foo' must have a method body because 'E' is an extension type.
}
