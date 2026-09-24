enum E {
  v;
  final int x;
  const E.named() : x = 0;
  const E() : x = 42, this.named();
//            ^^^^^^
// [diag.fieldInitializerRedirectingConstructor] The redirecting constructor can't have a field initializer.
}
