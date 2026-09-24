class Foo {}
extension Bar on Foo {
  static final _baz = 7;
//             ^^^^
// [diag.unusedField] The value of the field '_baz' isn't used.
}
