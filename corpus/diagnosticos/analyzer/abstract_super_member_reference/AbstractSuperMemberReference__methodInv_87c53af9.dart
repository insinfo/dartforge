class A {
  void foo(int _) {}
}

mixin M implements A {
  void bar() {
    super.foo(0);
//        ^^^
// [diag.abstractSuperMemberReference] The method 'foo' is always abstract in the supertype.
  }
}
