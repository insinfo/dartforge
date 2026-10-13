mixin M {
  set foo(int _) {}
}

enum E with M {
  v;
  @override
  set foo(int _) {}
}
