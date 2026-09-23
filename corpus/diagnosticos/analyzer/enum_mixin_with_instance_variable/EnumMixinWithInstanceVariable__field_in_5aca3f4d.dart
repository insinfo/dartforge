mixin M {
  abstract final int foo;
}

enum E with M {
  v;
  final int foo = 0;
}
