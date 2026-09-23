class A {
  int get foo => 0;
}

mixin M implements A {
  void bar() {
    super.foo;
//        ^^^
// [diag.abstractSuperMemberReference] The getter 'foo' is always abstract in the supertype.
  }
}
