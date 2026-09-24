mixin M on Enum {
  static int index() => 0;
//           ^^^^^
// [diag.conflictingStaticAndInstance] Class 'M' can't define static member 'index' and have instance member 'Enum.index' with the same name.
}
