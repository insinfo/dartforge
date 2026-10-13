enum _E {
  v;
  static set foo(int _) {}
//           ^^^
// [diag.unusedElement] The declaration 'foo' isn't referenced.
}

void f() {
  _E.v;
}
