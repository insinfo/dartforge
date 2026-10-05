abstract class I {
  void m();
}
class B implements I {}
class B implements I {
  void m() {
    this.m();
  }
}
class C implements I {
  void k() {
    this.k();
  }
}
class C implements I {
  void m() {}
}
