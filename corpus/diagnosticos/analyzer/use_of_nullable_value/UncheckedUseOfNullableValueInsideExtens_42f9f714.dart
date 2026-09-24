class A {
  set foo(int _) {}
}

extension E on A {
  set bar(int _) {}

  void baz() {
    foo = 0;
    this.foo = 0;

    bar = 0;
    this.bar = 0;
  }
}
