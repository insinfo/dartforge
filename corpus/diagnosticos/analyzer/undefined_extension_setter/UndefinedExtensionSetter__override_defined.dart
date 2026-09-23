extension E on int {
  void set foo(int _) {}
}
f() {
  E(0).foo = 1;
}
