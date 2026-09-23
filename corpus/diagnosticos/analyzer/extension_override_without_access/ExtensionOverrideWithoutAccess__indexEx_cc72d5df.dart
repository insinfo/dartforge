class C {}
extension E on C {
  void operator []=(int i, int v) {}
}
f(C c) {
  E(c)[2] = 5;
}
