enum E {
  v;
  set foo(int _);
//^^^^^^^^^^^^^^^
// [diag.enumWithAbstractMember] 'foo' must have a method body because 'E' is an enum.
}
