f(int x) {
  while (true) {
    break x;
//        ^
// [diag.labelUndefined] Can't reference an undefined label 'x'.
  }
}
