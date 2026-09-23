class A {
  set foo(int a);
  noSuchMethod(im) => 1;
}

class B extends A {
  set foo(int a) => super.foo = a; // ref
  noSuchMethod(im) => 2;
}
