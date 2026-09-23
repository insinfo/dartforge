class A {
  int _f;
//    ^^
// [diag.unusedField] The value of the field '_f' isn't used.
  A() : _f = 0;
}
