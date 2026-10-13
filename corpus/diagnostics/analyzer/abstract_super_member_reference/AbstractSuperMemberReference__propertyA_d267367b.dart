class A {
  int get foo;
  noSuchMethod(im) => 1;
}

class B extends A {
  get foo => super.foo; // ref
  noSuchMethod(im) => 2;
}
