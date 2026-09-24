abstract class A {
  void foo() {}
}

abstract class B extends A {
  void foo();
}

class C extends B {
  void bar() {
    super.foo();
  }
}
