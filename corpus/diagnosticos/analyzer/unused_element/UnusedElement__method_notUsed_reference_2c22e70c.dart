class A {
  static _m(int p) {
//       ^^
// [diag.unusedElement] The declaration '_m' isn't referenced.
    _m(p - 1);
  }
}
