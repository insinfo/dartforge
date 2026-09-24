extension type const E(int it) {
  external const factory E.named() => null;
//                                 ^^
// [diag.externalFactoryWithBody] External factories can't have a body.
//                                    ^^^^
// [diag.returnOfInvalidTypeFromConstructor] A value of type 'Null' can't be returned from the constructor 'E.named' because it has a return type of 'E'.
}
