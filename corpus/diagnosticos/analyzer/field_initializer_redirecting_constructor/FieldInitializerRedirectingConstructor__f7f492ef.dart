enum E {
  v(0);
  final int x;
  const E.named() : x = 0;
  const E(this.x) : this.named();
//        ^^^^^^
// [diag.fieldInitializerRedirectingConstructor] The redirecting constructor can't have a field initializer.
}
