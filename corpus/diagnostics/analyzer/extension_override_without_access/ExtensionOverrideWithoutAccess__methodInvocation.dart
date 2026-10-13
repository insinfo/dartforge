class C {}
extension E on C {
  void m() {}
}
f(C c) {
  E(c).m();
}
