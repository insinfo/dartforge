extension E on String {
  static void foo() {}
//            ^^^
// [diag.extensionConflictingStaticAndInstance] An extension can't define static member 'foo' and an instance member with the same name.
  void foo() {}
}
