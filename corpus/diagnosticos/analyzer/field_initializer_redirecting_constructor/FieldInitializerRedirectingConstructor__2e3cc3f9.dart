enum E {
  v;
  final int x;
  const E.named() : x = 0;
  const E() : this.named(), x = 42;
//                          ^^^^^^
// [diag.fieldInitializerRedirectingConstructor] The redirecting constructor can't have a field initializer.
}
