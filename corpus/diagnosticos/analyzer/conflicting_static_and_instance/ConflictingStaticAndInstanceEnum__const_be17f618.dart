enum E {
  a, noSuchMethod, b
//   ^^^^^^^^^^^^
// [diag.conflictingStaticAndInstance] Class 'E' can't define static member 'noSuchMethod' and have instance member 'E.noSuchMethod' with the same name.
}
