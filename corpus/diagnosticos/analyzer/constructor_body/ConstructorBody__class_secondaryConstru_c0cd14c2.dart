class C {
  external factory C() => null;
//                     ^^
// [diag.externalFactoryWithBody] External factories can't have a body.
//                        ^^^^
// [diag.returnOfInvalidTypeFromConstructor] A value of type 'Null' can't be returned from the constructor 'C.new' because it has a return type of 'C'.
}
