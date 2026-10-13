f() {
  late int v;

  void g() {
    v = 0;
  }

  g();
  v;
}
