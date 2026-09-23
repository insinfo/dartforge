enum E {
  v;
  int get foo;
//^^^^^^^^^^^^
// [diag.enumWithAbstractMember] 'foo' must have a method body because 'E' is an enum.
}
