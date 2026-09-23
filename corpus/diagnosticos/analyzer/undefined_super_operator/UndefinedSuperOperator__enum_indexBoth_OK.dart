mixin M {
  int operator [](int index) => 0;
  void operator []=(int index, int value) {}
}

enum E with M {
  v;
  void f() {
    super[0]++;
  }
}
