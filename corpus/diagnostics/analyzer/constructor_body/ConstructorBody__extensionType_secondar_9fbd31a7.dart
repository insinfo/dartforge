extension type const E(int it) {
  external const factory E.named() {}
//                       ^^^^^^^
// [diag.bodyMightCompleteNormally] The body might complete normally, causing 'null' to be returned, but the return type, 'E', is a potentially non-nullable type.
//                                 ^
// [diag.externalFactoryWithBody] External factories can't have a body.
}
