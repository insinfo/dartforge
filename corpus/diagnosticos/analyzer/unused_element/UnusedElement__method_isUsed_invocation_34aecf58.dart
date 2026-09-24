class A {
  _m() {}
}
mixin M on A {
  useMethod() {
    _m();
  }
}
