class I {
  m(p) {}
}
class B {
  noSuchMethod(v) => null;
}
class C extends B implements I {
  noSuchMethod(v);
}
