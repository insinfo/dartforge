enum _E {
  v;
  static final int foo = 0;
//                 ^^^
// [diag.unusedField] The value of the field 'foo' isn't used.
}

void f() {
  _E.v;
}
