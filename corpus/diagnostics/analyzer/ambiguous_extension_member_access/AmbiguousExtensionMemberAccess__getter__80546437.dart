extension E1 on int {
  void get a => 1;
}

extension E2 on int {
  static void get a => 2;
}

f() {
  0.a;
}
