f(int x) {
  while (true) {
    continue x;
//           ^
// [diag.labelUndefined] Can't reference an undefined label 'x'.
  }
}
