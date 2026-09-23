enum E {
  v;
  static void _foo() {}
//            ^^^^
// [diag.unusedElement] The declaration '_foo' isn't referenced.
}
