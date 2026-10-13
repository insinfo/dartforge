class A {
  static abstract final int foo;
//                          ^^^
// [diag.inducedGetterWithoutBody] The getter induced by 'foo' must have a body.
}
