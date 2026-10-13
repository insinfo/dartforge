extension E on String {
  static set foo(_) {}
//           ^^^
// [diag.extensionConflictingStaticAndInstance] An extension can't define static member 'foo' and an instance member with the same name.
  set foo(_) {}
}
