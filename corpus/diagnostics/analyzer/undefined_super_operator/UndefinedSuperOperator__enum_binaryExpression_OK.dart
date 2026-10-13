mixin M {
  int operator +(int a) => 0;
}

enum E with M {
  v;
  void f() {
    super + 0;
  }
}
