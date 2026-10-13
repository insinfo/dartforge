mixin M1 {
  void foo() {}
}

mixin M2 on M1 {
  void bar() {
    super.foo();
  }
}

class X with M1 {}
augment class X with M2 {}
