f() {
  x: while (true) {
//^^
// [diag.unusedLabel] The label 'x' isn't used.
    break y;
//        ^
// [diag.labelUndefined] Can't reference an undefined label 'y'.
  }
}
