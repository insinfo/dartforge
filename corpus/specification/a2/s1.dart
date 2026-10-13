extension type A(int it) {
  void m() { super.toString(); super + 1; super.x; super.x = 1; super[0]; }
  static void s() { super.m(); }
}
