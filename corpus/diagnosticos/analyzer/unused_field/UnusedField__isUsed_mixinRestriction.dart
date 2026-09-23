class Foo {
  int _f = 0;
}
mixin M on Foo {
  int g() => _f;
}
