class C {
  C? m1() => this;
  C m2() => this;
  void m3() {
    m1()?.m2()?.m2();
//      ^^
// [context 1] The operator '?.' is causing the short circuiting.
//            ^^
// [diag.invalidNullAwareOperatorAfterShortCircuit][context 1] The receiver can't be 'null' because of short-circuiting, so the null-aware operator '?.' can't be used.
  }
}
