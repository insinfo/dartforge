class A {
  void foo() {}
}

mixin B {
  void foo();
}

class C extends A with B {
  void bar() {
    super.foo(); // ref
  }
}
