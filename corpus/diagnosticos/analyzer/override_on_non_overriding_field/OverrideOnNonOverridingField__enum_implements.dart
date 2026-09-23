class A {
  int get a => 0;
  void set b(int _) {}
}

enum E implements A {
  v;
  @override
  int get a => 0;

  @override
  void set b(int _) {}
}
