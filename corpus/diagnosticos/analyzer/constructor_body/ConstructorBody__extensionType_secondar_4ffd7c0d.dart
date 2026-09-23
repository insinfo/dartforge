extension type const E(int it) {
  external const E.named() {}
//                         ^
// [diag.externalMethodWithBody] An external or native method can't have a body.
// [diag.constConstructorWithBody] Const constructors can't have a body.
}
