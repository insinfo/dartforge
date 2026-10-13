class A {
  int x = 0;
  void foo() {}
}
class A {
  int y = 0;
  void bar() {
    bar();
    this.bar();
    this.foo();
    y;
    this.y;
    this.x;
    foo();
  }
}
