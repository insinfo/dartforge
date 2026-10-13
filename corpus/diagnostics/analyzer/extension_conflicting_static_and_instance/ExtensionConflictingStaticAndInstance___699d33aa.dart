extension E on String {
  static int get foo => 0;
//               ^^^
// [diag.extensionConflictingStaticAndInstance] An extension can't define static member 'foo' and an instance member with the same name.
  void foo() {}
}
