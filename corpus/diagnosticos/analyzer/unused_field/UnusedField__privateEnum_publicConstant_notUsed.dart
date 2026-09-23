enum _E {
  v;
//^
// [diag.unusedField] The value of the field 'v' isn't used.
}

void f() {
  _E;
}
