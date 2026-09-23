extension type E(int it) {
  external E.named() {}
//                   ^
// [diag.externalMethodWithBody] An external or native method can't have a body.
}
