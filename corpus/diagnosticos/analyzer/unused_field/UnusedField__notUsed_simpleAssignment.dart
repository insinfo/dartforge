class A {
  int _f = 0;
//    ^^
// [diag.unusedField] The value of the field '_f' isn't used.
  m() {
    _f = 1;
  }
}
f(A a) {
  a._f = 2;
}
