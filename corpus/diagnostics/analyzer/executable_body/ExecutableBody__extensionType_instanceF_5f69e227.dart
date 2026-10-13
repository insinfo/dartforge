extension type E(int i) {
  abstract final int foo;
//                   ^^^
// [diag.inducedGetterWithoutBody] The getter induced by 'foo' must have a body.
}
