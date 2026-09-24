mixin M {
  static String toString() => 'x';
//              ^^^^^^^^
// [diag.conflictingStaticAndInstance] Class 'M' can't define static member 'toString' and have instance member 'Object.toString' with the same name.
}
