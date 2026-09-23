class C {
  factory C.named() => 7;
//                     ^
// [diag.returnOfInvalidTypeFromConstructor] A value of type 'int' can't be returned from the constructor 'C.named' because it has a return type of 'C'.
}
