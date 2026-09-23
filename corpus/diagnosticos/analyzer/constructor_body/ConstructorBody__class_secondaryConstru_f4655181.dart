class C {
  external const C() => null;
//                   ^^
// [diag.externalMethodWithBody] An external or native method can't have a body.
// [diag.constConstructorWithBody] Const constructors can't have a body.
//                   ^^^^^^^^
// [diag.returnInGenerativeConstructor] Constructors can't return values.
//                      ^^^^
// [diag.returnOfInvalidTypeFromConstructor] A value of type 'Null' can't be returned from the constructor 'C.new' because it has a return type of 'C'.
}
