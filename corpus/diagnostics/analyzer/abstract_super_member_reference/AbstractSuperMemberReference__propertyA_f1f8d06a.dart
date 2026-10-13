abstract class A {
  set foo(int _);
}

abstract class B extends A {
  void bar() {
    super.foo = 0;
//        ^^^
// [diag.abstractSuperMemberReference] The setter 'foo' is always abstract in the supertype.
  }
}
