abstract class A {
  void set foo(int _) {}
}

abstract class B extends A {
  void set foo(int _);
}

class C extends B {
  void bar() {
    super.foo = 0;
  }
}
