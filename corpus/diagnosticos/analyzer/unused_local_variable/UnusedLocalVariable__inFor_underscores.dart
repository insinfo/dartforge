f() {
  for (var _ in [1,2,3]) {
    for (var __ in [4,5,6]) {
//           ^^
// [diag.unusedLocalVariable] The value of the local variable '__' isn't used.
      // do something
    }
  }
}
