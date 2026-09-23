mixin A {
  void foo();
  noSuchMethod(im) => 42;
}

class B extends Object with A {
  void foo() => super.foo(); // ref
//                    ^^^
// [diag.abstractSuperMemberReference] The method 'foo' is always abstract in the supertype.
  noSuchMethod(im) => 87;
}
