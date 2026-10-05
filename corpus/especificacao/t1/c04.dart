abstract class I {
  void m();
}
class B implements I {}
class B implements I {
  void m() {}
}
class C implements I {
  void m() {}
}
class C implements I {}
