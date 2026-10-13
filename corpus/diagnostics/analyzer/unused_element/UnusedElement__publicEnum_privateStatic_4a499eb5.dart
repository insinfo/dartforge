enum E {
  v;
  static set _foo(int _) {}
//           ^^^^
// [diag.unusedElement] The declaration '_foo' isn't referenced.
}
