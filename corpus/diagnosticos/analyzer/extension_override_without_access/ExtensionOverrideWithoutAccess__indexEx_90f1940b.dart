class C {}
extension E on C {
  int operator [](int i) => 4;
}
f(C c) {
  E(c)[2];
}
