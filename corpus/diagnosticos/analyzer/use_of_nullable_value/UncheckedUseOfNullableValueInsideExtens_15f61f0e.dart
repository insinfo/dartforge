class A {
  A operator-() => this;
}

extension E on A {
  void bar() {
    -this;
  }
}
