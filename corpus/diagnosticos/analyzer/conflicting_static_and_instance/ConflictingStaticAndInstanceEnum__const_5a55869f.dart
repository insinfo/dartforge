enum E {
  a, runtimeType, b
//   ^^^^^^^^^^^
// [diag.conflictingStaticAndInstance] Class 'E' can't define static member 'runtimeType' and have instance member 'E.runtimeType' with the same name.
}
