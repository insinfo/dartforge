class _A {
//    ^^
// [diag.unusedElement] The declaration '_A' isn't referenced.
  static staticMethod() {
//       ^^^^^^^^^^^^
// [diag.unusedElement] The declaration 'staticMethod' isn't referenced.
    new _A();
  }
  instanceMethod() {
    new _A();
  }
}
