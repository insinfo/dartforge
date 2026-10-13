main() {
  _f(int p) {
//^^
// [diag.unusedElement] The declaration '_f' isn't referenced.
    _f(p - 1);
  }
}
