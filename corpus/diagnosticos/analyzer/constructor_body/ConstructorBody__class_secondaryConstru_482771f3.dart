class C {
  const C();
  const C.named() => C();
//                ^^
// [diag.constConstructorWithBody] Const constructors can't have a body.
//                ^^^^^^^
// [diag.returnInGenerativeConstructor] Constructors can't return values.
}
