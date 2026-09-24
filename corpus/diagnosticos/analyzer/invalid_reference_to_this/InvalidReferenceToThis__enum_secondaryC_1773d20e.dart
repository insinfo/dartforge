enum E {
  v.named();
  const E.named() {
//                ^
// [diag.constConstructorWithBody] Const constructors can't have a body.
    this;
  }
}
