class C {
  const C();
  const C.named() : this() {}
//                         ^
// [diag.redirectingConstructorWithBody] Redirecting constructors can't have a body.
// [diag.constConstructorWithBody] Const constructors can't have a body.
}
