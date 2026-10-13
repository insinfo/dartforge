class B {
  void m(int i) {}
}

class I {
  void m(String s) {}
}

class C extends B implements I {
  void m(dynamic d) {}
}
