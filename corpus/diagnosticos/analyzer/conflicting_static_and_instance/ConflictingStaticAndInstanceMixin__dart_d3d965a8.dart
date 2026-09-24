mixin M on Enum {
  static set index(int _) {}
//           ^^^^^
// [diag.conflictingStaticAndInstance] Class 'M' can't define static member 'index' and have instance member 'Enum.index' with the same name.
}
