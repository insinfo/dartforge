class A {
  int x = 0;
  A.named() {}
  A() : this.named(), x = 42;
//                    ^^^^^^
// [diag.fieldInitializerRedirectingConstructor] The redirecting constructor can't have a field initializer.
}
