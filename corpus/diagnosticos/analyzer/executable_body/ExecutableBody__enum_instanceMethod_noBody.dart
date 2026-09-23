enum E {
  v;
  void foo();
//^^^^^^^^^^^
// [diag.enumWithAbstractMember] 'foo' must have a method body because 'E' is an enum.
}
