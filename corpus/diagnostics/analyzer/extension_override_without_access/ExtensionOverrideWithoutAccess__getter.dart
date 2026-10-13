class C {}
extension E on C {
  int get g => 0;
}
f(C c) {
  E(c).g;
}
