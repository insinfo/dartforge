mixin B {
  void m(int i) {}
}

class I {
  void m(String s) {}
}

class C extends Object with B implements I {
  void m(dynamic d) {}
}
