enum E {
  v;
  static int hashCode() => 0;
//           ^^^^^^^^
// [diag.conflictingStaticAndInstance] Class 'E' can't define static member 'hashCode' and have instance member 'E.hashCode' with the same name.
}
