extension type const E(int it) {
  const E.named() : it = 0 {}
//                         ^
// [diag.constConstructorWithBody] Const constructors can't have a body.
}
