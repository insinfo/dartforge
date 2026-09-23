class C {}
extension E on C {
  set s(int x) {}
}
f(C c) {
  E(c).s = 3;
}
