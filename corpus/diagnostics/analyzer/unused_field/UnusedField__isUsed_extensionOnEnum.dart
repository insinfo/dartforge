enum Foo {a, b}
extension Bar on Foo {
  int baz() => _baz;
  static final _baz = 1;
}
