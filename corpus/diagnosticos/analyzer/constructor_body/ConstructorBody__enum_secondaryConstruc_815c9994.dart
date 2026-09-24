enum E {
  v;
  const E();
  const E.named() => null;
//        ^^^^^
// [diag.unusedElement] The declaration 'E.named' isn't referenced.
//                ^^
// [diag.constConstructorWithBody] Const constructors can't have a body.
//                ^^^^^^^^
// [diag.returnInGenerativeConstructor] Constructors can't return values.
//                   ^^^^
// [diag.returnOfInvalidTypeFromConstructor] A value of type 'Null' can't be returned from the constructor 'E.named' because it has a return type of 'E'.
}
