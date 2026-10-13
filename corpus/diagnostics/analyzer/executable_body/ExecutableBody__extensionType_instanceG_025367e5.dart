extension type E(int i) {
  int get foo;
//^^^^^^^^^^^^
// [diag.extensionTypeWithAbstractMember] 'foo' must have a method body because 'E' is an extension type.
}
