enum E {
  a, index, b
//   ^^^^^
// [diag.conflictingStaticAndInstance] Class 'E' can't define static member 'index' and have instance member 'E.index' with the same name.
}
