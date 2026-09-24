enum E {
  v1, v2.named();
  const E();
  const E.named() : this() => null;
//                         ^^
// [diag.redirectingConstructorWithBody] Redirecting constructors can't have a body.
// [diag.constConstructorWithBody] Const constructors can't have a body.
//                         ^^^^^^^^
// [diag.returnInGenerativeConstructor] Constructors can't return values.
//                            ^^^^
// [diag.returnOfInvalidTypeFromConstructor] A value of type 'Null' can't be returned from the constructor 'E.named' because it has a return type of 'E'.
}
