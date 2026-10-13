mixin M {
  int get a => 0;
  void set b(int _) {}
}

enum E with M {
  v;
  @override
  int get a => 0;

  @override
  void set b(int _) {}
}
