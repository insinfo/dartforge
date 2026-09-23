abstract class A {
  int get foo;
}

abstract class B extends A {
  bar() {
    super.foo; // ref
//        ^^^
// [diag.abstractSuperMemberReference] The getter 'foo' is always abstract in the supertype.
  }
}
