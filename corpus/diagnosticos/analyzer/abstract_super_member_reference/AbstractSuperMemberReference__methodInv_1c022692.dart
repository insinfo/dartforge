abstract class A {
  void foo(int _);
}

abstract class B extends A {
  void bar() {
    super.foo(0);
//        ^^^
// [diag.abstractSuperMemberReference] The method 'foo' is always abstract in the supertype.
  }

  void foo(int _) {} // does not matter
}
