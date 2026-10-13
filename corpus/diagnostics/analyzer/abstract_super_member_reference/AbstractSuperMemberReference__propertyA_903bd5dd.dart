mixin A {
  set foo(int a);
  noSuchMethod(im) {}
}

class B extends Object with A {
  set foo(int a) => super.foo = a; // ref
//                        ^^^
// [diag.abstractSuperMemberReference] The setter 'foo' is always abstract in the supertype.
  noSuchMethod(im) {}
}
