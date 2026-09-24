class C {}
extension E on C {
  int operator +(int x) => x;
}
f(C c) {
  E(c) + 2;
}
