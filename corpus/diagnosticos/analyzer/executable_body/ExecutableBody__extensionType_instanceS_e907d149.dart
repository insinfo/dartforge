extension type E(int i) {
  set foo(int _);
//^^^^^^^^^^^^^^^
// [diag.extensionTypeWithAbstractMember] 'foo' must have a method body because 'E' is an extension type.
}
