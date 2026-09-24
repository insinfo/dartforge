class A {
  set _s(int x) {
//    ^^
// [diag.unusedElement] The declaration '_s' isn't referenced.
    if (x > 5) {
      _s = x - 1;
    }
  }
}
