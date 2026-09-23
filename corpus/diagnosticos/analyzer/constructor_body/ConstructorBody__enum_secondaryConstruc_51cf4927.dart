enum E {
  v;
  external const E() {}
//                   ^
// [diag.externalMethodWithBody] An external or native method can't have a body.
// [diag.constConstructorWithBody] Const constructors can't have a body.
}
