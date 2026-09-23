mixin A {
  int get foo;
  noSuchMethod(im) => 1;
}

class B extends Object with A {
  int get foo => super.foo; // ref
//                     ^^^
// [diag.abstractSuperMemberReference] The getter 'foo' is always abstract in the supertype.
  noSuchMethod(im) => 2;
}
