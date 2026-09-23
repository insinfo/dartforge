class A {}

mixin M {
  void foo() {}
}

class B = A with M;

class C extends B {
  void bar() {
    super.foo();
  }
}
