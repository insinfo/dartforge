class C {
  const factory C() => null;
//^^^^^
// [diag.constFactory] Only redirecting factory constructors can be declared to be 'const'.
//                     ^^^^
// [diag.returnOfInvalidTypeFromConstructor] A value of type 'Null' can't be returned from the constructor 'C.new' because it has a return type of 'C'.
}
