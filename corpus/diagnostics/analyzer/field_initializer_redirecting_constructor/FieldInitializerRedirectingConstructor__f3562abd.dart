class A {
  int x = 0;
  A.named() {}
  A() : x = 42, this.named();
//      ^^^^^^
// [diag.fieldInitializerRedirectingConstructor] The redirecting constructor can't have a field initializer.
}
