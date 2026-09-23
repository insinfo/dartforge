extension type const E(int it) {
  const E.named() : this(0) {}
//                          ^
// [diag.redirectingConstructorWithBody] Redirecting constructors can't have a body.
// [diag.constConstructorWithBody] Const constructors can't have a body.
}
