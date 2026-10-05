enum E1 {
  v;
  static int E1() => 0;
}
enum E2 {
  v;
  E2();
  E2.named() : this();
}
enum E3 {
  v;
  void E3() {}
}
