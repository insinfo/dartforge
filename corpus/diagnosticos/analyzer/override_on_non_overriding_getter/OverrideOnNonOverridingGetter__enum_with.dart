mixin M {
  int get foo => 0;
}

enum E with M {
  v;
  @override
  int get foo => 0;
}
