enum _E {
  v;
  static set _foo(int _) {}
//           ^^^^
// [diag.unusedElement] The declaration '_foo' isn't referenced.
}

void f() {
  _E.v;
}
