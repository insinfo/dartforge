class A {
  int get foo => 0;
}

abstract class B implements A {
}

class C extends B {
  int get foo => super.foo; // ref
//                     ^^^
// [diag.abstractSuperMemberReference] The getter 'foo' is always abstract in the supertype.
}
