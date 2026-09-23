enum _E {
  v;
  static int get _foo => 0;
//               ^^^^
// [diag.unusedElement] The declaration '_foo' isn't referenced.
}

void f() {
  _E.v;
}
