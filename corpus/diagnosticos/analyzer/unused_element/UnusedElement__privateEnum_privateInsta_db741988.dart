enum _E {
  v;
  set _foo(int _) {}
//    ^^^^
// [diag.unusedElement] The declaration '_foo' isn't referenced.
}

void f() {
  _E.v;
}
