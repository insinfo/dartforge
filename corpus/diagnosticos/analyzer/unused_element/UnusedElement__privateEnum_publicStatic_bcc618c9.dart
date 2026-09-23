enum _E {
  v;
  static void foo() {}
//            ^^^
// [diag.unusedElement] The declaration 'foo' isn't referenced.
}

void f() {
  _E.v;
}
