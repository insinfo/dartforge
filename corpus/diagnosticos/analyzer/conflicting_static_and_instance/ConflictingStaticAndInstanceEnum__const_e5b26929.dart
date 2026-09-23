enum E {
  a, toString, b
//   ^^^^^^^^
// [diag.conflictingStaticAndInstance] Class 'E' can't define static member 'toString' and have instance member 'E.toString' with the same name.
}
