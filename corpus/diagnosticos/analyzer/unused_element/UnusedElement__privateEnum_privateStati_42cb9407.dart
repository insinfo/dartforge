enum _E {
  v;
  static void _foo() {}
//            ^^^^
// [diag.unusedElement] The declaration '_foo' isn't referenced.
}

void f() {
  _E.v;
}
