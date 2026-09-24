abstract class A {
  void foo();
}

abstract class B extends A {
  void bar() {
    super.foo; // ref
//        ^^^
// [diag.abstractSuperMemberReference] The method 'foo' is always abstract in the supertype.
  }
}
