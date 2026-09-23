mixin A {
  _m() {}
}
class C with A {
  useMethod() {
    _m();
  }
}
