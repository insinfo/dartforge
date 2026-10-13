class C {
  external const factory C() {}
//                       ^
// [diag.bodyMightCompleteNormally] The body might complete normally, causing 'null' to be returned, but the return type, 'C', is a potentially non-nullable type.
//                           ^
// [diag.externalFactoryWithBody] External factories can't have a body.
}
