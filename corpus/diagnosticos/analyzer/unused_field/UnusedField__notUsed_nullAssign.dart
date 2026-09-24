class A {
  var _f;
  m() {
    _f ??= doSomething();
  }
}
doSomething() => 0;
