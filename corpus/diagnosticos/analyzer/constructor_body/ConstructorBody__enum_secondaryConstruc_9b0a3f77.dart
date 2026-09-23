enum E {
  v;
  const E() {}
//          ^
// [diag.constConstructorWithBody] Const constructors can't have a body.
}
