class A {
  int x = 0;
  A.named() {}
  A(this.x) : this.named();
//  ^^^^^^
// [diag.fieldInitializerRedirectingConstructor] The redirecting constructor can't have a field initializer.
}
