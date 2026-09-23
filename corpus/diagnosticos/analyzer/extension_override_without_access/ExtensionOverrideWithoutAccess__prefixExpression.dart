class C {}
extension E on C {
  int operator -() => 7;
}
f(C c) {
  -E(c);
}
