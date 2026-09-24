mixin M {
  static String runtimeType() => 'x';
//              ^^^^^^^^^^^
// [diag.conflictingStaticAndInstance] Class 'M' can't define static member 'runtimeType' and have instance member 'Object.runtimeType' with the same name.
}
