enum E {
  v;
  const E();
  const factory E.named() => null;
//^^^^^
// [diag.constFactory] Only redirecting factory constructors can be declared to be 'const'.
//                           ^^^^
// [diag.returnOfInvalidTypeFromConstructor] A value of type 'Null' can't be returned from the constructor 'E.named' because it has a return type of 'E'.
}
