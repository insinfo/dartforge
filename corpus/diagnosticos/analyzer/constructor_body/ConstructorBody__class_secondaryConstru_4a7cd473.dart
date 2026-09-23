class C {
  const C();
  const C.named() : this() => null;
//                         ^^
// [diag.redirectingConstructorWithBody] Redirecting constructors can't have a body.
// [diag.constConstructorWithBody] Const constructors can't have a body.
//                         ^^^^^^^^
// [diag.returnInGenerativeConstructor] Constructors can't return values.
//                            ^^^^
// [diag.returnOfInvalidTypeFromConstructor] A value of type 'Null' can't be returned from the constructor 'C.named' because it has a return type of 'C'.
}
