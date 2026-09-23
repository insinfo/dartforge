enum E {
  v1, v2.named();
  const E();
  const E.named() : this() {}
//                         ^
// [diag.redirectingConstructorWithBody] Redirecting constructors can't have a body.
// [diag.constConstructorWithBody] Const constructors can't have a body.
}
