extension type const E(int it) {
  const E.named() : it = 0 => E(0);
//                         ^^
// [diag.constConstructorWithBody] Const constructors can't have a body.
//                         ^^^^^^^^
// [diag.returnInGenerativeConstructor] Constructors can't return values.
}
