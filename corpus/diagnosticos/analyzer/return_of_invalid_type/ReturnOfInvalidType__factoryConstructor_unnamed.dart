class C {
  factory C() => 7;
//               ^
// [diag.returnOfInvalidTypeFromConstructor] A value of type 'int' can't be returned from the constructor 'C.new' because it has a return type of 'C'.
}
