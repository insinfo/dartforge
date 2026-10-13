enum _E {
  v;
  static int get foo => 0;
//               ^^^
// [diag.unusedElement] The declaration 'foo' isn't referenced.
}

void f() {
  _E.v;
}
