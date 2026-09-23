class A {
  int _f = 0;
//    ^^
// [diag.unusedField] The value of the field '_f' isn't used.
  main() {
    _f++;
  }
}
